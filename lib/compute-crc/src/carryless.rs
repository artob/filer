// This is free and unencumbered software released into the public domain.

//! CRCs by carry-less multiplication on x86-64, behind the `intrinsics`
//! feature: PCLMULQDQ's 128 bits, and VPCLMULQDQ's 256 and 512, chosen at
//! run time, with `std`, or else where the build enables them, by
//! `compute-multiversion`, a fork of `multiversion` that lets their
//! intrinsics be called without `unsafe` code here.
//!
//! A CRC is linear: the register that a message leaves is the sum of what
//! each of its words would leave alone, followed by the zeros of the
//! words after it. So a block is summed word by word, each on its own:
//! every 64-bit word times its weight, which is `x` to the power of the
//! bits after it, modulo the polynomial, a constant of a table built at
//! compile time; the products exclusive-ored together, none waiting on
//! another, two words an instruction pair, or four, or eight; and the sum,
//! sixteen bytes that leave the register as the block does, fed to the
//! tables once, a block of at most [`UNITS`] units. The register a block
//! starts from is a word before it, times the first word's weight.
//!
//! This is the definition's arithmetic alone, as `LEGAL.md` records: the
//! CRC of a message as the sum of its parts' CRCs, each times a power of
//! `x`. No remainder is carried along the message by multiplying it, as
//! folding does.

/// The bytes of a unit: two words, a vector of 128 bits.
pub(crate) const UNIT: usize = 16;

/// The units of a block, of 16 KiB: as many as the table has weights for.
/// The sum of a block is fed to the tables once, which costs little
/// beside so many units.
pub(crate) const UNITS: usize = 1024;

/// The fewest bytes worth summing: below them, the tables alone are as
/// fast.
pub(crate) const LEAST: usize = 64;

/// The weights of a block's words: a pair for each unit, the first unit's
/// first. A block shorter than [`UNITS`] takes the last of them. Aligned as
/// the widest vector is, so that none of a whole block's spans two lines
/// of the cache.
#[repr(align(64))]
pub(crate) struct Weights(pub(crate) [[u64; 2]; UNITS]);

/// Returns the weights of the CRC of `width` bits whose polynomial is
/// `poly`, as its register holds it: with its bits reversed, if
/// `reflected`, and without `x` to the power of `width` either way.
///
/// A word's weight is `x` to the power of the bits after it in the block,
/// modulo the polynomial. A reflected CRC's words and weights have their
/// bits reversed, the least significant the highest power, and the product
/// of two such comes out a bit short of reversed: times `x`. So its
/// weights are divided by `x`, which is the polynomial without its
/// constant term, divided by `x`, as their product is the polynomial plus
/// one.
// Evaluated at compile time, where any overflow or index out of range
// fails the build instead of panicking, as the tables are.
#[allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "evaluated at compile time"
)]
pub(crate) const fn weights(width: u32, poly: u64, reflected: bool) -> Weights {
    // The polynomial with `x^0` its least significant bit.
    let poly = if reflected {
        poly.reverse_bits() >> (u64::BITS - width)
    } else {
        poly
    };
    let mut weights = [[0; 2]; UNITS];
    let mut power = if reflected {
        ((poly ^ 1) >> 1) | (1 << (width - 1))
    } else {
        1
    };
    // From the block's last word, back to its first.
    let mut after = 0;
    while after < 2 * UNITS {
        weights[UNITS - 1 - after / 2][1 - after % 2] = if reflected {
            power.reverse_bits()
        } else {
            power
        };
        // Times `x^64`: `x`, sixty-four times.
        let mut bit = 0;
        while bit < u64::BITS {
            let out = (power >> (width - 1)) & 1;
            power = if width == u64::BITS {
                power << 1
            } else {
                (power << 1) & ((1 << width) - 1)
            };
            power ^= poly & out.wrapping_neg();
            bit += 1;
        }
        after += 1;
    }
    Weights(weights)
}

/// Returns the sum of `units`, each word times its weight of `weights`,
/// which has a pair for each unit, and of `register`, a word that the
/// first word's weight multiplies: 128 bits that leave a register, from
/// zero, as the units do from `register`. Or `None`, where the CPU has no
/// carry-less multiplication.
///
/// A word's first byte is its least significant, or with `forward`, as a
/// CRC that is not reflected reads them, its most. `widest` is the widest
/// vector to take, in bits: all that the CPU has, but in tests.
///
/// 512 bits only where the CPU also has AVX-512's BF16: those that slow
/// their clocks for vectors so wide, Intel's of 2017 to 2021, lack it, or
/// VPCLMULQDQ.
#[multiversion::multiversion(targets(
    "x86_64+avx512f+avx512vl+avx512bw+avx512bf16+vpclmulqdq+avx2+pclmulqdq+sse4.1+ssse3",
    "x86_64+avx2+vpclmulqdq+pclmulqdq+sse4.1+ssse3",
    "x86_64+pclmulqdq+sse4.1+ssse3"
))]
pub(crate) fn sum(
    register: u64,
    units: &[[u8; UNIT]],
    weights: &[[u64; 2]],
    forward: bool,
    widest: usize,
) -> Option<u128> {
    /// The sum of the units that `units` begins with, four at a time, and
    /// how many they are.
    #[multiversion::target::target_cfg(all(
        target_arch = "x86_64",
        target_feature = "avx512f",
        target_feature = "avx512bw",
        target_feature = "vpclmulqdq"
    ))]
    #[multiversion::inherit_target]
    fn fours(units: &[[u8; UNIT]], weights: &[[u64; 2]], forward: bool) -> (u128, usize) {
        use core::arch::x86_64::{
            _mm_cvtsi128_si64, _mm_extract_epi64, _mm_xor_si128, _mm512_castsi512_si128,
            _mm512_clmulepi64_epi128, _mm512_extracti32x4_epi32, _mm512_set_epi64,
            _mm512_setzero_si512, _mm512_shuffle_epi8, _mm512_ternarylogic_epi64,
        };
        let word =
            |bytes: Option<&[u8; 8]>| bytes.copied().map_or(0, u64::from_le_bytes).cast_signed();
        let words = |unit: &[u8; UNIT]| (word(unit.first_chunk()), word(unit.last_chunk()));
        // Each word's bytes, last to first.
        let (low, high) = (0x0001_0203_0405_0607, 0x0809_0A0B_0C0D_0E0F);
        let swap = _mm512_set_epi64(high, low, high, low, high, low, high, low);
        let (fours, _) = units.as_chunks::<4>();
        let (weights, _) = weights.as_chunks::<4>();
        let mut sum = _mm512_setzero_si512();
        for ([a, b, c, d], [i, j, k, l]) in fours.iter().zip(weights) {
            let (a, b, c, d) = (words(a), words(b), words(c), words(d));
            let data = _mm512_set_epi64(d.1, d.0, c.1, c.0, b.1, b.0, a.1, a.0);
            let data = match forward {
                true => _mm512_shuffle_epi8(data, swap),
                false => data,
            };
            let weights = _mm512_set_epi64(
                l[1].cast_signed(),
                l[0].cast_signed(),
                k[1].cast_signed(),
                k[0].cast_signed(),
                j[1].cast_signed(),
                j[0].cast_signed(),
                i[1].cast_signed(),
                i[0].cast_signed(),
            );
            let first = _mm512_clmulepi64_epi128::<0x00>(data, weights);
            let second = _mm512_clmulepi64_epi128::<0x11>(data, weights);
            sum = _mm512_ternarylogic_epi64::<0x96>(sum, first, second);
        }
        let halves = (
            _mm_xor_si128(
                _mm512_castsi512_si128(sum),
                _mm512_extracti32x4_epi32::<1>(sum),
            ),
            _mm_xor_si128(
                _mm512_extracti32x4_epi32::<2>(sum),
                _mm512_extracti32x4_epi32::<3>(sum),
            ),
        );
        let sum = _mm_xor_si128(halves.0, halves.1);
        let low = _mm_cvtsi128_si64(sum).cast_unsigned();
        let high = _mm_extract_epi64::<1>(sum).cast_unsigned();
        let sum = u128::from(low) | (u128::from(high) << 64);
        (sum, fours.as_flattened().len())
    }

    #[multiversion::target::target_cfg(not(all(
        target_arch = "x86_64",
        target_feature = "avx512f",
        target_feature = "avx512bw",
        target_feature = "vpclmulqdq"
    )))]
    fn fours(_: &[[u8; UNIT]], _: &[[u64; 2]], _: bool) -> (u128, usize) {
        (0, 0)
    }

    /// The sum of the units that `units` begins with, two at a time, and
    /// how many they are.
    #[multiversion::target::target_cfg(all(
        target_arch = "x86_64",
        target_feature = "avx2",
        target_feature = "vpclmulqdq"
    ))]
    #[multiversion::inherit_target]
    fn twos(units: &[[u8; UNIT]], weights: &[[u64; 2]], forward: bool) -> (u128, usize) {
        use core::arch::x86_64::{
            _mm_cvtsi128_si64, _mm_extract_epi64, _mm_xor_si128, _mm256_castsi256_si128,
            _mm256_clmulepi64_epi128, _mm256_extracti128_si256, _mm256_set_epi64x,
            _mm256_setzero_si256, _mm256_shuffle_epi8, _mm256_xor_si256,
        };
        let word =
            |bytes: Option<&[u8; 8]>| bytes.copied().map_or(0, u64::from_le_bytes).cast_signed();
        let words = |unit: &[u8; UNIT]| (word(unit.first_chunk()), word(unit.last_chunk()));
        let (low, high) = (0x0001_0203_0405_0607, 0x0809_0A0B_0C0D_0E0F);
        let swap = _mm256_set_epi64x(high, low, high, low);
        let (twos, _) = units.as_chunks::<2>();
        let (weights, _) = weights.as_chunks::<2>();
        let mut sum = _mm256_setzero_si256();
        for ([a, b], [i, j]) in twos.iter().zip(weights) {
            let (a, b) = (words(a), words(b));
            let data = _mm256_set_epi64x(b.1, b.0, a.1, a.0);
            let data = match forward {
                true => _mm256_shuffle_epi8(data, swap),
                false => data,
            };
            let weights = _mm256_set_epi64x(
                j[1].cast_signed(),
                j[0].cast_signed(),
                i[1].cast_signed(),
                i[0].cast_signed(),
            );
            let first = _mm256_clmulepi64_epi128::<0x00>(data, weights);
            let second = _mm256_clmulepi64_epi128::<0x11>(data, weights);
            sum = _mm256_xor_si256(sum, _mm256_xor_si256(first, second));
        }
        let sum = _mm_xor_si128(
            _mm256_castsi256_si128(sum),
            _mm256_extracti128_si256::<1>(sum),
        );
        let low = _mm_cvtsi128_si64(sum).cast_unsigned();
        let high = _mm_extract_epi64::<1>(sum).cast_unsigned();
        let sum = u128::from(low) | (u128::from(high) << 64);
        (sum, twos.as_flattened().len())
    }

    #[multiversion::target::target_cfg(not(all(
        target_arch = "x86_64",
        target_feature = "avx2",
        target_feature = "vpclmulqdq"
    )))]
    fn twos(_: &[[u8; UNIT]], _: &[[u64; 2]], _: bool) -> (u128, usize) {
        (0, 0)
    }

    /// The sum of `units`, one at a time, and of `register` times `first`.
    #[multiversion::target::target_cfg(all(
        target_arch = "x86_64",
        target_feature = "pclmulqdq",
        target_feature = "sse4.1",
        target_feature = "ssse3"
    ))]
    #[multiversion::inherit_target]
    fn ones(
        register: u64,
        first: u64,
        units: &[[u8; UNIT]],
        weights: &[[u64; 2]],
        forward: bool,
    ) -> Option<u128> {
        use core::arch::x86_64::{
            _mm_clmulepi64_si128, _mm_cvtsi128_si64, _mm_extract_epi64, _mm_set_epi64x,
            _mm_shuffle_epi8, _mm_xor_si128,
        };
        let word =
            |bytes: Option<&[u8; 8]>| bytes.copied().map_or(0, u64::from_le_bytes).cast_signed();
        let swap = _mm_set_epi64x(0x0809_0A0B_0C0D_0E0F, 0x0001_0203_0405_0607);
        let mut sum = _mm_clmulepi64_si128::<0x00>(
            _mm_set_epi64x(0, register.cast_signed()),
            _mm_set_epi64x(0, first.cast_signed()),
        );
        for (unit, weights) in units.iter().zip(weights) {
            let data = _mm_set_epi64x(word(unit.last_chunk()), word(unit.first_chunk()));
            let data = match forward {
                true => _mm_shuffle_epi8(data, swap),
                false => data,
            };
            let weights = _mm_set_epi64x(weights[1].cast_signed(), weights[0].cast_signed());
            let first = _mm_clmulepi64_si128::<0x00>(data, weights);
            let second = _mm_clmulepi64_si128::<0x11>(data, weights);
            sum = _mm_xor_si128(sum, _mm_xor_si128(first, second));
        }
        let low = _mm_cvtsi128_si64(sum).cast_unsigned();
        let high = _mm_extract_epi64::<1>(sum).cast_unsigned();
        Some(u128::from(low) | (u128::from(high) << 64))
    }

    #[multiversion::target::target_cfg(not(all(
        target_arch = "x86_64",
        target_feature = "pclmulqdq",
        target_feature = "sse4.1",
        target_feature = "ssse3"
    )))]
    fn ones(_: u64, _: u64, _: &[[u8; UNIT]], _: &[[u64; 2]], _: bool) -> Option<u128> {
        None
    }

    let first = weights.first().map_or(0, |pair| pair[0]);
    let (mut sum, mut taken) = (0, 0);
    if widest >= 512 {
        (sum, taken) = fours(units, weights, forward);
    }
    if widest >= 256 {
        let (units, weights) = (units.get(taken..)?, weights.get(taken..)?);
        let (more, pairs) = twos(units, weights, forward);
        (sum, taken) = (sum ^ more, taken.checked_add(pairs)?);
    }
    let (units, weights) = (units.get(taken..)?, weights.get(taken..)?);
    Some(sum ^ ones(register, first, units, weights, forward)?)
}

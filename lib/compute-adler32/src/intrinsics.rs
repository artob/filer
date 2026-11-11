// This is free and unencumbered software released into the public domain.

//! Adler-32's sums in the CPU's vectors, behind the `intrinsics` feature:
//! on x86-64, AVX2's of 256 bits, and AVX-512's of 512; on ARM, NEON's of
//! 128, by its dot products where the CPU has them; chosen at run time,
//! with `std`, or else where
//! the build enables them, by `compute-multiversion`, a fork of
//! `multiversion` that lets their intrinsics be called without `unsafe`
//! code here.
//!
//! Over a block, the first sum grows by the sum of the bytes, and the
//! second by each byte as many times as there are bytes from it to the
//! block's end, itself included, which is RFC 1950's definition summed
//! in another order. A chunk of 64 bytes is taken at a time: its bytes'
//! sum, by an instruction that sums each eight; each byte times the bytes
//! from it to the chunk's end, by instructions that multiply bytes by
//! constants and add the products, each two and then each four, or, with
//! AVX-512's VNNI, each four at once; and, as every byte before a chunk
//! counts 64 times more for it, the sum of the bytes before each.

/// The bytes of a chunk: the widest vector's.
pub(crate) const CHUNK: usize = 64;

/// The most chunks summed at once: few enough that a 32-bit lane of the
/// products' sums, which a chunk adds 63,750 to at most, every byte
/// `0xFF`, stays below 2^32, as a test shows.
pub(crate) const MOST: usize = 4096;

/// The fewest bytes worth summing in vectors: a chunk.
pub(crate) const LEAST: usize = CHUNK;

/// Returns the sum of the bytes of `chunks`, at most [`MOST`], and the sum
/// of each byte times the count of bytes from it to the last, itself
/// included. Or `None`, where the CPU has no vectors for it.
///
/// `widest` is the widest vector to take, in bits: all that the CPU has,
/// but in tests, where 256 also keeps ARM from its dot products. 512 bits
/// only where the CPU also has AVX-512's BF16:
/// those that slow their clocks for vectors so wide, Intel's of 2017 to
/// 2021, lack it.
#[multiversion::multiversion(targets(
    "x86_64+avx512f+avx512bw+avx512vl+avx512bf16+avx512vnni+avx2",
    "x86_64+avx2",
    "aarch64+dotprod+aes",
    "aarch64+aes"
))]
pub(crate) fn sums(chunks: &[[u8; CHUNK]], widest: usize) -> Option<(u64, u64)> {
    /// By vectors of 512 bits, a chunk each: the products and their sums
    /// by AVX-512's VNNI, an instruction a chunk, into four sums by turns,
    /// as each waits on its last.
    #[multiversion::target::target_cfg(all(
        target_arch = "x86_64",
        target_feature = "avx512f",
        target_feature = "avx512bw",
        target_feature = "avx512vnni"
    ))]
    #[multiversion::inherit_target]
    fn wide(chunks: &[[u8; CHUNK]]) -> Option<(u64, u64)> {
        use core::arch::x86_64::{
            _mm512_add_epi32, _mm512_add_epi64, _mm512_castsi512_si256, _mm512_cvtepu32_epi64,
            _mm512_dpbusd_epi32, _mm512_extracti64x4_epi64, _mm512_reduce_add_epi64,
            _mm512_sad_epu8, _mm512_set_epi64, _mm512_setzero_si512,
        };
        let word =
            |bytes: Option<&[u8; 8]>| bytes.copied().map_or(0, u64::from_le_bytes).cast_signed();
        let words = |unit: &[u8; 16]| (word(unit.first_chunk()), word(unit.last_chunk()));
        let zero = _mm512_setzero_si512();
        let load = |chunk: &[u8; CHUNK]| {
            let ([a, b, c, d], []) = chunk.as_chunks::<16>() else {
                return zero;
            };
            let (a, b, c, d) = (words(a), words(b), words(c), words(d));
            _mm512_set_epi64(d.1, d.0, c.1, c.0, b.1, b.0, a.1, a.0)
        };
        // The bytes from each byte to the chunk's end: 64 down to 1.
        let counts = _mm512_set_epi64(
            0x0102_0304_0506_0708,
            0x090A_0B0C_0D0E_0F10,
            0x1112_1314_1516_1718,
            0x191A_1B1C_1D1E_1F20,
            0x2122_2324_2526_2728,
            0x292A_2B2C_2D2E_2F30,
            0x3132_3334_3536_3738,
            0x393A_3B3C_3D3E_3F40,
        );
        // The bytes' sum; the sum of the bytes before each chunk; and the
        // products' sums.
        let (mut sum, mut before, mut products) = (zero, zero, [zero; 4]);
        let (fours, last) = chunks.as_chunks::<4>();
        for [first, second, third, fourth] in fours {
            let [p, q, r, s] = products;
            let data = load(first);
            before = _mm512_add_epi64(before, sum);
            sum = _mm512_add_epi64(sum, _mm512_sad_epu8(data, zero));
            let p = _mm512_dpbusd_epi32(p, data, counts);
            let data = load(second);
            before = _mm512_add_epi64(before, sum);
            sum = _mm512_add_epi64(sum, _mm512_sad_epu8(data, zero));
            let q = _mm512_dpbusd_epi32(q, data, counts);
            let data = load(third);
            before = _mm512_add_epi64(before, sum);
            sum = _mm512_add_epi64(sum, _mm512_sad_epu8(data, zero));
            let r = _mm512_dpbusd_epi32(r, data, counts);
            let data = load(fourth);
            before = _mm512_add_epi64(before, sum);
            sum = _mm512_add_epi64(sum, _mm512_sad_epu8(data, zero));
            let s = _mm512_dpbusd_epi32(s, data, counts);
            products = [p, q, r, s];
        }
        let [mut p, q, r, s] = products;
        for chunk in last {
            let data = load(chunk);
            before = _mm512_add_epi64(before, sum);
            sum = _mm512_add_epi64(sum, _mm512_sad_epu8(data, zero));
            p = _mm512_dpbusd_epi32(p, data, counts);
        }
        // Together, no more than one lane would hold of them all.
        let products = _mm512_add_epi32(_mm512_add_epi32(p, q), _mm512_add_epi32(r, s));
        let products = _mm512_add_epi64(
            _mm512_cvtepu32_epi64(_mm512_castsi512_si256(products)),
            _mm512_cvtepu32_epi64(_mm512_extracti64x4_epi64::<1>(products)),
        );
        let sum = _mm512_reduce_add_epi64(sum).cast_unsigned();
        let before = _mm512_reduce_add_epi64(before).cast_unsigned();
        let products = _mm512_reduce_add_epi64(products).cast_unsigned();
        let chunk = u64::try_from(CHUNK).ok()?;
        Some((sum, before.checked_mul(chunk)?.checked_add(products)?))
    }

    /// By NEON's vectors of 128 bits, four to a chunk, each with sums of
    /// its own, so that none waits on another's: the bytes' sums and the
    /// products' by the dot products of ARMv8.2, an instruction each. A
    /// vector is read whole as 128 bits, which only the `aes` feature's
    /// intrinsics take without a pointer.
    #[multiversion::target::target_cfg(all(
        target_arch = "aarch64",
        target_feature = "dotprod",
        target_feature = "aes"
    ))]
    #[multiversion::inherit_target]
    fn wide(chunks: &[[u8; CHUNK]]) -> Option<(u64, u64)> {
        use core::arch::aarch64::{
            vaddlvq_u32, vaddvq_u64, vdotq_u32, vdupq_n_u8, vdupq_n_u32, vdupq_n_u64, vpadalq_u32,
            vreinterpretq_u8_p128,
        };
        let load = |unit: &[u8; 16]| vreinterpretq_u8_p128(u128::from_le_bytes(*unit));
        // The bytes from each byte to the chunk's end: 64 down to 1.
        let counts: [u8; CHUNK] =
            core::array::from_fn(|n| u8::try_from(CHUNK.saturating_sub(n)).unwrap_or(0));
        let (counts, _) = counts.as_chunks::<16>();
        let ones = vdupq_n_u8(1);
        // Of each of a chunk's four vectors: the bytes' sum; the sum of the
        // bytes before each chunk; and the products' sum.
        let mut sums = [vdupq_n_u32(0); 4];
        let mut before = [vdupq_n_u64(0); 4];
        let mut products = [vdupq_n_u32(0); 4];
        for chunk in chunks {
            let (units, _) = chunk.as_chunks::<16>();
            let each = sums.iter_mut().zip(&mut before).zip(&mut products);
            for (((sum, before), products), (unit, counts)) in each.zip(units.iter().zip(counts)) {
                let data = load(unit);
                *before = vpadalq_u32(*before, *sum);
                *sum = vdotq_u32(*sum, data, ones);
                *products = vdotq_u32(*products, data, load(counts));
            }
        }
        let (mut sum, mut bytes_before, mut weighted) = (0_u64, 0_u64, 0_u64);
        for ((part, before), products) in sums.iter().zip(&before).zip(&products) {
            sum = sum.wrapping_add(vaddlvq_u32(*part));
            bytes_before = bytes_before.wrapping_add(vaddvq_u64(*before));
            weighted = weighted.wrapping_add(vaddlvq_u32(*products));
        }
        let chunk = u64::try_from(CHUNK).ok()?;
        Some((sum, bytes_before.checked_mul(chunk)?.checked_add(weighted)?))
    }

    #[multiversion::target::target_cfg(not(any(
        all(
            target_arch = "x86_64",
            target_feature = "avx512f",
            target_feature = "avx512bw",
            target_feature = "avx512vnni"
        ),
        all(
            target_arch = "aarch64",
            target_feature = "dotprod",
            target_feature = "aes"
        )
    )))]
    fn wide(_: &[[u8; CHUNK]]) -> Option<(u64, u64)> {
        None
    }

    /// By vectors of 256 bits, two to a chunk.
    #[multiversion::target::target_cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
    #[multiversion::inherit_target]
    fn half(chunks: &[[u8; CHUNK]]) -> Option<(u64, u64)> {
        use core::arch::x86_64::{
            _mm256_add_epi32, _mm256_add_epi64, _mm256_castsi256_si128, _mm256_cvtepu32_epi64,
            _mm256_extract_epi64, _mm256_extracti128_si256, _mm256_madd_epi16,
            _mm256_maddubs_epi16, _mm256_sad_epu8, _mm256_set_epi64x, _mm256_set1_epi16,
            _mm256_setzero_si256,
        };
        let word =
            |bytes: Option<&[u8; 8]>| bytes.copied().map_or(0, u64::from_le_bytes).cast_signed();
        let words = |unit: &[u8; 16]| (word(unit.first_chunk()), word(unit.last_chunk()));
        // The four lanes of 64 bits, summed.
        let total = |lanes| {
            let lanes = [
                _mm256_extract_epi64::<0>(lanes),
                _mm256_extract_epi64::<1>(lanes),
                _mm256_extract_epi64::<2>(lanes),
                _mm256_extract_epi64::<3>(lanes),
            ];
            lanes
                .iter()
                .fold(0_u64, |sum, lane| sum.wrapping_add(lane.cast_unsigned()))
        };
        // The bytes from each byte to the chunk's end: 64 down to 33, and
        // 32 down to 1.
        let first_counts = _mm256_set_epi64x(
            0x2122_2324_2526_2728,
            0x292A_2B2C_2D2E_2F30,
            0x3132_3334_3536_3738,
            0x393A_3B3C_3D3E_3F40,
        );
        let second_counts = _mm256_set_epi64x(
            0x0102_0304_0506_0708,
            0x090A_0B0C_0D0E_0F10,
            0x1112_1314_1516_1718,
            0x191A_1B1C_1D1E_1F20,
        );
        let (zero, ones) = (_mm256_setzero_si256(), _mm256_set1_epi16(1));
        let (mut sum, mut before, mut products) = (zero, zero, zero);
        for chunk in chunks {
            let ([a, b, c, d], []) = chunk.as_chunks::<16>() else {
                continue;
            };
            let (a, b, c, d) = (words(a), words(b), words(c), words(d));
            let first = _mm256_set_epi64x(b.1, b.0, a.1, a.0);
            let second = _mm256_set_epi64x(d.1, d.0, c.1, c.0);
            before = _mm256_add_epi64(before, sum);
            sum = _mm256_add_epi64(sum, _mm256_sad_epu8(first, zero));
            sum = _mm256_add_epi64(sum, _mm256_sad_epu8(second, zero));
            let pairs = _mm256_maddubs_epi16(first, first_counts);
            products = _mm256_add_epi32(products, _mm256_madd_epi16(pairs, ones));
            let pairs = _mm256_maddubs_epi16(second, second_counts);
            products = _mm256_add_epi32(products, _mm256_madd_epi16(pairs, ones));
        }
        let products = _mm256_add_epi64(
            _mm256_cvtepu32_epi64(_mm256_castsi256_si128(products)),
            _mm256_cvtepu32_epi64(_mm256_extracti128_si256::<1>(products)),
        );
        let chunk = u64::try_from(CHUNK).ok()?;
        let weighted = total(before)
            .checked_mul(chunk)?
            .checked_add(total(products))?;
        Some((total(sum), weighted))
    }

    /// By NEON's vectors of 128 bits, four to a chunk, each with sums of
    /// its own, so that none waits on another's: the bytes' sums, each two
    /// and then each four; and the products, each byte times its count,
    /// each two of which, below 2^15, add in 16 bits, and each two of
    /// those in 32. A vector is read whole as 128 bits, which only the
    /// `aes` feature's intrinsics take without a pointer.
    #[multiversion::target::target_cfg(all(target_arch = "aarch64", target_feature = "aes"))]
    #[multiversion::inherit_target]
    fn half(chunks: &[[u8; CHUNK]]) -> Option<(u64, u64)> {
        use core::arch::aarch64::{
            vaddlvq_u32, vaddq_u16, vaddvq_u64, vdupq_n_u32, vdupq_n_u64, vget_low_u8,
            vmull_high_u8, vmull_u8, vpadalq_u16, vpadalq_u32, vpaddlq_u8, vreinterpretq_u8_p128,
        };
        let load = |unit: &[u8; 16]| vreinterpretq_u8_p128(u128::from_le_bytes(*unit));
        // The bytes from each byte to the chunk's end: 64 down to 1.
        let counts: [u8; CHUNK] =
            core::array::from_fn(|n| u8::try_from(CHUNK.saturating_sub(n)).unwrap_or(0));
        let (counts, _) = counts.as_chunks::<16>();
        // Of each of a chunk's four vectors: the bytes' sum; the sum of the
        // bytes before each chunk; and the products' sum.
        let mut sums = [vdupq_n_u32(0); 4];
        let mut before = [vdupq_n_u64(0); 4];
        let mut products = [vdupq_n_u32(0); 4];
        for chunk in chunks {
            let (units, _) = chunk.as_chunks::<16>();
            let each = sums.iter_mut().zip(&mut before).zip(&mut products);
            for (((sum, before), products), (unit, counts)) in each.zip(units.iter().zip(counts)) {
                let (data, counts) = (load(unit), load(counts));
                *before = vpadalq_u32(*before, *sum);
                *sum = vpadalq_u16(*sum, vpaddlq_u8(data));
                let pairs = vaddq_u16(
                    vmull_u8(vget_low_u8(data), vget_low_u8(counts)),
                    vmull_high_u8(data, counts),
                );
                *products = vpadalq_u16(*products, pairs);
            }
        }
        let (mut sum, mut bytes_before, mut weighted) = (0_u64, 0_u64, 0_u64);
        for ((part, before), products) in sums.iter().zip(&before).zip(&products) {
            sum = sum.wrapping_add(vaddlvq_u32(*part));
            bytes_before = bytes_before.wrapping_add(vaddvq_u64(*before));
            weighted = weighted.wrapping_add(vaddlvq_u32(*products));
        }
        let chunk = u64::try_from(CHUNK).ok()?;
        Some((sum, bytes_before.checked_mul(chunk)?.checked_add(weighted)?))
    }

    #[multiversion::target::target_cfg(not(any(
        all(target_arch = "x86_64", target_feature = "avx2"),
        all(target_arch = "aarch64", target_feature = "aes")
    )))]
    fn half(_: &[[u8; CHUNK]]) -> Option<(u64, u64)> {
        None
    }

    let sums = match widest >= 512 {
        true => wide(chunks),
        false => None,
    };
    match sums {
        None => half(chunks),
        sums => sums,
    }
}

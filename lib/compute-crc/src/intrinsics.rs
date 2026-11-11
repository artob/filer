// This is free and unencumbered software released into the public domain.

//! The CRC instructions of x86-64's SSE4.2, for CRC-32C, and of ARM's
//! `crc` extension, for CRC-32 and CRC-32C, behind the `intrinsics`
//! feature: chosen at run time, with `std`, or else where the build enables
//! them, by `compute-multiversion`, a fork of `multiversion` that lets
//! their intrinsics be called without `unsafe` code here. Carry-less
//! multiplication, x86-64's and ARM's, for every CRC, is `carryless`'s.
//!
//! Each takes the register eight bytes an instruction, from any state, as
//! the tables do, and returns what it comes to; or `None` where the CPU
//! lacks the instruction, and the tables are to be used.
//!
//! An instruction waits on the one before it, which leaves the CPU room
//! for more, so each runs [three streams](streams) at once and combines
//! them, as a CRC's definition allows: the register, fed a message from
//! some state, comes to that state times `x` to the power of the message's
//! bits, plus what the message comes to from zero, modulo the polynomial.
//! The powers are constants, and their products with each byte a register
//! may hold are tables, taken bit by bit at compile time: the integer
//! instructions alone, never carry-less multiplication.

/// The bytes that each of the three streams takes before they combine:
/// enough that combining costs little beside them, and few enough that
/// short inputs take three. On Apple M4, 256 came within 10% of 1,024's
/// speed over long inputs, and was 1.8 times as fast over pieces of 1 KiB.
#[cfg(any(
    all(
        target_arch = "aarch64",
        any(feature = "crc32", feature = "crc32-bzip2", feature = "crc32c")
    ),
    all(target_arch = "x86_64", feature = "crc32c")
))]
const STREAM: usize = 256;

/// The polynomials of CRC-32, and of CRC-32/BZIP2 too, and of CRC-32C,
/// their bits reversed as the instructions read them, without `x^32`.
#[cfg(all(
    target_arch = "aarch64",
    any(feature = "crc32", feature = "crc32-bzip2")
))]
const CRC32: u32 = 0xEDB8_8320;
#[cfg(all(
    any(target_arch = "aarch64", target_arch = "x86_64"),
    feature = "crc32c"
))]
const CRC32C: u32 = 0x82F6_3B78;

/// Feeds `bytes` to a register of the polynomial `POLY`, from `state`, by
/// `step`, which feeds it eight bytes, in blocks of three streams of
/// [`STREAM`] bytes, each fed its own register, the first from `state` and
/// the others from zero, which then combine; and returns the register, and
/// the bytes past the last whole block, for one stream to take.
#[cfg(any(
    all(
        target_arch = "aarch64",
        any(feature = "crc32", feature = "crc32-bzip2", feature = "crc32c")
    ),
    all(target_arch = "x86_64", feature = "crc32c")
))]
#[inline(always)]
fn streams<const POLY: u32>(
    mut state: u32,
    bytes: &[u8],
    step: impl Fn(u32, [u8; 8]) -> u32,
) -> (u32, &[u8]) {
    // What the first stream's register and the second's are multiplied by,
    // for the bytes of the streams after them, by tables.
    let far = &const { times::<POLY>(power::<POLY>(2 * STREAM)) };
    let near = &const { times::<POLY>(power::<POLY>(STREAM)) };
    let (blocks, rest) = bytes.as_chunks::<{ 3 * STREAM }>();
    for block in blocks {
        let ([first, second, third], []) = block.as_chunks::<STREAM>() else {
            continue;
        };
        let (mut a, mut b, mut c) = (state, 0, 0);
        let together = first
            .as_chunks::<8>()
            .0
            .iter()
            .zip(second.as_chunks::<8>().0)
            .zip(third.as_chunks::<8>().0);
        for ((x, y), z) in together {
            (a, b, c) = (step(a, *x), step(b, *y), step(c, *z));
        }
        state = by(far, a) ^ by(near, b) ^ c;
    }
    (state, rest)
}

/// Returns `a` times `b` modulo `x^32` plus `POLY`, each a polynomial of
/// degree below 32, its bits reversed: the most significant, `x^0`.
#[cfg(any(
    all(
        target_arch = "aarch64",
        any(feature = "crc32", feature = "crc32-bzip2", feature = "crc32c")
    ),
    all(target_arch = "x86_64", feature = "crc32c")
))]
const fn multiply<const POLY: u32>(a: u32, mut b: u32) -> u32 {
    let mut product = 0;
    let mut term = 0_u32;
    while term < 32 {
        // `a`'s `x^term`, which multiplies `b`, times `x` once a term.
        let set = (a >> 31_u32.wrapping_sub(term)) & 1;
        product ^= b & set.wrapping_neg();
        // Times `x`: `x^31` shifts out, to `x^32`, which is `POLY`.
        b = (b >> 1) ^ (POLY & (b & 1).wrapping_neg());
        term = term.wrapping_add(1);
    }
    product
}

/// Tables of what each byte of a register comes to, times a constant: a
/// table for each of its four bytes, the first its least significant.
#[cfg(any(
    all(
        target_arch = "aarch64",
        any(feature = "crc32", feature = "crc32-bzip2", feature = "crc32c")
    ),
    all(target_arch = "x86_64", feature = "crc32c")
))]
type Times = [[u32; 256]; 4];

/// Returns the tables of [`Times`] for `constant`, modulo `x^32` plus
/// `POLY`.
#[cfg(any(
    all(
        target_arch = "aarch64",
        any(feature = "crc32", feature = "crc32-bzip2", feature = "crc32c")
    ),
    all(target_arch = "x86_64", feature = "crc32c")
))]
// Evaluated at compile time, where any overflow or index out of range
// fails the build instead of panicking, as the tables are.
#[allow(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::indexing_slicing,
    reason = "evaluated at compile time"
)]
const fn times<const POLY: u32>(constant: u32) -> Times {
    let mut tables = [[0; 256]; 4];
    let mut byte = 0;
    while byte < 4 {
        let mut value = 0;
        while value < 256 {
            // A byte's value, in its place in the register.
            let register = (value as u32) << (8 * byte);
            tables[byte][value] = multiply::<POLY>(register, constant);
            value += 1;
        }
        byte += 1;
    }
    tables
}

/// Returns `register` times the constant that `times` tabulates: each of
/// its bytes' products, summed.
#[cfg(any(
    all(
        target_arch = "aarch64",
        any(feature = "crc32", feature = "crc32-bzip2", feature = "crc32c")
    ),
    all(target_arch = "x86_64", feature = "crc32c")
))]
#[inline(always)]
fn by(times: &Times, register: u32) -> u32 {
    let mut product = 0;
    for (table, byte) in times.iter().zip(register.to_le_bytes()) {
        product ^= table.get(usize::from(byte)).copied().unwrap_or_default();
    }
    product
}

/// Returns `x` to the power of `bytes` bytes' bits, modulo `x^32` plus
/// `POLY`, its bits reversed, by squaring.
#[cfg(any(
    all(
        target_arch = "aarch64",
        any(feature = "crc32", feature = "crc32-bzip2", feature = "crc32c")
    ),
    all(target_arch = "x86_64", feature = "crc32c")
))]
const fn power<const POLY: u32>(bytes: usize) -> u32 {
    // `x^0` and `x^1`.
    let (mut power, mut square) = (1 << 31, 1 << 30);
    let mut bits = bytes.wrapping_mul(8);
    while bits > 0 {
        if bits & 1 != 0 {
            power = multiply::<POLY>(power, square);
        }
        square = multiply::<POLY>(square, square);
        bits >>= 1;
    }
    power
}

/// Feeds `bytes` to a CRC-32C register, by SSE4.2's `crc32` or ARM's
/// `crc32c*`.
#[cfg(feature = "crc32c")]
#[multiversion::multiversion(targets("x86_64+sse4.2", "aarch64+crc"))]
pub(crate) fn crc32c(state: u32, bytes: &[u8]) -> Option<u32> {
    #[multiversion::target::target_cfg(all(target_arch = "x86_64", target_feature = "sse4.2"))]
    #[multiversion::inherit_target]
    fn feed(state: u32, bytes: &[u8]) -> Option<u32> {
        use core::arch::x86_64::{_mm_crc32_u8, _mm_crc32_u64};
        // Always fits: the instruction leaves the top half zero.
        let word = |crc: u32, word: [u8; 8]| {
            let crc = _mm_crc32_u64(u64::from(crc), u64::from_le_bytes(word));
            u32::try_from(crc).unwrap_or_default()
        };
        let (state, bytes) =
            crate::intrinsics::streams::<{ crate::intrinsics::CRC32C }>(state, bytes, word);
        let (words, rest) = bytes.as_chunks::<8>();
        let crc = words.iter().fold(state, |crc, &bytes| word(crc, bytes));
        Some(rest.iter().fold(crc, |crc, &byte| _mm_crc32_u8(crc, byte)))
    }

    #[multiversion::target::target_cfg(all(target_arch = "aarch64", target_feature = "crc"))]
    #[multiversion::inherit_target]
    fn feed(state: u32, bytes: &[u8]) -> Option<u32> {
        use core::arch::aarch64::{__crc32cb, __crc32cd};
        let word = |crc, word| __crc32cd(crc, u64::from_le_bytes(word));
        let (state, bytes) =
            crate::intrinsics::streams::<{ crate::intrinsics::CRC32C }>(state, bytes, word);
        let (words, rest) = bytes.as_chunks::<8>();
        let crc = words.iter().fold(state, |crc, &bytes| word(crc, bytes));
        Some(rest.iter().fold(crc, |crc, &byte| __crc32cb(crc, byte)))
    }

    #[multiversion::target::target_cfg(not(any(
        all(target_arch = "x86_64", target_feature = "sse4.2"),
        all(target_arch = "aarch64", target_feature = "crc")
    )))]
    fn feed(_: u32, _: &[u8]) -> Option<u32> {
        None
    }

    feed(state, bytes)
}

/// Feeds `bytes` to a CRC-32 register, by ARM's `crc32*`; x86 has no
/// instruction for it.
#[cfg(feature = "crc32")]
#[multiversion::multiversion(targets("aarch64+crc"))]
pub(crate) fn crc32(state: u32, bytes: &[u8]) -> Option<u32> {
    #[multiversion::target::target_cfg(all(target_arch = "aarch64", target_feature = "crc"))]
    #[multiversion::inherit_target]
    fn feed(state: u32, bytes: &[u8]) -> Option<u32> {
        use core::arch::aarch64::{__crc32b, __crc32d};
        let word = |crc, word| __crc32d(crc, u64::from_le_bytes(word));
        let (state, bytes) =
            crate::intrinsics::streams::<{ crate::intrinsics::CRC32 }>(state, bytes, word);
        let (words, rest) = bytes.as_chunks::<8>();
        let crc = words.iter().fold(state, |crc, &bytes| word(crc, bytes));
        Some(rest.iter().fold(crc, |crc, &byte| __crc32b(crc, byte)))
    }

    #[multiversion::target::target_cfg(not(all(target_arch = "aarch64", target_feature = "crc")))]
    fn feed(_: u32, _: &[u8]) -> Option<u32> {
        None
    }

    feed(state, bytes)
}

/// Feeds `bytes` to a CRC-32/BZIP2 register, by ARM's `crc32*`: CRC-32's
/// polynomial, read from each byte's most significant bit, as the
/// instructions read from its least with the bits of the register and of
/// each word reversed.
#[cfg(feature = "crc32-bzip2")]
#[multiversion::multiversion(targets("aarch64+crc"))]
pub(crate) fn crc32_bzip2(state: u32, bytes: &[u8]) -> Option<u32> {
    #[multiversion::target::target_cfg(all(target_arch = "aarch64", target_feature = "crc"))]
    #[multiversion::inherit_target]
    fn feed(state: u32, bytes: &[u8]) -> Option<u32> {
        use core::arch::aarch64::{__crc32b, __crc32d};
        let word = |crc, word| __crc32d(crc, u64::from_be_bytes(word).reverse_bits());
        let (state, bytes) = crate::intrinsics::streams::<{ crate::intrinsics::CRC32 }>(
            state.reverse_bits(),
            bytes,
            word,
        );
        let (words, rest) = bytes.as_chunks::<8>();
        let crc = words.iter().fold(state, |crc, &bytes| word(crc, bytes));
        let crc = rest
            .iter()
            .fold(crc, |crc, &byte| __crc32b(crc, byte.reverse_bits()));
        Some(crc.reverse_bits())
    }

    #[multiversion::target::target_cfg(not(all(target_arch = "aarch64", target_feature = "crc")))]
    fn feed(_: u32, _: &[u8]) -> Option<u32> {
        None
    }

    feed(state, bytes)
}

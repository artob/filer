// This is free and unencumbered software released into the public domain.

//! Adler-32, written clean room from RFC 1950, and dedicated to the public
//! domain: the checksum that zlib streams end with, and so PNG images and
//! the compressed files of btrfs, SquashFS, and APFS that are zlib's,
//! computed over bytes fed in pieces, as they arrive.
//!
//! Adler-32 is two sums modulo 65,521, the largest prime below 2^16: of
//! the bytes, plus one, and of each successive value of that first sum,
//! the second in the high half of the result. [`Adler32`] is fed by
//! [`update`](Adler32::update), or by [`chain`](Adler32::chain), which
//! returns the sum that goes on from there; it is a `Copy` value, so that
//! a copy taken anywhere goes on from there, as [`compute-crc`'s CRCs] are.
//! It sums the bytes in 16 lanes, every 16th byte each, which compilers
//! vectorize, and combines the lanes and reduces the sums every 64 KiB;
//! and fewer than 48 bytes one at a time.
//!
//! Its source is RFC 1950's definition; zlib's own code is not copied. It
//! is pure, safe Rust, and builds with `#![no_std]`, without an allocator.
//! The `std` feature, enabled by default, adds nothing by itself.
//!
//! The `intrinsics` feature, enabled by default (D66), sums 64 bytes at a
//! time in the CPU's vectors where it has them, from 64 bytes up: with
//! `std`, chosen at run time; without it, only where the build enables
//! them. It depends on `compute-multiversion`, a fork of `multiversion`
//! that lets their intrinsics be called without `unsafe` code here. On
//! x86-64, they are AVX2's of 256 bits, and AVX-512's of 512 with its
//! VNNI, where the CPU does not slow its clock for them: over a mebibyte,
//! 120 GB/s on Zen 5 and 86 on Granite Rapids. On ARM, NEON's of 128
//! bits, by its dot products where the CPU has them: 48 GB/s on Neoverse
//! V3, and 74 on Apple M4, where it takes 37 without them.
//!
//! Without `intrinsics`, as `default-features = false,
//! features = ["all", "std"]` builds it, it has no dependencies, all of it
//! public domain, and sums by the lanes alone: over long inputs, 7 times
//! slower on Zen 5, 18 GB/s, and on Granite Rapids, 13; 2.7 times on
//! Neoverse V3, 18; and 3 times on Apple M4, 23 (2026-11-11).
//!
//! It is one of the `compute-*` crates: clean-room, public-domain
//! checksums and hashes, each of the same shape: `new`, `update`, `chain`,
//! and `finish`.
//!
//! [`compute-crc`'s CRCs]: https://docs.rs/compute-crc
//!
//! # Examples
//!
//! ```
//! use compute_adler32::{Adler32, adler32};
//!
//! assert_eq!(adler32(b"123456789"), 0x091E_01DE);
//! let mut sum = Adler32::new();
//! sum.update(b"1234");
//! sum.update(b"56789");
//! assert_eq!(sum.finish(), Adler32::CHECK);
//! assert_eq!(Adler32::new().chain(b"1234").chain(b"56789"), sum);
//! ```

#![no_std]
#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
// No input may make this crate panic.
#![deny(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::let_underscore_must_use,
    clippy::panic,
    clippy::string_slice,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable,
    clippy::unwrap_used,
    unused_must_use
)]
// Tests may panic: that is how they fail.
#![cfg_attr(
    test,
    allow(
        clippy::arithmetic_side_effects,
        clippy::as_conversions,
        clippy::expect_used,
        clippy::indexing_slicing,
        clippy::let_underscore_must_use,
        clippy::panic,
        clippy::unreachable,
        clippy::unwrap_used
    )
)]

// Detecting the CPU's instructions at run time takes `std`.
#[cfg(all(feature = "intrinsics", feature = "std"))]
extern crate std;

// The CPU's instructions are taken only where a target takes vectors:
// not on soft-float targets, x86-64's without SSE2 and aarch64's without
// NEON, where Rust refuses their target features (rust-lang/rust#117938).
#[cfg(all(
    feature = "intrinsics",
    any(
        all(target_arch = "x86_64", target_feature = "sse2"),
        all(target_arch = "aarch64", target_feature = "neon")
    )
))]
mod intrinsics;

/// The modulus of both sums: the largest prime below 2^16.
const BASE: u32 = 65_521;

/// `BASE`, as the 64 bits that lanes combine in.
const WIDE_BASE: u64 = 65_521;

/// How many lanes take the bytes in turn, each its own two sums.
const LANES: usize = 16;

/// The bytes the lanes take before they are combined and the sums
/// reduced: as many groups of `LANES` as keep a lane's second sum below
/// 2^32, at every byte `0xFF`, as the tests show.
const BLOCK: usize = LANES * 4096;

/// Adler-32 (RFC 1950 §8.2) computed over bytes fed in pieces.
///
/// ```
/// use compute_adler32::Adler32;
///
/// // From where a zlib stream's dictionary left it, say.
/// let start = Adler32::new().chain(b"dictionary").state();
/// let sum = Adler32::from_state(start).chain(b"data");
/// assert_eq!(sum.finish(), Adler32::new().chain(b"dictionarydata").finish());
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Adler32 {
    /// The sum of the bytes, plus one, and the sum of its values, each
    /// below `BASE`.
    a: u32,
    b: u32,
}

impl Adler32 {
    /// Starts an Adler-32 over no bytes.
    #[must_use]
    pub const fn new() -> Self {
        Self { a: 1, b: 0 }
    }

    /// Feeds `bytes`.
    pub fn update(&mut self, bytes: &[u8]) {
        // Too few bytes for the CPU's vectors stay here, a comparison away
        // from the lanes.
        #[cfg(all(
            feature = "intrinsics",
            any(
                all(target_arch = "x86_64", target_feature = "sse2"),
                all(target_arch = "aarch64", target_feature = "neon")
            )
        ))]
        let bytes = if bytes.len() < intrinsics::LEAST {
            bytes
        } else {
            self.vectors(bytes, usize::MAX)
        };
        self.lanes(bytes);
    }

    /// Feeds the chunks of 64 bytes that `bytes` begins with, by the CPU's
    /// vectors, where it has them, and returns the bytes left: all of
    /// them, if it has none. `widest` is the widest vector to take, in
    /// bits.
    #[cfg(all(
        feature = "intrinsics",
        any(
            all(target_arch = "x86_64", target_feature = "sse2"),
            all(target_arch = "aarch64", target_feature = "neon")
        )
    ))]
    #[inline(never)]
    fn vectors<'a>(&mut self, bytes: &'a [u8], widest: usize) -> &'a [u8] {
        let (chunks, _) = bytes.as_chunks::<{ intrinsics::CHUNK }>();
        let (mut a, mut b) = (u64::from(self.a), u64::from(self.b));
        let mut left = bytes;
        for block in chunks.chunks(intrinsics::MOST) {
            let Some((sum, weighted)) = intrinsics::sums(block, widest) else {
                break;
            };
            // The second sum counts the first, as the block found it, once
            // a byte, and each byte once for itself and for each after it.
            // In 64 bits, which these sums never reach.
            let n = block.as_flattened().len();
            let count = u64::try_from(n).unwrap_or(u64::MAX);
            b = b
                .wrapping_add(count.wrapping_mul(a))
                .wrapping_add(weighted)
                .wrapping_rem(WIDE_BASE);
            a = a.wrapping_add(sum).wrapping_rem(WIDE_BASE);
            left = left.get(n..).unwrap_or_default();
        }
        self.a = u32::try_from(a).unwrap_or_default();
        self.b = u32::try_from(b).unwrap_or_default();
        left
    }

    /// Feeds `bytes` by sixteen lanes, which compilers vectorize.
    fn lanes(&mut self, bytes: &[u8]) {
        let (mut a, mut b) = (self.a, self.b);
        for block in bytes.chunks(BLOCK) {
            // Each lane sums every 16th byte, as Adler-32 sums them all,
            // independently, which compilers vectorize; the lanes'
            // arithmetic, though wrapping, never wraps within a block.
            // Too few bytes for the lanes to gain by go a byte at a time,
            // as the last few do: 47 of them stay far below 2^32.
            let (groups, rest) = match block.len() < 3 * LANES {
                true => (&[][..], block),
                false => block.as_chunks::<LANES>(),
            };
            if !groups.is_empty() {
                let (mut lane_a, mut lane_b) = ([0_u32; LANES], [0_u32; LANES]);
                for group in groups {
                    for (sum, &byte) in lane_a.iter_mut().zip(group) {
                        *sum = sum.wrapping_add(u32::from(byte));
                    }
                    for (sum, &first) in lane_b.iter_mut().zip(&lane_a) {
                        *sum = sum.wrapping_add(first);
                    }
                }
                // Then they combine. Over `n` bytes, each byte adds to the
                // second sum once for each value of the first that it is part
                // of: `n - j` times, for byte `j`, which is lane `i`'s `k`th,
                // `16 (m - k) - i` times, where lane `i`'s second sum counts
                // it `m - k` times, over `m` groups. In 64 bits, which these
                // sums never reach.
                let n = u64::try_from(groups.len().saturating_mul(LANES)).unwrap_or(u64::MAX);
                let (mut sum_a, mut sum_b, mut weighted) = (0_u64, 0_u64, 0_u64);
                for ((&first, &second), i) in lane_a.iter().zip(&lane_b).zip(0_u64..) {
                    sum_a = sum_a.wrapping_add(u64::from(first));
                    sum_b = sum_b.wrapping_add(u64::from(second));
                    weighted = weighted.wrapping_add(i.wrapping_mul(u64::from(first)));
                }
                let wide_a = u64::from(a).wrapping_add(sum_a);
                let wide_b = u64::from(b)
                    .wrapping_add(n.wrapping_mul(u64::from(a)))
                    .wrapping_add(sum_b.wrapping_mul(16))
                    .wrapping_sub(weighted);
                a = u32::try_from(wide_a.wrapping_rem(WIDE_BASE)).unwrap_or_default();
                b = u32::try_from(wide_b.wrapping_rem(WIDE_BASE)).unwrap_or_default();
            }
            // And the last few bytes, a byte at a time.
            for &byte in rest {
                a = a.wrapping_add(u32::from(byte));
                b = b.wrapping_add(a);
            }
            a = a.wrapping_rem(BASE);
            b = b.wrapping_rem(BASE);
        }
        (self.a, self.b) = (a, b);
    }

    /// Feeds `bytes`, and returns the sum that goes on from there, so that
    /// calls chain.
    #[must_use]
    pub fn chain(mut self, bytes: &[u8]) -> Self {
        self.update(bytes);
        self
    }

    /// Returns the Adler-32 of the bytes fed: the second sum above the
    /// first.
    #[must_use]
    pub const fn finish(&self) -> u32 {
        (self.b << 16) | self.a
    }

    /// Resumes an Adler-32 from `state`, an Adler-32 as
    /// [`state`](Self::state) and [`finish`](Self::finish) return it.
    #[must_use]
    pub const fn from_state(state: u32) -> Self {
        Self {
            a: (state & 0xFFFF) % BASE,
            b: (state >> 16) % BASE,
        }
    }

    /// Returns its state: the Adler-32 of the bytes fed, as nothing is
    /// inverted at the end.
    #[must_use]
    pub const fn state(&self) -> u32 {
        self.finish()
    }

    /// The Adler-32 of the nine bytes `123456789`.
    pub const CHECK: u32 = 0x091E_01DE;
}

impl Default for Adler32 {
    fn default() -> Self {
        Self::new()
    }
}

/// Returns the Adler-32 of `bytes`.
#[must_use]
pub fn adler32(bytes: &[u8]) -> u32 {
    Adler32::new().chain(bytes).finish()
}

// The README's example runs as a doctest.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

#[cfg(test)]
mod tests {
    use super::*;

    /// Adler-32 as RFC 1950 defines it, a byte at a time, reducing each
    /// step.
    fn defined(start: Adler32, bytes: &[u8]) -> u32 {
        let (mut a, mut b) = (start.a, start.b);
        for &byte in bytes {
            a = (a + u32::from(byte)) % BASE;
            b = (b + a) % BASE;
        }
        (b << 16) | a
    }

    #[test]
    fn bounds_the_lanes_as_documented() {
        assert_eq!(WIDE_BASE, u64::from(BASE));
        // After `m` groups of 0xFF, a lane's second sum is
        // `255 * m * (m + 1) / 2`.
        let m = (BLOCK / LANES) as u64;
        assert!(255 * m * (m + 1) / 2 <= u64::from(u32::MAX));
    }

    /// The CPU's vectors, where it has them, come to what the definition
    /// does, from any state: of each width, over less than a block of
    /// chunks, a block, and several, with bytes to spare, at the sums'
    /// largest.
    #[cfg(all(
        feature = "intrinsics",
        any(
            all(target_arch = "x86_64", target_feature = "sse2"),
            all(target_arch = "aarch64", target_feature = "neon")
        )
    ))]
    #[test]
    fn vectors_match_the_definition() {
        const BLOCK: usize = intrinsics::MOST * intrinsics::CHUNK;
        const LEN: usize = 2 * BLOCK + 300;
        static ONES: [u8; LEN] = [0xFF; LEN];
        static MIXED: [u8; LEN] = {
            let mut bytes = [0; LEN];
            let mut n = 0;
            while n < LEN {
                bytes[n] = (n * 37 % 251) as u8;
                n += 1;
            }
            bytes
        };
        // A lane of the products' sums, at its largest.
        assert!(63_750 * intrinsics::MOST as u64 <= u64::from(u32::MAX));
        let ends = [
            BLOCK - 64,
            BLOCK - 1,
            BLOCK,
            BLOCK + 63,
            BLOCK + 64,
            2 * BLOCK,
            LEN,
        ];
        // Whether the CPU has the vectors.
        #[cfg(all(feature = "std", target_arch = "x86_64"))]
        let vectors = std::is_x86_feature_detected!("avx2");
        #[cfg(all(feature = "std", target_arch = "aarch64"))]
        let vectors = std::arch::is_aarch64_feature_detected!("aes");
        for widest in [256, 512] {
            for bytes in [&ONES, &MIXED] {
                for start in [1, 0xFFF0_FFF0, 0x1234_5678] {
                    for len in (0..=300).step_by(7).chain(ends) {
                        let bytes = &bytes[..len];
                        let mut sum = Adler32::from_state(start);
                        let rest = sum.vectors(bytes, widest);
                        #[cfg(feature = "std")]
                        if vectors {
                            assert_eq!(rest.len(), len % intrinsics::CHUNK);
                        }
                        sum.lanes(rest);
                        let defined = defined(Adler32::from_state(start), bytes);
                        assert_eq!(sum.finish(), defined, "{len} bytes by {widest} bits");
                    }
                }
            }
        }
    }

    #[test]
    fn matches_the_definition() {
        assert_eq!(adler32(b"123456789"), Adler32::CHECK);
        assert_eq!(adler32(b"123456789"), defined(Adler32::new(), b"123456789"));
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
        assert_eq!(adler32(b""), 1);
        // Every length, around the reductions, at the sums' largest.
        let ones = [0xFF_u8; 2 * BLOCK + 40];
        let mixed: [u8; 2 * BLOCK + 40] = core::array::from_fn(|n| (n * 37 % 251) as u8);
        for bytes in [&ones, &mixed] {
            let ends = [0..40, BLOCK - 20..BLOCK + 20, bytes.len() - 20..bytes.len()];
            for len in ends.into_iter().flatten() {
                assert_eq!(
                    adler32(&bytes[..len]),
                    defined(Adler32::new(), &bytes[..len]),
                    "{len}"
                );
            }
            // In pieces, from any state.
            for piece in [1, 7, 16, 17, 5552, BLOCK - 1, BLOCK, BLOCK + 1] {
                let sum = bytes.chunks(piece).fold(Adler32::new(), Adler32::chain);
                assert_eq!(sum.finish(), defined(Adler32::new(), bytes), "{piece}");
                let resumed = bytes.chunks(piece).fold(Adler32::new(), |sum, chunk| {
                    Adler32::from_state(sum.state()).chain(chunk)
                });
                assert_eq!(resumed, sum, "{piece}");
            }
        }
    }
}

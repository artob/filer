// This is free and unencumbered software released into the public domain.

//! Adler-32, written clean room from RFC 1950, and dedicated to the public
//! domain: the checksum that zlib streams end with, and so PNG images and
//! the compressed files of btrfs, SquashFS, and APFS that are zlib's,
//! computed over bytes fed in pieces, as they arrive.
//!
//! Adler-32 is two sums modulo 65,521, the largest prime below 2^16: of
//! the bytes, plus one, and of each successive value of that first sum,
//! the second in the high half of the result. [`Adler32`] is a `Copy`
//! value whose [`update`](Adler32::update) returns the next, so that a
//! copy taken anywhere goes on from there, as [`compute-crc`'s CRCs] do.
//! It sums the bytes in 16 lanes, every 16th byte each, which compilers
//! vectorize, and combines the lanes and reduces the sums every 64 KiB.
//!
//! Its source is RFC 1950's definition; zlib's own code is not copied. It
//! is pure, safe Rust without dependencies, and builds with `#![no_std]`,
//! without an allocator. The `std` feature, enabled by default, currently
//! adds nothing.
//!
//! It is one of the `compute-*` crates: clean-room, public-domain
//! checksums and hashes, each of the same shape.
//!
//! [`compute-crc`'s CRCs]: https://docs.rs/compute-crc
//!
//! # Examples
//!
//! ```
//! use compute_adler32::{Adler32, adler32};
//!
//! assert_eq!(adler32(b"123456789"), 0x091E_01DE);
//! let sum = Adler32::new().update(b"1234").update(b"56789");
//! assert_eq!(sum.finish(), Adler32::CHECK);
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
/// let start = Adler32::new().update(b"dictionary").state();
/// let sum = Adler32::from_state(start).update(b"data");
/// assert_eq!(sum.finish(), Adler32::new().update(b"dictionarydata").finish());
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

    /// Feeds `bytes`, and returns the sum that goes on from there.
    #[must_use]
    pub fn update(self, bytes: &[u8]) -> Self {
        let (mut a, mut b) = (self.a, self.b);
        for block in bytes.chunks(BLOCK) {
            // Each lane sums every 16th byte, as Adler-32 sums them all,
            // independently, which compilers vectorize; the lanes'
            // arithmetic, though wrapping, never wraps within a block.
            let (groups, rest) = block.as_chunks::<LANES>();
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
            // And the last few bytes, a byte at a time.
            for &byte in rest {
                a = a.wrapping_add(u32::from(byte));
                b = b.wrapping_add(a);
            }
            a = a.wrapping_rem(BASE);
            b = b.wrapping_rem(BASE);
        }
        Self { a, b }
    }

    /// Returns the Adler-32 of the bytes fed: the second sum above the
    /// first.
    #[must_use]
    pub const fn finish(self) -> u32 {
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
    pub const fn state(self) -> u32 {
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
    Adler32::new().update(bytes).finish()
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
    fn defined(bytes: &[u8]) -> u32 {
        let (mut a, mut b) = (1_u32, 0_u32);
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

    #[test]
    fn matches_the_definition() {
        assert_eq!(adler32(b"123456789"), Adler32::CHECK);
        assert_eq!(adler32(b"123456789"), defined(b"123456789"));
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
        assert_eq!(adler32(b""), 1);
        // Every length, around the reductions, at the sums' largest.
        let ones = [0xFF_u8; 2 * BLOCK + 40];
        let mixed: [u8; 2 * BLOCK + 40] = core::array::from_fn(|n| (n * 37 % 251) as u8);
        for bytes in [&ones, &mixed] {
            let ends = [0..40, BLOCK - 20..BLOCK + 20, bytes.len() - 20..bytes.len()];
            for len in ends.into_iter().flatten() {
                assert_eq!(adler32(&bytes[..len]), defined(&bytes[..len]), "{len}");
            }
            // In pieces, from any state.
            for piece in [1, 7, 16, 17, 5552, BLOCK - 1, BLOCK, BLOCK + 1] {
                let sum = bytes.chunks(piece).fold(Adler32::new(), Adler32::update);
                assert_eq!(sum.finish(), defined(bytes), "{piece}");
                let resumed = bytes.chunks(piece).fold(Adler32::new(), |sum, chunk| {
                    Adler32::from_state(sum.state()).update(chunk)
                });
                assert_eq!(resumed, sum, "{piece}");
            }
        }
    }
}

// This is free and unencumbered software released into the public domain.

//! SHA-256, written clean room from FIPS 180-4, and dedicated to the public
//! domain: the hash that xz and 7z may check data with, over bytes fed in
//! pieces, as they arrive.
//!
//! It is pure, safe Rust without dependencies, and builds with
//! `#![no_std]`, without an allocator. It checks what is stored, as an
//! archive's checks do; it makes no claim against attackers, such as
//! taking the same time whatever the bytes, which a cryptographic library
//! would. The `std` feature, enabled by default, adds nothing by itself.
//!
//! The `intrinsics` feature, enabled by default (D66), takes the blocks by
//! the CPU's SHA-256 instructions where it has them: x86-64's SHA
//! extensions, and ARMv8's SHA2, which every Apple Silicon has. With
//! `std`, they are chosen at run time; without it, only where the build
//! enables them. It depends on `compute-multiversion`, a fork of
//! `multiversion` that lets their intrinsics be called without `unsafe`
//! code here. Without it, as `default-features = false, features =
//! ["all", "std"]` builds it, it has no dependencies, all of it public
//! domain, and is 6.2 times slower on Apple M4, 459 MB/s, not 2,859, and
//! 7.7 times on x86-64 (Zen 1), 159 MB/s, not 1,228 (2026-11-08); and 7.5
//! times on Neoverse V3, 0.28 GB/s, not 2.10 (2026-11-11).
//!
//! It is one of the `compute-*` crates: clean-room, public-domain
//! checksums and hashes, each of the same shape: `new`, `update`, `chain`,
//! and `finish`.
//!
//! # Examples
//!
//! ```
//! use compute_sha256::{Sha256, sha256};
//!
//! let mut sha = Sha256::new();
//! sha.update(b"a");
//! sha.update(b"bc");
//! assert_eq!(sha.finish(), sha256(b"abc"));
//! assert_eq!(sha256(b"abc")[..4], [0xBA, 0x78, 0x16, 0xBF]);
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

/// SHA-256's round constants (FIPS 180-4 §4.2.2): the first 32 bits of
/// the fractional parts of the cube roots of the first 64 primes.
const K: [u32; 64] = [
    0x428A_2F98,
    0x7137_4491,
    0xB5C0_FBCF,
    0xE9B5_DBA5,
    0x3956_C25B,
    0x59F1_11F1,
    0x923F_82A4,
    0xAB1C_5ED5,
    0xD807_AA98,
    0x1283_5B01,
    0x2431_85BE,
    0x550C_7DC3,
    0x72BE_5D74,
    0x80DE_B1FE,
    0x9BDC_06A7,
    0xC19B_F174,
    0xE49B_69C1,
    0xEFBE_4786,
    0x0FC1_9DC6,
    0x240C_A1CC,
    0x2DE9_2C6F,
    0x4A74_84AA,
    0x5CB0_A9DC,
    0x76F9_88DA,
    0x983E_5152,
    0xA831_C66D,
    0xB003_27C8,
    0xBF59_7FC7,
    0xC6E0_0BF3,
    0xD5A7_9147,
    0x06CA_6351,
    0x1429_2967,
    0x27B7_0A85,
    0x2E1B_2138,
    0x4D2C_6DFC,
    0x5338_0D13,
    0x650A_7354,
    0x766A_0ABB,
    0x81C2_C92E,
    0x9272_2C85,
    0xA2BF_E8A1,
    0xA81A_664B,
    0xC24B_8B70,
    0xC76C_51A3,
    0xD192_E819,
    0xD699_0624,
    0xF40E_3585,
    0x106A_A070,
    0x19A4_C116,
    0x1E37_6C08,
    0x2748_774C,
    0x34B0_BCB5,
    0x391C_0CB3,
    0x4ED8_AA4A,
    0x5B9C_CA4F,
    0x682E_6FF3,
    0x748F_82EE,
    0x78A5_636F,
    0x84C8_7814,
    0x8CC7_0208,
    0x90BE_FFFA,
    0xA450_6CEB,
    0xBEF9_A3F7,
    0xC671_78F2,
];

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

/// SHA-256's initial hash (FIPS 180-4 §5.3.3): the first 32 bits of the
/// fractional parts of the square roots of the first 8 primes.
const H0: [u32; 8] = [
    0x6A09_E667,
    0xBB67_AE85,
    0x3C6E_F372,
    0xA54F_F53A,
    0x510E_527F,
    0x9B05_688C,
    0x1F83_D9AB,
    0x5BE0_CD19,
];

/// A SHA-256 computed over bytes fed in pieces.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sha256 {
    hash: [u32; 8],
    /// The bytes of a block not yet whole.
    block: [u8; 64],
    held: usize,
    /// How many bytes have been fed, in all.
    len: u64,
}

impl Sha256 {
    /// Starts a SHA-256 over no bytes.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            hash: H0,
            block: [0; 64],
            held: 0,
            len: 0,
        }
    }

    /// Feeds `bytes`.
    pub fn update(&mut self, mut bytes: &[u8]) {
        self.len = self
            .len
            .wrapping_add(u64::try_from(bytes.len()).unwrap_or(u64::MAX));
        if self.held > 0 {
            let n = bytes.len().min(64_usize.saturating_sub(self.held));
            let (head, rest) = bytes.split_at(n);
            if let Some(to) = self.block.get_mut(self.held..self.held.saturating_add(n)) {
                to.copy_from_slice(head);
            }
            self.held = self.held.saturating_add(n);
            bytes = rest;
            if self.held < 64 {
                return;
            }
            let block = self.block;
            self.compress_all(core::slice::from_ref(&block));
            self.held = 0;
        }
        let (blocks, rest) = bytes.as_chunks::<64>();
        self.compress_all(blocks);
        if let Some(to) = self.block.get_mut(..rest.len()) {
            to.copy_from_slice(rest);
        }
        self.held = rest.len();
    }

    /// Feeds `bytes`, and returns the hash that goes on from there, so
    /// that calls chain.
    #[must_use]
    pub fn chain(mut self, bytes: &[u8]) -> Self {
        self.update(bytes);
        self
    }

    /// Returns the SHA-256 of the bytes fed, which it may go on from.
    #[must_use]
    pub fn finish(&self) -> [u8; 32] {
        self.clone().pad()
    }

    /// Pads the bytes fed, and returns their hash.
    fn pad(mut self) -> [u8; 32] {
        // A one bit, zeros up to 56 bytes into a block, and the length in
        // bits, big-endian (§5.1.1).
        let bits = self.len.wrapping_mul(8);
        let zeros = 55_usize.wrapping_sub(self.held) & 63;
        self.update(&[0x80]);
        self.update([0; 64].get(..zeros).unwrap_or_default());
        self.update(&bits.to_be_bytes());
        let mut digest = [0; 32];
        for (to, word) in digest.as_chunks_mut::<4>().0.iter_mut().zip(self.hash) {
            *to = word.to_be_bytes();
        }
        digest
    }

    /// Takes `blocks` into the hash: by the CPU's instructions, if it has
    /// them and `intrinsics` is enabled, or else by its rounds.
    fn compress_all(&mut self, blocks: &[[u8; 64]]) {
        #[cfg(all(
            feature = "intrinsics",
            any(
                all(target_arch = "x86_64", target_feature = "sse2"),
                all(target_arch = "aarch64", target_feature = "neon")
            )
        ))]
        if intrinsics::compress(&mut self.hash, blocks) {
            return;
        }
        for block in blocks {
            self.compress(block);
        }
    }

    /// Takes a block into the hash (§6.2.2).
    fn compress(&mut self, block: &[u8; 64]) {
        let mut w = [0_u32; 64];
        for (word, bytes) in w.iter_mut().zip(block.as_chunks::<4>().0) {
            *word = u32::from_be_bytes(*bytes);
        }
        for t in 16..64_usize {
            let at = |back: usize| w.get(t.wrapping_sub(back)).copied().unwrap_or_default();
            let (w15, w2) = (at(15), at(2));
            let s0 = w15.rotate_right(7) ^ w15.rotate_right(18) ^ (w15 >> 3);
            let s1 = w2.rotate_right(17) ^ w2.rotate_right(19) ^ (w2 >> 10);
            let word = at(16).wrapping_add(s0).wrapping_add(at(7)).wrapping_add(s1);
            if let Some(slot) = w.get_mut(t) {
                *slot = word;
            }
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = self.hash;
        for (&k, &w) in K.iter().zip(&w) {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(k)
                .wrapping_add(w);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            (h, g, f, e) = (g, f, e, d.wrapping_add(t1));
            (d, c, b, a) = (c, b, a, t1.wrapping_add(t2));
        }
        for (hash, word) in self.hash.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *hash = hash.wrapping_add(word);
        }
    }
}

impl Default for Sha256 {
    fn default() -> Self {
        Self::new()
    }
}

/// Returns the SHA-256 of `bytes`.
#[must_use]
pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::new().chain(bytes).finish()
}

// The README's example runs as a doctest.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

#[cfg(test)]
mod tests {
    use super::*;

    /// Returns `digest` in hexadecimal.
    fn hex(digest: [u8; 32]) -> [u8; 64] {
        let digits = b"0123456789abcdef";
        let mut hex = [0; 64];
        for (pair, byte) in hex.as_chunks_mut::<2>().0.iter_mut().zip(digest) {
            *pair = [
                digits[usize::from(byte >> 4)],
                digits[usize::from(byte & 15)],
            ];
        }
        hex
    }

    /// FIPS 180-4's examples, and the NIST test vectors.
    #[test]
    fn hashes_as_fips_180_4_does() {
        let cases: [(&[u8], &[u8; 64]); 3] = [
            (
                b"",
                b"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            ),
            (
                b"abc",
                b"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            ),
            (
                b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
                b"248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1",
            ),
        ];
        for (bytes, expected) in cases {
            assert_eq!(&hex(sha256(bytes)), expected);
        }
        let mut million = Sha256::new();
        for _ in 0..1000 {
            million.update(&[b'a'; 1000]);
        }
        assert_eq!(
            &hex(million.finish()),
            b"cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
    }

    /// The CPU's instructions, where it has them, take blocks into any
    /// hash as the rounds do.
    #[cfg(all(
        feature = "intrinsics",
        any(
            all(target_arch = "x86_64", target_feature = "sse2"),
            all(target_arch = "aarch64", target_feature = "neon")
        )
    ))]
    #[test]
    fn instructions_match_the_rounds() {
        let bytes: [u8; 640] = core::array::from_fn(|n| (n * 131 % 251) as u8);
        let blocks = bytes.as_chunks::<64>().0;
        for start in [H0, [0; 8], core::array::from_fn(|n| 0x0123_4567 * n as u32)] {
            for count in 0..=blocks.len() {
                let mut rounds = Sha256 {
                    hash: start,
                    ..Sha256::new()
                };
                for block in &blocks[..count] {
                    rounds.compress(block);
                }
                let mut hash = start;
                if intrinsics::compress(&mut hash, &blocks[..count]) {
                    assert_eq!(hash, rounds.hash, "{count} blocks");
                }
            }
        }
    }

    #[test]
    fn hashes_the_same_in_pieces() {
        let bytes: [u8; 300] = core::array::from_fn(|n| (n * 37 % 251) as u8);
        for len in [0, 1, 55, 56, 63, 64, 65, 119, 120, 128, 300] {
            let whole = sha256(&bytes[..len]);
            for piece in [1, 7, 63, 64, 65] {
                let mut sha = Sha256::new();
                for chunk in bytes[..len].chunks(piece) {
                    sha.update(chunk);
                }
                assert_eq!(sha.finish(), whole, "{len} in pieces of {piece}");
                // Finishing leaves it as it was.
                assert_eq!(sha.finish(), whole);
            }
        }
    }
}

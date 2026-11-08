// This is free and unencumbered software released into the public domain.

//! SHA-512 and its truncation SHA-512/256, written clean room from FIPS
//! 180-4, and dedicated to the public domain: the hashes that ZFS may stamp
//! on its blocks, among others, over bytes fed in pieces, as they arrive.
//!
//! It is pure, safe Rust without dependencies, and builds with
//! `#![no_std]`, without an allocator. It checks what is stored, as a file
//! system's checks do; it makes no claim against attackers, such as taking
//! the same time whatever the bytes, which a cryptographic library would.
//! The `std` feature, enabled by default, adds nothing by itself. Its
//! constants are computed from their definitions in FIPS 180-4: the round
//! constants and SHA-512's initial hash from the primes' roots, and
//! SHA-512/256's by its generation function.
//!
//! The `intrinsics` feature, enabled by default (D66), takes the blocks by
//! ARMv8.2's SHA-512 instructions where the CPU has them, as every Apple
//! Silicon does. With `std`, they are chosen at run time; without it, only
//! where the build enables them. It depends on `compute-multiversion`, a
//! fork of `multiversion` that lets their intrinsics be called without
//! `unsafe` code here. Without it, as `default-features = false, features
//! = ["all", "std"]` builds it, it has no dependencies, all of it public
//! domain, and is 2.6 times slower on Apple M4, 733 MB/s, not 1,900
//! (2026-11-08); on x86-64, whose instructions it does not take, no
//! slower.
//!
//! It is one of the `compute-*` crates: clean-room, public-domain
//! checksums and hashes, each of the same shape: `new`, `update`, `chain`,
//! and `finish`.
//!
//! # Examples
//!
//! ```
//! use compute_sha512::{Sha512, Sha512_256, sha512, sha512_256};
//!
//! let mut sha = Sha512::new();
//! sha.update(b"a");
//! sha.update(b"bc");
//! assert_eq!(sha.finish(), sha512(b"abc"));
//! assert_eq!(sha512(b"abc")[..4], [0xDD, 0xAF, 0x35, 0xA1]);
//! let sha = Sha512_256::new().chain(b"a").chain(b"bc");
//! assert_eq!(sha.finish(), sha512_256(b"abc"));
//! assert_eq!(sha512_256(b"abc")[..4], [0x53, 0x04, 0x8E, 0x26]);
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

#[cfg(feature = "intrinsics")]
mod intrinsics;

/// SHA-512's round constants (FIPS 180-4 §4.2.3): the first 64 bits of
/// the fractional parts of the cube roots of the first 80 primes.
const K: [u64; 80] = [
    0x428A_2F98_D728_AE22,
    0x7137_4491_23EF_65CD,
    0xB5C0_FBCF_EC4D_3B2F,
    0xE9B5_DBA5_8189_DBBC,
    0x3956_C25B_F348_B538,
    0x59F1_11F1_B605_D019,
    0x923F_82A4_AF19_4F9B,
    0xAB1C_5ED5_DA6D_8118,
    0xD807_AA98_A303_0242,
    0x1283_5B01_4570_6FBE,
    0x2431_85BE_4EE4_B28C,
    0x550C_7DC3_D5FF_B4E2,
    0x72BE_5D74_F27B_896F,
    0x80DE_B1FE_3B16_96B1,
    0x9BDC_06A7_25C7_1235,
    0xC19B_F174_CF69_2694,
    0xE49B_69C1_9EF1_4AD2,
    0xEFBE_4786_384F_25E3,
    0x0FC1_9DC6_8B8C_D5B5,
    0x240C_A1CC_77AC_9C65,
    0x2DE9_2C6F_592B_0275,
    0x4A74_84AA_6EA6_E483,
    0x5CB0_A9DC_BD41_FBD4,
    0x76F9_88DA_8311_53B5,
    0x983E_5152_EE66_DFAB,
    0xA831_C66D_2DB4_3210,
    0xB003_27C8_98FB_213F,
    0xBF59_7FC7_BEEF_0EE4,
    0xC6E0_0BF3_3DA8_8FC2,
    0xD5A7_9147_930A_A725,
    0x06CA_6351_E003_826F,
    0x1429_2967_0A0E_6E70,
    0x27B7_0A85_46D2_2FFC,
    0x2E1B_2138_5C26_C926,
    0x4D2C_6DFC_5AC4_2AED,
    0x5338_0D13_9D95_B3DF,
    0x650A_7354_8BAF_63DE,
    0x766A_0ABB_3C77_B2A8,
    0x81C2_C92E_47ED_AEE6,
    0x9272_2C85_1482_353B,
    0xA2BF_E8A1_4CF1_0364,
    0xA81A_664B_BC42_3001,
    0xC24B_8B70_D0F8_9791,
    0xC76C_51A3_0654_BE30,
    0xD192_E819_D6EF_5218,
    0xD699_0624_5565_A910,
    0xF40E_3585_5771_202A,
    0x106A_A070_32BB_D1B8,
    0x19A4_C116_B8D2_D0C8,
    0x1E37_6C08_5141_AB53,
    0x2748_774C_DF8E_EB99,
    0x34B0_BCB5_E19B_48A8,
    0x391C_0CB3_C5C9_5A63,
    0x4ED8_AA4A_E341_8ACB,
    0x5B9C_CA4F_7763_E373,
    0x682E_6FF3_D6B2_B8A3,
    0x748F_82EE_5DEF_B2FC,
    0x78A5_636F_4317_2F60,
    0x84C8_7814_A1F0_AB72,
    0x8CC7_0208_1A64_39EC,
    0x90BE_FFFA_2363_1E28,
    0xA450_6CEB_DE82_BDE9,
    0xBEF9_A3F7_B2C6_7915,
    0xC671_78F2_E372_532B,
    0xCA27_3ECE_EA26_619C,
    0xD186_B8C7_21C0_C207,
    0xEADA_7DD6_CDE0_EB1E,
    0xF57D_4F7F_EE6E_D178,
    0x06F0_67AA_7217_6FBA,
    0x0A63_7DC5_A2C8_98A6,
    0x113F_9804_BEF9_0DAE,
    0x1B71_0B35_131C_471B,
    0x28DB_77F5_2304_7D84,
    0x32CA_AB7B_40C7_2493,
    0x3C9E_BE0A_15C9_BEBC,
    0x431D_67C4_9C10_0D4C,
    0x4CC5_D4BE_CB3E_42B6,
    0x597F_299C_FC65_7E2A,
    0x5FCB_6FAB_3AD6_FAEC,
    0x6C44_198C_4A47_5817,
];

/// SHA-512's initial hash (FIPS 180-4 §5.3.5): the first 64 bits of the
/// fractional parts of the square roots of the first 8 primes.
const H0: [u64; 8] = [
    0x6A09_E667_F3BC_C908,
    0xBB67_AE85_84CA_A73B,
    0x3C6E_F372_FE94_F82B,
    0xA54F_F53A_5F1D_36F1,
    0x510E_527F_ADE6_82D1,
    0x9B05_688C_2B3E_6C1F,
    0x1F83_D9AB_FB41_BD6B,
    0x5BE0_CD19_137E_2179,
];

/// SHA-512/256's initial hash (FIPS 180-4 §5.3.6.2), by the generation
/// function of §5.3.6: SHA-512 of `SHA-512/256` from [`H0`], each word of
/// it exclusive-ored with `0xA5A5A5A5A5A5A5A5`.
const H0_256: [u64; 8] = [
    0x2231_2194_FC2B_F72C,
    0x9F55_5FA3_C84C_64C2,
    0x2393_B86B_6F53_B151,
    0x9638_7719_5940_EABD,
    0x9628_3EE2_A88E_FFE3,
    0xBE5E_1E25_5386_3992,
    0x2B01_99FC_2C85_B8AA,
    0x0EB7_2DDC_81C5_2CA2,
];

/// The state the two hashes share: the hash so far, and what is fed of a
/// block not yet whole.
#[derive(Clone, Debug, PartialEq, Eq)]
struct State {
    hash: [u64; 8],
    /// The bytes of a block not yet whole.
    block: [u8; 128],
    held: usize,
    /// How many bytes have been fed, in all.
    len: u128,
}

impl State {
    const fn new(hash: [u64; 8]) -> Self {
        Self {
            hash,
            block: [0; 128],
            held: 0,
            len: 0,
        }
    }

    fn update(&mut self, mut bytes: &[u8]) {
        self.len = self
            .len
            .wrapping_add(u128::try_from(bytes.len()).unwrap_or(u128::MAX));
        if self.held > 0 {
            let n = bytes.len().min(128_usize.saturating_sub(self.held));
            let (head, rest) = bytes.split_at(n);
            if let Some(to) = self.block.get_mut(self.held..self.held.saturating_add(n)) {
                to.copy_from_slice(head);
            }
            self.held = self.held.saturating_add(n);
            bytes = rest;
            if self.held < 128 {
                return;
            }
            let block = self.block;
            self.compress_all(core::slice::from_ref(&block));
            self.held = 0;
        }
        let (blocks, rest) = bytes.as_chunks::<128>();
        self.compress_all(blocks);
        if let Some(to) = self.block.get_mut(..rest.len()) {
            to.copy_from_slice(rest);
        }
        self.held = rest.len();
    }

    /// Pads the bytes fed, and returns the hash: a one bit, zeros up to
    /// 112 bytes into a block, and the length in bits, as 128 bits,
    /// big-endian (§5.1.2).
    fn pad(mut self) -> [u64; 8] {
        let bits = self.len.wrapping_mul(8);
        let zeros = 111_usize.wrapping_sub(self.held) & 127;
        self.update(&[0x80]);
        self.update([0; 128].get(..zeros).unwrap_or_default());
        self.update(&bits.to_be_bytes());
        self.hash
    }

    /// Takes `blocks` into the hash: by the CPU's instructions, if it has
    /// them and `intrinsics` is enabled, or else by its rounds.
    fn compress_all(&mut self, blocks: &[[u8; 128]]) {
        #[cfg(feature = "intrinsics")]
        if intrinsics::compress(&mut self.hash, blocks) {
            return;
        }
        for block in blocks {
            self.compress(block);
        }
    }

    /// Takes a block into the hash (§6.4.2).
    fn compress(&mut self, block: &[u8; 128]) {
        let mut w = [0_u64; 80];
        for (word, bytes) in w.iter_mut().zip(block.as_chunks::<8>().0) {
            *word = u64::from_be_bytes(*bytes);
        }
        for t in 16..80_usize {
            let at = |back: usize| w.get(t.wrapping_sub(back)).copied().unwrap_or_default();
            let (w15, w2) = (at(15), at(2));
            let s0 = w15.rotate_right(1) ^ w15.rotate_right(8) ^ (w15 >> 7);
            let s1 = w2.rotate_right(19) ^ w2.rotate_right(61) ^ (w2 >> 6);
            let word = at(16).wrapping_add(s0).wrapping_add(at(7)).wrapping_add(s1);
            if let Some(slot) = w.get_mut(t) {
                *slot = word;
            }
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = self.hash;
        for (&k, &w) in K.iter().zip(&w) {
            let s1 = e.rotate_right(14) ^ e.rotate_right(18) ^ e.rotate_right(41);
            let ch = (e & f) ^ (!e & g);
            let t1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(k)
                .wrapping_add(w);
            let s0 = a.rotate_right(28) ^ a.rotate_right(34) ^ a.rotate_right(39);
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

/// Returns the first `N` bytes of `hash`, big-endian.
fn digest<const N: usize>(hash: [u64; 8]) -> [u8; N] {
    let mut digest = [0; N];
    let words = hash.map(u64::to_be_bytes);
    for (to, from) in digest.iter_mut().zip(words.as_flattened()) {
        *to = *from;
    }
    digest
}

/// A SHA-512 computed over bytes fed in pieces.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sha512(State);

impl Sha512 {
    /// Starts a SHA-512 over no bytes.
    #[must_use]
    pub const fn new() -> Self {
        Self(State::new(H0))
    }

    /// Feeds `bytes`.
    pub fn update(&mut self, bytes: &[u8]) {
        self.0.update(bytes);
    }

    /// Feeds `bytes`, and returns the hash that goes on from there, so
    /// that calls chain.
    #[must_use]
    pub fn chain(mut self, bytes: &[u8]) -> Self {
        self.update(bytes);
        self
    }

    /// Returns the SHA-512 of the bytes fed, which it may go on from.
    #[must_use]
    pub fn finish(&self) -> [u8; 64] {
        digest(self.0.clone().pad())
    }
}

impl Default for Sha512 {
    fn default() -> Self {
        Self::new()
    }
}

/// A SHA-512/256 computed over bytes fed in pieces: SHA-512 from its own
/// initial hash, its digest cut to 256 bits, as ZFS stamps it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sha512_256(State);

impl Sha512_256 {
    /// Starts a SHA-512/256 over no bytes.
    #[must_use]
    pub const fn new() -> Self {
        Self(State::new(H0_256))
    }

    /// Feeds `bytes`.
    pub fn update(&mut self, bytes: &[u8]) {
        self.0.update(bytes);
    }

    /// Feeds `bytes`, and returns the hash that goes on from there, so
    /// that calls chain.
    #[must_use]
    pub fn chain(mut self, bytes: &[u8]) -> Self {
        self.update(bytes);
        self
    }

    /// Returns the SHA-512/256 of the bytes fed, which it may go on from.
    #[must_use]
    pub fn finish(&self) -> [u8; 32] {
        digest(self.0.clone().pad())
    }
}

impl Default for Sha512_256 {
    fn default() -> Self {
        Self::new()
    }
}

/// Returns the SHA-512 of `bytes`.
#[must_use]
pub fn sha512(bytes: &[u8]) -> [u8; 64] {
    Sha512::new().chain(bytes).finish()
}

/// Returns the SHA-512/256 of `bytes`.
#[must_use]
pub fn sha512_256(bytes: &[u8]) -> [u8; 32] {
    Sha512_256::new().chain(bytes).finish()
}

// The README's example runs as a doctest.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

#[cfg(test)]
mod tests {
    use super::*;

    /// Returns `bytes` in hexadecimal.
    fn hex<const N: usize>(bytes: [u8; N]) -> [u8; 128] {
        let digits = b"0123456789abcdef";
        let mut hex = [0; 128];
        for (pair, byte) in hex.as_chunks_mut::<2>().0.iter_mut().zip(bytes) {
            *pair = [
                digits[usize::from(byte >> 4)],
                digits[usize::from(byte & 15)],
            ];
        }
        hex
    }

    /// FIPS 180-4's examples, and a million `a`s: SHA-512's digest, and
    /// SHA-512/256's.
    #[test]
    fn hashes_as_fips_180_4_does() {
        let cases: [(&[u8], &str, &str); 3] = [
            (
                b"",
                "cf83e1357eefb8bdf1542850d66d8007d620e4050b5715dc83f4a921d36ce9ce\
                 47d0d13c5d85f2b0ff8318d2877eec2f63b931bd47417a81a538327af927da3e",
                "c672b8d1ef56ed28ab87c3622c5114069bdd3ad7b8f9737498d0c01ecef0967a",
            ),
            (
                b"abc",
                "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a\
                 2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f",
                "53048e2681941ef99b2e29b76b4c7dabe4c2d0c634fc6d46e0e2f13107e7af23",
            ),
            (
                b"abcdefghbcdefghicdefghijdefghijkefghijklfghijklmghijklmnhijklmno\
                  ijklmnopjklmnopqklmnopqrlmnopqrsmnopqrstnopqrstu",
                "8e959b75dae313da8cf4f72814fc143f8f7779c6eb9f7fa17299aeadb6889018\
                 501d289e4900f7e4331b99dec4b5433ac7d329eeb6dd26545e96e55b874be909",
                "3928e184fb8690f840da3988121d31be65cb9d3ef83ee6146feac861e19b563a",
            ),
        ];
        for (bytes, expected, expected_256) in cases {
            assert_eq!(&hex(sha512(bytes))[..], expected.as_bytes());
            assert_eq!(&hex(sha512_256(bytes))[..64], expected_256.as_bytes());
        }
        let (mut million, mut million_256) = (Sha512::new(), Sha512_256::new());
        for _ in 0..1000 {
            million.update(&[b'a'; 1000]);
            million_256.update(&[b'a'; 1000]);
        }
        assert_eq!(
            &hex(million.finish())[..],
            &b"e718483d0ce769644e2e42c7bc15b4638e1f98b13b2044285632a803afa973eb\
               de0ff244877ea60a4cb0432ce577c31beb009c5c2c49aa2e4eadb217ad8cc09b"[..]
        );
        assert_eq!(
            &hex(million_256.finish())[..64],
            &b"9a59a052930187a97038cae692f30708aa6491923ef5194394dc68d56c74fb21"[..]
        );
    }

    /// The CPU's instructions, where it has them, take blocks into any
    /// hash as the rounds do.
    #[cfg(feature = "intrinsics")]
    #[test]
    fn instructions_match_the_rounds() {
        let bytes: [u8; 1280] = core::array::from_fn(|n| (n * 131 % 251) as u8);
        let blocks = bytes.as_chunks::<128>().0;
        let mixed = core::array::from_fn(|n| 0x0123_4567_89AB_CDEF * (n as u64 + 1));
        for start in [H0, H0_256, [0; 8], mixed] {
            for count in 0..=blocks.len() {
                let mut rounds = State::new(start);
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
        let bytes: [u8; 600] = core::array::from_fn(|n| (n * 37 % 251) as u8);
        for len in [0, 1, 111, 112, 127, 128, 129, 239, 240, 256, 600] {
            let (whole, whole_256) = (sha512(&bytes[..len]), sha512_256(&bytes[..len]));
            for piece in [1, 7, 127, 128, 129] {
                let (mut sha, mut sha_256) = (Sha512::new(), Sha512_256::new());
                for chunk in bytes[..len].chunks(piece) {
                    sha.update(chunk);
                    sha_256 = sha_256.chain(chunk);
                }
                assert_eq!(sha.finish(), whole, "{len} in pieces of {piece}");
                assert_eq!(sha_256.finish(), whole_256, "{len} in pieces of {piece}");
                // Finishing leaves it as it was.
                assert_eq!(sha.finish(), whole);
            }
        }
    }
}

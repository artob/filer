// This is free and unencumbered software released into the public domain.

//! Fletcher's checksums as file systems stamp them, written clean room
//! from their definitions, and dedicated to the public domain: each
//! variant a type and a feature of its own, all enabled by `all`.
//!
//! | Type | Checksum | Feature | Stamped by |
//! | --- | --- | --- | --- |
//! | [`Fletcher4`] | Fletcher-4 | `fletcher4` | ZFS, its default |
//! | [`Fletcher2`] | Fletcher-2 | `fletcher2` | ZFS, on older pools |
//! | [`Fletcher64`] | Fletcher-64 | `fletcher64` | APFS, on every object |
//!
//! Each computes over bytes fed in pieces, as they arrive, by `update`, or
//! by `chain`, which returns the checksum that goes on from there; a
//! function of each, such as [`fletcher4`], computes it over one slice.
//! They read whole words, and leave out bytes past the last, as no block
//! that they check holds any. ZFS's read the words in the byte order of
//! the CPU that wrote the block, little-endian by `new`, big-endian by
//! `new_big_endian`; APFS's are little-endian.
//!
//! Neither ZFS's specification nor APFS's reference defines them beyond
//! their names, so each was established by observation, checked against
//! what ZFS 2.4.1 and macOS 26 stamp: Fletcher-4's four 64-bit sums of
//! 32-bit words, each summing the one before; Fletcher-2's two pairs of
//! 64-bit sums of 64-bit words; and Fletcher-64's two sums of 32-bit words
//! modulo 2³² − 1, stored as Fletcher's check values, which bring both
//! sums to zero. No implementation's code is copied. It is pure, safe Rust
//! without dependencies, and builds with `#![no_std]`, without an
//! allocator. The `std` feature, enabled by default, adds nothing by
//! itself.
//!
//! Fletcher-4 runs in four lanes, each over every fourth word, folded into
//! the whole every few thousand words, so that the sums of each lane are
//! independent and may run side by side.
//!
//! It is one of the `compute-*` crates: clean-room, public-domain
//! checksums and hashes, each of the same shape: `new`, `update`, `chain`,
//! and `finish`.
//!
//! # Examples
//!
//! ```
//! # #[cfg(feature = "fletcher4")] {
//! use compute_fletcher::{Fletcher4, fletcher4};
//!
//! let bytes = [1, 0, 0, 0, 2, 0, 0, 0];
//! assert_eq!(fletcher4(&bytes), [3, 4, 5, 6]);
//! let sum = Fletcher4::new().chain(&bytes[..3]).chain(&bytes[3..]);
//! assert_eq!(sum.finish(), fletcher4(&bytes));
//! # }
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

/// The bytes of a word not yet whole, of `N` bytes.
#[cfg(any(feature = "fletcher2", feature = "fletcher4", feature = "fletcher64"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Partial<const N: usize> {
    bytes: [u8; N],
    held: usize,
}

#[cfg(any(feature = "fletcher2", feature = "fletcher4", feature = "fletcher64"))]
impl<const N: usize> Partial<N> {
    const fn new() -> Self {
        Self {
            bytes: [0; N],
            held: 0,
        }
    }

    /// Feeds `bytes`, and calls `words` with the whole words they make,
    /// first any that completes the one held.
    fn feed(&mut self, mut bytes: &[u8], mut words: impl FnMut(&[[u8; N]])) {
        if self.held > 0 {
            let n = bytes.len().min(N.saturating_sub(self.held));
            let (head, rest) = bytes.split_at(n);
            if let Some(to) = self.bytes.get_mut(self.held..self.held.saturating_add(n)) {
                to.copy_from_slice(head);
            }
            self.held = self.held.saturating_add(n);
            bytes = rest;
            if self.held < N {
                return;
            }
            words(core::slice::from_ref(&self.bytes));
            self.held = 0;
        }
        let (whole, rest) = bytes.as_chunks::<N>();
        words(whole);
        if let Some(to) = self.bytes.get_mut(..rest.len()) {
            to.copy_from_slice(rest);
        }
        self.held = rest.len();
    }
}

/// The groups of four words folded into the whole at a time: few enough
/// that `n (n + 1) (n + 2)` of their words cannot overflow.
#[cfg(feature = "fletcher4")]
const BATCH: usize = 4096;

/// How the sums of lane `j` of four, each over every fourth word from the
/// `j`th, make the sums of all the words: `whole[r] = Σ COMBINE[j][r][s]
/// lane[s]`, wrapping. The weights of a lane's word are 1, `k`, `C(k+1, 2)`,
/// and `C(k+2, 3)`, `k` the lane's words from it to the end; the whole's,
/// 1, `4k − j`, `C(4k − j + 1, 2)`, and `C(4k − j + 2, 3)`; each of the
/// latter is the combination of the former that these rows give.
#[cfg(feature = "fletcher4")]
const COMBINE: [[[u64; 4]; 4]; 4] = {
    const fn neg(n: u64) -> u64 {
        0_u64.wrapping_sub(n)
    }
    [
        [
            [1, 0, 0, 0],
            [0, 4, 0, 0],
            [0, neg(6), 16, 0],
            [0, 4, neg(48), 64],
        ],
        [
            [1, 0, 0, 0],
            [neg(1), 4, 0, 0],
            [0, neg(10), 16, 0],
            [0, 10, neg(64), 64],
        ],
        [
            [1, 0, 0, 0],
            [neg(2), 4, 0, 0],
            [1, neg(14), 16, 0],
            [0, 20, neg(80), 64],
        ],
        [
            [1, 0, 0, 0],
            [neg(3), 4, 0, 0],
            [3, neg(18), 16, 0],
            [neg(1), 34, neg(96), 64],
        ],
    ]
};

/// ZFS's Fletcher-4: four 64-bit sums of 32-bit words, `a` of the words,
/// `b` of the `a`s, `c` of the `b`s, and `d` of the `c`s, each wrapping.
///
/// ```
/// use compute_fletcher::{Fletcher4, fletcher4};
///
/// let bytes = [1, 0, 0, 0, 2, 0, 0, 0];
/// assert_eq!(fletcher4(&bytes), [3, 4, 5, 6]);
/// let big = Fletcher4::new_big_endian().chain(&[0, 0, 0, 1, 0, 0, 0, 2]);
/// assert_eq!(big.finish(), [3, 4, 5, 6]);
/// ```
#[cfg(feature = "fletcher4")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Fletcher4 {
    sums: [u64; 4],
    partial: Partial<4>,
    big_endian: bool,
}

#[cfg(feature = "fletcher4")]
impl Fletcher4 {
    /// Starts a Fletcher-4 over no bytes, of little-endian words, as
    /// x86-64 and ARM write them.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            sums: [0; 4],
            partial: Partial::new(),
            big_endian: false,
        }
    }

    /// Starts a Fletcher-4 over no bytes, of big-endian words, as SPARC
    /// and POWER write them.
    #[must_use]
    pub const fn new_big_endian() -> Self {
        Self {
            big_endian: true,
            ..Self::new()
        }
    }

    /// Feeds `bytes`.
    pub fn update(&mut self, bytes: &[u8]) {
        let (sums, big_endian) = (&mut self.sums, self.big_endian);
        self.partial
            .feed(bytes, |words| feed4(sums, words, big_endian));
    }

    /// Feeds `bytes`, and returns the checksum that goes on from there, so
    /// that calls chain.
    #[must_use]
    pub fn chain(mut self, bytes: &[u8]) -> Self {
        self.update(bytes);
        self
    }

    /// Returns the Fletcher-4 of the words fed, `[a, b, c, d]`.
    #[must_use]
    pub const fn finish(&self) -> [u64; 4] {
        self.sums
    }
}

#[cfg(feature = "fletcher4")]
impl Default for Fletcher4 {
    fn default() -> Self {
        Self::new()
    }
}

/// Feeds whole `words` to Fletcher-4's `sums`: groups of four in lanes,
/// folded into the sums a batch at a time, and the rest one by one.
#[cfg(feature = "fletcher4")]
fn feed4(sums: &mut [u64; 4], words: &[[u8; 4]], big_endian: bool) {
    let word = |bytes: &[u8; 4]| {
        u64::from(match big_endian {
            true => u32::from_be_bytes(*bytes),
            false => u32::from_le_bytes(*bytes),
        })
    };
    let (groups, rest) = words.as_chunks::<4>();
    for batch in groups.chunks(BATCH) {
        // The sums of each lane, from zero.
        let (mut a, mut b, mut c, mut d) = ([0_u64; 4], [0_u64; 4], [0_u64; 4], [0_u64; 4]);
        for group in batch {
            let lanes = a.iter_mut().zip(&mut b).zip(&mut c).zip(&mut d);
            for ((((a, b), c), d), bytes) in lanes.zip(group) {
                *a = a.wrapping_add(word(bytes));
                *b = b.wrapping_add(*a);
                *c = c.wrapping_add(*b);
                *d = d.wrapping_add(*c);
            }
        }
        // The sums of the batch's words, from zero.
        let mut part = [0_u64; 4];
        for (j, combine) in COMBINE.iter().enumerate() {
            let lane = [a, b, c, d].map(|sums| sums.get(j).copied().unwrap_or_default());
            for (part, row) in part.iter_mut().zip(combine) {
                for (weight, sum) in row.iter().zip(lane) {
                    *part = part.wrapping_add(weight.wrapping_mul(sum));
                }
            }
        }
        let n = u64::try_from(batch.len())
            .unwrap_or_default()
            .wrapping_mul(4);
        extend(sums, part, n);
    }
    for bytes in rest {
        let [a, b, c, d] = sums;
        *a = a.wrapping_add(word(bytes));
        *b = b.wrapping_add(*a);
        *c = c.wrapping_add(*b);
        *d = d.wrapping_add(*c);
    }
}

/// Extends Fletcher-4's `sums` by `n` more words, whose sums from zero are
/// `part`: each word of the sums so far is `n` words further from the end,
/// so `b` gains `n a`, `c` gains `n b + C(n+1, 2) a`, and `d` gains `n c +
/// C(n+1, 2) b + C(n+2, 3) a`.
#[cfg(feature = "fletcher4")]
fn extend(sums: &mut [u64; 4], part: [u64; 4], n: u64) {
    // Exact: `n` is at most four times `BATCH`.
    let n1 = n.wrapping_mul(n.wrapping_add(1)) / 2;
    let n2 = n1.wrapping_mul(n.wrapping_add(2)) / 3;
    let [a, b, c, d] = *sums;
    let [pa, pb, pc, pd] = part;
    *sums = [
        a.wrapping_add(pa),
        b.wrapping_add(n.wrapping_mul(a)).wrapping_add(pb),
        c.wrapping_add(n.wrapping_mul(b))
            .wrapping_add(n1.wrapping_mul(a))
            .wrapping_add(pc),
        d.wrapping_add(n.wrapping_mul(c))
            .wrapping_add(n1.wrapping_mul(b))
            .wrapping_add(n2.wrapping_mul(a))
            .wrapping_add(pd),
    ];
}

/// Returns ZFS's Fletcher-4 of `bytes`, little-endian words.
#[cfg(feature = "fletcher4")]
#[must_use]
pub fn fletcher4(bytes: &[u8]) -> [u64; 4] {
    Fletcher4::new().chain(bytes).finish()
}

/// ZFS's Fletcher-2: two pairs of 64-bit sums of 64-bit words, taken in
/// pairs, `a0` and `a1` of the first and second of each, and `b0` and `b1`
/// of the `a`s, each wrapping. ZFS deprecates it, for new data, as its
/// sums lose the high bits of what they add.
///
/// ```
/// use compute_fletcher::fletcher2;
///
/// let bytes = [[1, 0, 0, 0, 0, 0, 0, 0], [2, 0, 0, 0, 0, 0, 0, 0]].repeat(2);
/// assert_eq!(fletcher2(bytes.as_flattened()), [2, 4, 3, 6]);
/// ```
#[cfg(feature = "fletcher2")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Fletcher2 {
    sums: [u64; 4],
    partial: Partial<16>,
    big_endian: bool,
}

#[cfg(feature = "fletcher2")]
impl Fletcher2 {
    /// Starts a Fletcher-2 over no bytes, of little-endian words, as
    /// x86-64 and ARM write them.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            sums: [0; 4],
            partial: Partial::new(),
            big_endian: false,
        }
    }

    /// Starts a Fletcher-2 over no bytes, of big-endian words, as SPARC
    /// and POWER write them.
    #[must_use]
    pub const fn new_big_endian() -> Self {
        Self {
            big_endian: true,
            ..Self::new()
        }
    }

    /// Feeds `bytes`.
    pub fn update(&mut self, bytes: &[u8]) {
        let (sums, big_endian) = (&mut self.sums, self.big_endian);
        self.partial.feed(bytes, |pairs| {
            let word = |bytes: [u8; 8]| match big_endian {
                true => u64::from_be_bytes(bytes),
                false => u64::from_le_bytes(bytes),
            };
            let [a0, a1, b0, b1] = sums;
            let (pairs, _) = pairs.as_flattened().as_chunks::<8>().0.as_chunks::<2>();
            for &[w0, w1] in pairs {
                let (w0, w1) = (word(w0), word(w1));
                *a0 = a0.wrapping_add(w0);
                *a1 = a1.wrapping_add(w1);
                *b0 = b0.wrapping_add(*a0);
                *b1 = b1.wrapping_add(*a1);
            }
        });
    }

    /// Feeds `bytes`, and returns the checksum that goes on from there, so
    /// that calls chain.
    #[must_use]
    pub fn chain(mut self, bytes: &[u8]) -> Self {
        self.update(bytes);
        self
    }

    /// Returns the Fletcher-2 of the words fed, `[a0, a1, b0, b1]`.
    #[must_use]
    pub const fn finish(&self) -> [u64; 4] {
        self.sums
    }
}

#[cfg(feature = "fletcher2")]
impl Default for Fletcher2 {
    fn default() -> Self {
        Self::new()
    }
}

/// Returns ZFS's Fletcher-2 of `bytes`, little-endian words.
#[cfg(feature = "fletcher2")]
#[must_use]
pub fn fletcher2(bytes: &[u8]) -> [u64; 4] {
    Fletcher2::new().chain(bytes).finish()
}

/// Fletcher-64's modulus, `2³² − 1`.
#[cfg(feature = "fletcher64")]
const MODULUS: u64 = 0xFFFF_FFFF;

/// The words Fletcher-64 sums before reducing its sums: from below the
/// modulus `M`, after `k` words `s1 < (k + 1) M` and `s2 < (1 + k (k + 3) /
/// 2) M`, which is below 2⁶⁴ for `k` up to 92,680.
#[cfg(feature = "fletcher64")]
const REDUCE: usize = 65_536;

/// APFS's Fletcher-64: two sums of little-endian 32-bit words modulo
/// `2³² − 1`, `s1` of the words and `s2` of the `s1`s, which it returns as
/// Fletcher's check values: those two words that, fed after the bytes,
/// would bring both sums to zero. APFS stores it in each object's first 8
/// bytes, over the rest.
///
/// ```
/// use compute_fletcher::{Fletcher64, fletcher64};
///
/// let object = [1, 0, 0, 0, 2, 0, 0, 0];
/// let check = fletcher64(&object);
/// assert_eq!(check, 0x0000_0004_FFFF_FFF8);
/// let whole = Fletcher64::new().chain(&object).chain(&check.to_le_bytes());
/// assert_eq!(whole.sums(), [0, 0]);
/// ```
#[cfg(feature = "fletcher64")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Fletcher64 {
    sums: [u64; 2],
    /// The words summed since the sums were last reduced.
    unreduced: usize,
    partial: Partial<4>,
}

#[cfg(feature = "fletcher64")]
impl Fletcher64 {
    /// Starts a Fletcher-64 over no bytes.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            sums: [0; 2],
            unreduced: 0,
            partial: Partial::new(),
        }
    }

    /// Feeds `bytes`.
    pub fn update(&mut self, bytes: &[u8]) {
        let (sums, unreduced) = (&mut self.sums, &mut self.unreduced);
        self.partial.feed(bytes, |words| {
            for words in words.chunks(REDUCE) {
                if unreduced.saturating_add(words.len()) > REDUCE {
                    *sums = reduce(*sums);
                    *unreduced = 0;
                }
                let [s1, s2] = sums;
                for word in words {
                    *s1 = s1.wrapping_add(u64::from(u32::from_le_bytes(*word)));
                    *s2 = s2.wrapping_add(*s1);
                }
                *unreduced = unreduced.saturating_add(words.len());
            }
        });
    }

    /// Feeds `bytes`, and returns the checksum that goes on from there, so
    /// that calls chain.
    #[must_use]
    pub fn chain(mut self, bytes: &[u8]) -> Self {
        self.update(bytes);
        self
    }

    /// Returns the two sums of the words fed, modulo `2³² − 1`: both zero
    /// over an object and its check values.
    #[must_use]
    pub fn sums(&self) -> [u64; 2] {
        reduce(self.sums)
    }

    /// Returns the Fletcher-64 of the words fed: the check values `c1`,
    /// in the low 32 bits, and `c2`, in the high.
    #[must_use]
    pub fn finish(&self) -> u64 {
        let [s1, s2] = self.sums();
        let c1 = MODULUS.wrapping_sub(s1.wrapping_add(s2) % MODULUS);
        let c2 = MODULUS.wrapping_sub(s1.wrapping_add(c1) % MODULUS);
        c2 << 32 | c1
    }
}

#[cfg(feature = "fletcher64")]
impl Default for Fletcher64 {
    fn default() -> Self {
        Self::new()
    }
}

/// Returns `sums` modulo `2³² − 1`.
#[cfg(feature = "fletcher64")]
const fn reduce([s1, s2]: [u64; 2]) -> [u64; 2] {
    [s1 % MODULUS, s2 % MODULUS]
}

/// Returns APFS's Fletcher-64 of `bytes`.
#[cfg(feature = "fletcher64")]
#[must_use]
pub fn fletcher64(bytes: &[u8]) -> u64 {
    Fletcher64::new().chain(bytes).finish()
}

// The README's example runs as a doctest, with the checksums it uses.
#[cfg(all(doctest, feature = "fletcher4", feature = "fletcher64"))]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    /// A pattern ZFS was given, `n · 37 mod 251`.
    #[allow(dead_code)]
    fn pattern(len: usize) -> [u8; 131_072] {
        let mut bytes = [0; 131_072];
        for (n, byte) in bytes.iter_mut().enumerate().take(len) {
            *byte = (n * 37 % 251) as u8;
        }
        bytes
    }

    /// Fletcher-4 one word at a time, as defined.
    #[cfg(feature = "fletcher4")]
    fn plain4(bytes: &[u8], big_endian: bool) -> [u64; 4] {
        let mut sums = [0_u64; 4];
        for word in bytes.as_chunks::<4>().0 {
            let word = match big_endian {
                true => u32::from_be_bytes(*word),
                false => u32::from_le_bytes(*word),
            };
            sums[0] = sums[0].wrapping_add(u64::from(word));
            sums[1] = sums[1].wrapping_add(sums[0]);
            sums[2] = sums[2].wrapping_add(sums[1]);
            sums[3] = sums[3].wrapping_add(sums[2]);
        }
        sums
    }

    /// What ZFS 2.4.1 stamped on blocks of the pattern, each the file it
    /// was written as, uncompressed: of 131,072 bytes and of 4,096.
    #[cfg(feature = "fletcher4")]
    #[test]
    fn sums_as_zfs_does() {
        assert_eq!(
            fletcher4(&pattern(131_072)),
            [
                0x0000_3EBD_F09B_FD27,
                0x0FAF_A731_EE5E_E629,
                0x4DAB_F77E_9DC1_CE94,
                0xC713_52D4_A663_DE10
            ]
        );
        assert_eq!(
            fletcher4(&pattern(131_072)[..4096]),
            [
                0x0000_01F5_E0ED_F817,
                0x0003_EBAC_82FF_583A,
                0x053B_D6DE_BB55_F2EB,
                0x3EC9_0629_4234_AAEF
            ]
        );
    }

    /// The lanes, folded a batch at a time, come to what one word at a
    /// time does, fed whole or in pieces, of either byte order, and past
    /// a batch, where the sums wrap.
    #[cfg(feature = "fletcher4")]
    #[test]
    fn lanes_sum_as_words_do() {
        let ones = [0xFF; 131_072];
        let bytes = pattern(131_072);
        for bytes in [&bytes[..], &ones[..]] {
            for len in (0..=80).chain([4096, 65_536, 65_540, 131_072]) {
                let bytes = &bytes[..len];
                for big_endian in [false, true] {
                    let start = match big_endian {
                        true => Fletcher4::new_big_endian(),
                        false => Fletcher4::new(),
                    };
                    let expected = plain4(bytes, big_endian);
                    assert_eq!(start.chain(bytes).finish(), expected, "{len}");
                    for piece in [1, 3, 7, 16, 4097] {
                        let sum = bytes.chunks(piece).fold(start, Fletcher4::chain);
                        assert_eq!(sum.finish(), expected, "{len} in pieces of {piece}");
                    }
                }
            }
        }
    }

    /// What ZFS 2.4.1 stamped on blocks of the pattern, as for Fletcher-4.
    #[cfg(feature = "fletcher2")]
    #[test]
    fn sums_pairs_as_zfs_does() {
        assert_eq!(
            fletcher2(&pattern(131_072)),
            [
                0x127A_E34A_B723_814A,
                0xBE2C_8EFB_68D1_28F6,
                0xE8DD_1AF3_8EFC_F16D,
                0xB515_8DAA_9846_D5F8
            ]
        );
        assert_eq!(
            fletcher2(&pattern(131_072)[..4096]),
            [
                0xC2FF_4688_CA11_521D,
                0xA2EB_276E_B0F2_38FE,
                0x32A3_8394_05A4_54C3,
                0x4C8B_0AE1_DF56_BC48
            ]
        );
        let bytes = pattern(1000);
        let swapped: [u8; 1008] = core::array::from_fn(|n| bytes[n / 8 * 8 + 7 - n % 8]);
        assert_eq!(
            Fletcher2::new_big_endian().chain(&swapped).finish(),
            fletcher2(&bytes[..1008]),
        );
        for piece in [1, 15, 17] {
            let sum = bytes[..1000]
                .chunks(piece)
                .fold(Fletcher2::new(), Fletcher2::chain);
            assert_eq!(sum.finish(), fletcher2(&bytes[..1000]));
        }
    }

    /// What macOS stamped on an APFS container's superblock, in its first
    /// 8 bytes, over the rest; and the sums, with it, both zero.
    #[cfg(feature = "fletcher64")]
    #[test]
    fn checks_as_apfs_does() {
        let object = include_bytes!("../tests/data/apfs-container.bin");
        let (stored, rest) = object.split_at(8);
        let stored = u64::from_le_bytes(stored.try_into().unwrap());
        assert_eq!(fletcher64(rest), stored);
        let whole = Fletcher64::new().chain(rest).chain(&stored.to_le_bytes());
        assert_eq!(whole.sums(), [0, 0]);
        for piece in [1, 3, 5, 4096] {
            let sum = rest
                .chunks(piece)
                .fold(Fletcher64::new(), Fletcher64::chain);
            assert_eq!(sum.finish(), stored);
        }
    }

    /// Past the words it sums before reducing, the sums still come to
    /// what reducing every word gives.
    #[cfg(feature = "fletcher64")]
    #[test]
    fn reduces_before_its_sums_overflow() {
        let ones = [0xFF; 1 << 20];
        let (mut s1, mut s2) = (0_u64, 0_u64);
        for _ in 0..(1 << 18) {
            s1 = (s1 + 0xFFFF_FFFF) % MODULUS;
            s2 = (s2 + s1) % MODULUS;
        }
        assert_eq!(Fletcher64::new().chain(&ones).sums(), [s1, s2]);
        let pieces = ones
            .chunks(100_003)
            .fold(Fletcher64::new(), Fletcher64::chain);
        assert_eq!(pieces.sums(), [s1, s2]);
    }
}

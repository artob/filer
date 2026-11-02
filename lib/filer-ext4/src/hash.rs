// This is free and unencumbered software released into the public domain.

//! Directory hashes: what hashed (htree) directories index names by.
//!
//! A directory's index names its hash version: legacy, half-MD4, or TEA,
//! each over the name's bytes as signed or, by the superblock's flags,
//! unsigned chars, as the machine that made the directory read them.
//! Half-MD4 and TEA start from the superblock's seed, or, if it is zero,
//! from MD4's initial values. Each hash is 32 bits, with its lowest bit
//! clear, and half-MD4 and TEA have a minor hash too, which lookups need
//! not.
//!
//! Only code defines these exactly, so they are built from the published
//! algorithms (RFC 1320's MD4, with three rounds over eight words; TEA,
//! with 16 rounds) and checked against `debugfs`'s `dx_hash`
//! (`DESIGN.md` §17.15).
//!
//! The name is read in blocks of words, 32 bytes for half-MD4 and 16 for
//! TEA: each word gathers four bytes, the first the most significant,
//! over a pad made of the name's length in every byte; words past the
//! name are the pad.

/// MD4's initial values, the seed if the superblock's is zero.
const DEFAULT_SEED: [u32; 4] = [0x6745_2301, 0xEFCD_AB89, 0x98BA_DCFE, 0x1032_5476];

/// A hash version, as an index names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Version {
    Legacy,
    HalfMd4,
    Tea,
}

impl Version {
    /// Returns the version an index's byte names, if it is one of these.
    pub(crate) const fn new(byte: u8) -> Option<Self> {
        match byte {
            0 => Some(Self::Legacy),
            1 => Some(Self::HalfMd4),
            2 => Some(Self::Tea),
            _ => None,
        }
    }
}

/// How a volume's directories hash names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Hasher {
    pub(crate) version: Version,
    /// Bytes are read as unsigned chars, rather than signed.
    pub(crate) unsigned: bool,
    pub(crate) seed: [u32; 4],
}

impl Hasher {
    /// Returns the hash of `name`, its lowest bit clear, and its minor
    /// hash.
    pub(crate) fn hash(&self, name: &[u8]) -> (u32, u32) {
        let seed = if self.seed == [0; 4] {
            DEFAULT_SEED
        } else {
            self.seed
        };
        let (hash, minor) = match self.version {
            Version::Legacy => (legacy(name, self.unsigned), 0),
            Version::HalfMd4 => {
                let mut buf = seed;
                for (i, block) in name.chunks(32).enumerate() {
                    let left = name.len().saturating_sub(i.saturating_mul(32));
                    half_md4(&mut buf, &words::<8>(block, left, self.unsigned));
                }
                (buf[1], buf[2])
            },
            Version::Tea => {
                let mut buf = seed;
                for (i, block) in name.chunks(16).enumerate() {
                    let left = name.len().saturating_sub(i.saturating_mul(16));
                    tea(&mut buf, &words::<4>(block, left, self.unsigned));
                }
                (buf[0], buf[1])
            },
        };
        // The largest hash marks the end of the index.
        let hash = match hash & !1 {
            0xFFFF_FFFE => 0xFFFF_FFFC,
            hash => hash,
        };
        (hash, minor)
    }
}

/// Returns byte `byte` as a char of the kind given, widened.
fn char(byte: u8, unsigned: bool) -> u32 {
    if unsigned {
        u32::from(byte)
    } else {
        u32::from_ne_bytes(i32::from(i8::from_ne_bytes([byte])).to_ne_bytes())
    }
}

/// The legacy hash, of two registers mixed by each byte.
fn legacy(name: &[u8], unsigned: bool) -> u32 {
    let (mut a, mut b) = (0x12A3_FE2D_u32, 0x37AB_E8F9_u32);
    for &byte in name {
        let mut hash = b.wrapping_add(a ^ char(byte, unsigned).wrapping_mul(7_152_373));
        if hash & 0x8000_0000 != 0 {
            hash = hash.wrapping_sub(0x7FFF_FFFF);
        }
        b = a;
        a = hash;
    }
    a.wrapping_shl(1)
}

/// Returns `N` words of a block of the name, which leaves `len` bytes of
/// it from the block's start.
fn words<const N: usize>(block: &[u8], len: usize, unsigned: bool) -> [u32; N] {
    let len = u32::try_from(len).unwrap_or(u32::MAX);
    let pad = len | len.wrapping_shl(8);
    let pad = pad | pad.wrapping_shl(16);
    let mut words = [pad; N];
    for (word, bytes) in words.iter_mut().zip(block.chunks(4)) {
        *word = bytes.iter().fold(pad, |value, &byte| {
            value.wrapping_shl(8).wrapping_add(char(byte, unsigned))
        });
    }
    words
}

/// A round function of MD4.
type Round = fn(u32, u32, u32) -> u32;

/// MD4's three round functions.
fn f(x: u32, y: u32, z: u32) -> u32 {
    z ^ (x & (y ^ z))
}

fn g(x: u32, y: u32, z: u32) -> u32 {
    (x & y).wrapping_add((x ^ y) & z)
}

fn h(x: u32, y: u32, z: u32) -> u32 {
    x ^ y ^ z
}

/// Half-MD4: MD4's three rounds over eight words, each word twice a
/// round, added to `buf`.
fn half_md4(buf: &mut [u32; 4], input: &[u32; 8]) {
    const K2: u32 = 0x5A82_7999;
    const K3: u32 = 0x6ED9_EBA1;
    let [mut a, mut b, mut c, mut d] = *buf;
    let word = |i: usize| input.get(i).copied().unwrap_or(0);
    let step = |round: Round, a: u32, b, c, d, x: u32, s| {
        a.wrapping_add(round(b, c, d))
            .wrapping_add(x)
            .rotate_left(s)
    };
    // The rounds' word orders and shifts, a step per word.
    let rounds: [(Round, u32, [usize; 8], [u32; 4]); 3] = [
        (f, 0, [0, 1, 2, 3, 4, 5, 6, 7], [3, 7, 11, 19]),
        (g, K2, [1, 3, 5, 7, 0, 2, 4, 6], [3, 5, 9, 13]),
        (h, K3, [3, 7, 2, 6, 1, 5, 0, 4], [3, 9, 11, 15]),
    ];
    for (round, k, order, shifts) in rounds {
        for (i, &w) in order.iter().enumerate() {
            let x = word(w).wrapping_add(k);
            let s = shifts.get(i % 4).copied().unwrap_or(0);
            match i % 4 {
                0 => a = step(round, a, b, c, d, x, s),
                1 => d = step(round, d, a, b, c, x, s),
                2 => c = step(round, c, d, a, b, x, s),
                _ => b = step(round, b, c, d, a, x, s),
            }
        }
    }
    for (value, add) in buf.iter_mut().zip([a, b, c, d]) {
        *value = value.wrapping_add(add);
    }
}

/// TEA: 16 rounds of the cipher, keyed by four words, over `buf`'s first
/// two, added to them.
fn tea(buf: &mut [u32; 4], key: &[u32; 4]) {
    const DELTA: u32 = 0x9E37_79B9;
    let [k0, k1, k2, k3] = *key;
    let [mut b0, mut b1, ..] = *buf;
    let mut sum = 0u32;
    for _ in 0..16 {
        sum = sum.wrapping_add(DELTA);
        b0 = b0.wrapping_add(
            (b1.wrapping_shl(4).wrapping_add(k0))
                ^ b1.wrapping_add(sum)
                ^ (b1.wrapping_shr(5).wrapping_add(k1)),
        );
        b1 = b1.wrapping_add(
            (b0.wrapping_shl(4).wrapping_add(k2))
                ^ b0.wrapping_add(sum)
                ^ (b0.wrapping_shr(5).wrapping_add(k3)),
        );
    }
    let [first, second, ..] = buf;
    *first = first.wrapping_add(b0);
    *second = second.wrapping_add(b1);
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    /// Parses a seed as `debugfs` shows it, a UUID of the seed's bytes.
    fn seed(text: &str) -> [u32; 4] {
        if text == "-" {
            return [0; 4];
        }
        let bytes = hex(&text.replace('-', ""));
        core::array::from_fn(|i| u32::from_le_bytes(bytes[i * 4..i * 4 + 4].try_into().unwrap()))
    }

    /// Decodes bytes written in hexadecimal.
    fn hex(text: &str) -> Vec<u8> {
        text.as_bytes()
            .chunks(2)
            .map(|pair| u8::from_str_radix(core::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect()
    }

    fn number(text: &str) -> u32 {
        u32::from_str_radix(text.trim_start_matches("0x"), 16).unwrap()
    }

    /// Every hash version, signed and unsigned, with and without a seed,
    /// over names of every length a block boundary matters to, as
    /// `debugfs dx_hash` computes them.
    #[test]
    fn hashes_as_debugfs_does() {
        let vectors = include_str!("../tests/fixtures/dx_hash.txt");
        let mut checked = 0;
        for line in vectors.lines().filter(|line| !line.starts_with('#')) {
            let fields: Vec<&str> = line.split('\t').collect();
            let [version, seed_text, name, hash, minor] = fields[..] else {
                panic!("{line}");
            };
            let version: u8 = version.parse().unwrap();
            let name = hex(name);
            let hasher = Hasher {
                version: Version::new(version % 3).unwrap(),
                unsigned: version >= 3,
                seed: seed(seed_text),
            };
            assert_eq!(hasher.hash(&name), (number(hash), number(minor)), "{line}");
            checked += 1;
        }
        assert_eq!(checked, 360);
    }

    #[test]
    fn knows_three_versions() {
        assert_eq!(Version::new(1), Some(Version::HalfMd4));
        assert_eq!(Version::new(3), None);
        assert_eq!(Version::new(6), None);
    }
}

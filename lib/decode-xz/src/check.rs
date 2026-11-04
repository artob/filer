// This is free and unencumbered software released into the public domain.

//! The checks xz stamps on headers and blocks: CRC-32 and CRC-64, as the
//! `.xz` file format specification gives them, eight bytes at a time, and
//! SHA-256, from FIPS 180-4.

/// The CRC-32 and CRC-64 polynomials, their bits reversed, as both read
/// each byte from its least significant bit.
const CRC32_POLY: u32 = 0xEDB8_8320;
const CRC64_POLY: u64 = 0xC96C_5795_D787_0F42;

/// Tables for taking eight bytes at a time: `TABLES[0][b]` is the CRC of
/// byte `b`, and `TABLES[k][b]` that of `b` followed by `k` zero bytes.
/// `static`s, as a `const` would be copied wherever it is used, unless
/// optimized away.
static CRC32_TABLES: [[u32; 256]; 8] = crc32_tables();
static CRC64_TABLES: [[u64; 256]; 8] = crc64_tables();

/// Builds the CRC-32 tables, at compile time, where any overflow or index
/// out of range fails the build instead of panicking.
#[allow(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::indexing_slicing,
    reason = "evaluated at compile time"
)]
const fn crc32_tables() -> [[u32; 256]; 8] {
    let mut tables = [[0; 256]; 8];
    let mut byte = 0;
    while byte < 256 {
        let mut crc = byte as u32;
        let mut bit = 0;
        while bit < 8 {
            crc = if crc & 1 == 0 {
                crc >> 1
            } else {
                (crc >> 1) ^ CRC32_POLY
            };
            bit += 1;
        }
        tables[0][byte] = crc;
        byte += 1;
    }
    let mut k = 1;
    while k < 8 {
        let mut byte = 0;
        while byte < 256 {
            let previous = tables[k - 1][byte];
            tables[k][byte] = (previous >> 8) ^ tables[0][(previous & 0xFF) as usize];
            byte += 1;
        }
        k += 1;
    }
    tables
}

/// Builds the CRC-64 tables, as `crc32_tables` does.
#[allow(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::indexing_slicing,
    reason = "evaluated at compile time"
)]
const fn crc64_tables() -> [[u64; 256]; 8] {
    let mut tables = [[0; 256]; 8];
    let mut byte = 0;
    while byte < 256 {
        let mut crc = byte as u64;
        let mut bit = 0;
        while bit < 8 {
            crc = if crc & 1 == 0 {
                crc >> 1
            } else {
                (crc >> 1) ^ CRC64_POLY
            };
            bit += 1;
        }
        tables[0][byte] = crc;
        byte += 1;
    }
    let mut k = 1;
    while k < 8 {
        let mut byte = 0;
        while byte < 256 {
            let previous = tables[k - 1][byte];
            tables[k][byte] = (previous >> 8) ^ tables[0][(previous & 0xFF) as usize];
            byte += 1;
        }
        k += 1;
    }
    tables
}

/// Returns entry `byte` of table `k` of `tables`, always found: `k` is
/// below 8, and a byte below 256.
#[inline(always)]
fn entry<T: Copy + Default>(tables: &[[T; 256]; 8], k: usize, byte: u8) -> T {
    tables
        .get(k)
        .and_then(|table| table.get(usize::from(byte)))
        .copied()
        .unwrap_or_default()
}

/// A CRC-32 computed over bytes fed in pieces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Crc32 {
    /// The register, inverted, as CRC-32 starts and ends it.
    state: u32,
}

impl Crc32 {
    /// Starts a CRC-32 over no bytes.
    pub(crate) const fn new() -> Self {
        Self { state: !0 }
    }

    /// Feeds `bytes`.
    #[must_use]
    pub(crate) fn update(self, bytes: &[u8]) -> Self {
        let table = |k, byte| entry(&CRC32_TABLES, k, byte);
        let mut crc = self.state;
        let (groups, rest) = bytes.as_chunks::<8>();
        for group in groups {
            let [a, b, c, d, e, f, g, h] = *group;
            let [a, b, c, d] = (u32::from_le_bytes([a, b, c, d]) ^ crc).to_le_bytes();
            crc = table(7, a)
                ^ table(6, b)
                ^ table(5, c)
                ^ table(4, d)
                ^ table(3, e)
                ^ table(2, f)
                ^ table(1, g)
                ^ table(0, h);
        }
        for &byte in rest {
            let [low, ..] = crc.to_le_bytes();
            crc = table(0, low ^ byte) ^ (crc >> 8);
        }
        Self { state: crc }
    }

    /// Returns the CRC-32 of the bytes fed.
    pub(crate) const fn finish(self) -> u32 {
        !self.state
    }
}

/// Returns the CRC-32 of `bytes`.
pub(crate) fn crc32(bytes: &[u8]) -> u32 {
    Crc32::new().update(bytes).finish()
}

/// A CRC-64 (ECMA-182, reflected) computed over bytes fed in pieces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Crc64 {
    /// The register, inverted, as CRC-64 starts and ends it.
    state: u64,
}

impl Crc64 {
    /// Starts a CRC-64 over no bytes.
    pub(crate) const fn new() -> Self {
        Self { state: !0 }
    }

    /// Feeds `bytes`.
    #[must_use]
    pub(crate) fn update(self, bytes: &[u8]) -> Self {
        let table = |k, byte| entry(&CRC64_TABLES, k, byte);
        let mut crc = self.state;
        let (groups, rest) = bytes.as_chunks::<8>();
        for group in groups {
            let [a, b, c, d, e, f, g, h] = (u64::from_le_bytes(*group) ^ crc).to_le_bytes();
            crc = table(7, a)
                ^ table(6, b)
                ^ table(5, c)
                ^ table(4, d)
                ^ table(3, e)
                ^ table(2, f)
                ^ table(1, g)
                ^ table(0, h);
        }
        for &byte in rest {
            let [low, ..] = crc.to_le_bytes();
            crc = table(0, low ^ byte) ^ (crc >> 8);
        }
        Self { state: crc }
    }

    /// Returns the CRC-64 of the bytes fed.
    pub(crate) const fn finish(self) -> u64 {
        !self.state
    }
}

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
pub(crate) struct Sha256 {
    hash: [u32; 8],
    /// The bytes of a block not yet whole.
    block: [u8; 64],
    held: usize,
    /// How many bytes have been fed, in all.
    len: u64,
}

impl Sha256 {
    /// Starts a SHA-256 over no bytes.
    pub(crate) const fn new() -> Self {
        Self {
            hash: H0,
            block: [0; 64],
            held: 0,
            len: 0,
        }
    }

    /// Feeds `bytes`.
    pub(crate) fn update(&mut self, mut bytes: &[u8]) {
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
            self.compress(&block);
            self.held = 0;
        }
        let (blocks, rest) = bytes.as_chunks::<64>();
        for block in blocks {
            self.compress(block);
        }
        if let Some(to) = self.block.get_mut(..rest.len()) {
            to.copy_from_slice(rest);
        }
        self.held = rest.len();
    }

    /// Returns the SHA-256 of the bytes fed.
    pub(crate) fn finish(mut self) -> [u8; 32] {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_the_check_values() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        let crc = Crc64::new().update(b"1234").update(b"56789");
        assert_eq!(crc.finish(), 0x995D_C9BB_DF19_39FA);
    }

    #[test]
    fn hashes_as_fips_180_4_does() {
        let hex = |digest: [u8; 32]| {
            digest
                .iter()
                .map(|byte| alloc::format!("{byte:02x}"))
                .collect::<alloc::string::String>()
        };
        let sha256 = |bytes: &[u8]| {
            let mut sha = Sha256::new();
            sha.update(bytes);
            hex(sha.finish())
        };
        assert_eq!(
            sha256(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let two = b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq";
        assert_eq!(
            sha256(two),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        // In pieces that straddle blocks.
        let long = [b'a'; 1000];
        let mut sha = Sha256::new();
        for piece in long.chunks(7) {
            sha.update(piece);
        }
        let mut whole = Sha256::new();
        whole.update(&long);
        assert_eq!(hex(sha.finish()), hex(whole.finish()));
    }
}

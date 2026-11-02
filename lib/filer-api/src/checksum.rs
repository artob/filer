// This is free and unencumbered software released into the public domain.

//! Checksums that drivers share (D53), in pure Rust and without
//! allocation.
//!
//! [`Crc32c`] is CRC-32C (Castagnoli), which XFS and ext4 stamp on their
//! metadata and iSCSI on its frames. [`Crc32`] is CRC-32 (ISO-HDLC), which
//! ZIP stamps on members and GPT on its partition tables.

/// The CRC-32 and CRC-32C polynomials, their bits reversed, as both read
/// each byte from its least significant bit.
const CRC32_POLY: u32 = 0xEDB8_8320;
const CRC32C_POLY: u32 = 0x82F6_3B78;

/// Tables for taking eight bytes at a time: `TABLES[0][b]` is the CRC of
/// byte `b`, and `TABLES[k][b]` that of `b` followed by `k` zero bytes.
/// `static`s, as a `const` would be copied wherever it is used, unless
/// optimized away.
type Tables = [[u32; 256]; 8];
static CRC32_TABLES: Tables = tables(CRC32_POLY);
static CRC32C_TABLES: Tables = tables(CRC32C_POLY);

/// Builds the tables of the reflected polynomial `poly`, at compile time,
/// where any overflow or index out of range fails the build instead of
/// panicking.
#[allow(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::indexing_slicing,
    reason = "evaluated at compile time"
)]
const fn tables(poly: u32) -> Tables {
    let mut tables = [[0; 256]; 8];
    let mut byte = 0;
    while byte < 256 {
        let mut crc = byte;
        let mut bit = 0;
        while bit < 8 {
            crc = if crc & 1 == 0 {
                crc >> 1
            } else {
                (crc >> 1) ^ poly
            };
            bit += 1;
        }
        tables[0][byte as usize] = crc;
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

/// Feeds `bytes` to the register `crc` of a CRC of `tables`.
fn update(tables: &Tables, mut crc: u32, bytes: &[u8]) -> u32 {
    // Always found: `k` is below 8, and a byte below 256.
    let table = |k: usize, byte: u8| {
        tables
            .get(k)
            .and_then(|table| table.get(usize::from(byte)))
            .copied()
            .unwrap_or(0)
    };
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
    crc
}

/// A CRC-32C computed over bytes fed in pieces.
///
/// # Examples
///
/// ```
/// use filer_api::checksum::{Crc32c, crc32c};
///
/// assert_eq!(crc32c(b"123456789"), 0xE306_9283);
/// let crc = Crc32c::new().update(b"1234").update(b"56789");
/// assert_eq!(crc.finish(), 0xE306_9283);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Crc32c {
    /// The register, inverted, as CRC-32C starts and ends it.
    state: u32,
}

impl Crc32c {
    /// Starts a CRC-32C over no bytes.
    #[must_use]
    pub const fn new() -> Self {
        Self { state: !0 }
    }

    /// Feeds `bytes`.
    #[must_use]
    pub fn update(self, bytes: &[u8]) -> Self {
        Self {
            state: update(&CRC32C_TABLES, self.state, bytes),
        }
    }

    /// Returns the CRC-32C of the bytes fed.
    #[must_use]
    pub const fn finish(self) -> u32 {
        !self.state
    }
}

impl Default for Crc32c {
    fn default() -> Self {
        Self::new()
    }
}

/// Returns the CRC-32C of `bytes`.
#[must_use]
pub fn crc32c(bytes: &[u8]) -> u32 {
    Crc32c::new().update(bytes).finish()
}

/// A CRC-32 computed over bytes fed in pieces.
///
/// # Examples
///
/// ```
/// use filer_api::checksum::{Crc32, crc32};
///
/// assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
/// let crc = Crc32::new().update(b"1234").update(b"56789");
/// assert_eq!(crc.finish(), 0xCBF4_3926);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Crc32 {
    /// The register, inverted, as CRC-32 starts and ends it.
    state: u32,
}

impl Crc32 {
    /// Starts a CRC-32 over no bytes.
    #[must_use]
    pub const fn new() -> Self {
        Self { state: !0 }
    }

    /// Feeds `bytes`.
    #[must_use]
    pub fn update(self, bytes: &[u8]) -> Self {
        Self {
            state: update(&CRC32_TABLES, self.state, bytes),
        }
    }

    /// Returns the CRC-32 of the bytes fed.
    #[must_use]
    pub const fn finish(self) -> u32 {
        !self.state
    }
}

impl Default for Crc32 {
    fn default() -> Self {
        Self::new()
    }
}

/// Returns the CRC-32 of `bytes`.
#[must_use]
pub fn crc32(bytes: &[u8]) -> u32 {
    Crc32::new().update(bytes).finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    #[test]
    fn matches_known_values() {
        assert_eq!(crc32c(b""), 0);
        assert_eq!(crc32c(b"123456789"), 0xE306_9283);
        // RFC 3720's, for iSCSI.
        assert_eq!(crc32c(&[0; 32]), 0x8A91_36AA);
        assert_eq!(crc32c(&[0xFF; 32]), 0x62A8_AB43);
        let ascending: Vec<u8> = (0..32).collect();
        assert_eq!(crc32c(&ascending), 0x46DD_794E);
        let descending: Vec<u8> = (0..32).rev().collect();
        assert_eq!(crc32c(&descending), 0x113F_DB5C);
        assert_eq!(crc32(b""), 0);
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(
            crc32(b"The quick brown fox jumps over the lazy dog"),
            0x414F_A339
        );
    }

    #[test]
    fn feeds_in_any_pieces() {
        let bytes: Vec<u8> = (0..1000u32).map(|i| (i * 7 % 251) as u8).collect();
        let (whole_c, whole) = (crc32c(&bytes), crc32(&bytes));
        for split in [0, 1, 7, 8, 9, 500, 999, 1000] {
            let (a, b) = bytes.split_at(split);
            assert_eq!(Crc32c::new().update(a).update(b).finish(), whole_c);
            assert_eq!(Crc32::new().update(a).update(b).finish(), whole);
        }
        // Byte by byte, without the eight-byte path.
        let crc = bytes
            .iter()
            .fold(Crc32c::default(), |crc, &byte| crc.update(&[byte]));
        assert_eq!(crc.finish(), whole_c);
        let crc = bytes
            .iter()
            .fold(Crc32::default(), |crc, &byte| crc.update(&[byte]));
        assert_eq!(crc.finish(), whole);
    }
}

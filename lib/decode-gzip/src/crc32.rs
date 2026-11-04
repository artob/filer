// This is free and unencumbered software released into the public domain.

//! CRC-32 (ISO-HDLC), which a member's trailer holds of what it decodes
//! to, and its header's CRC-16 is the low half of, eight bytes at a time.

/// The polynomial, its bits reversed, as CRC-32 reads each byte from its
/// least significant bit.
const POLY: u32 = 0xEDB8_8320;

/// Tables for taking eight bytes at a time: `TABLES[0][b]` is the CRC of
/// byte `b`, and `TABLES[k][b]` that of `b` followed by `k` zero bytes. A
/// `static`, as a `const` would be copied wherever it is used, unless
/// optimized away.
type Tables = [[u32; 256]; 8];
static TABLES: Tables = tables();

/// Builds the tables, at compile time, where any overflow or index out of
/// range fails the build instead of panicking.
#[allow(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::indexing_slicing,
    reason = "evaluated at compile time"
)]
const fn tables() -> Tables {
    let mut tables = [[0; 256]; 8];
    let mut byte = 0;
    while byte < 256 {
        let mut crc = byte;
        let mut bit = 0;
        while bit < 8 {
            crc = if crc & 1 == 0 {
                crc >> 1
            } else {
                (crc >> 1) ^ POLY
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
        // Always found: `k` is below 8, and a byte below 256.
        let table = |k: usize, byte: u8| {
            TABLES
                .get(k)
                .and_then(|table| table.get(usize::from(byte)))
                .copied()
                .unwrap_or(0)
        };
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_the_check_value() {
        assert_eq!(Crc32::new().update(b"123456789").finish(), 0xCBF4_3926);
        let pieces = Crc32::new().update(b"1234").update(b"56789");
        assert_eq!(pieces.finish(), 0xCBF4_3926);
        assert_eq!(Crc32::new().finish(), 0);
    }
}

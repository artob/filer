// This is free and unencumbered software released into the public domain.

//! `$UpCase`: the volume's own table of uppercase UTF-16 units, by which
//! names are matched case-insensitively, as on Windows.

// The driver (NTFS-05) uses the rest of this module.
#![allow(dead_code)]

use alloc::{boxed::Box, vec::Vec};

/// The units in the table: every UTF-16 unit.
const UNITS: usize = 0x1_0000;

/// A volume's `$UpCase` table.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct UpCase(Box<[u16]>);

impl UpCase {
    /// Parses the table from `$UpCase`'s data, or returns `None` if it is
    /// not exactly one unit for each of the 65,536.
    pub(crate) fn parse(bytes: &[u8]) -> Option<Self> {
        let (units, rest) = bytes.as_chunks::<2>();
        if units.len() != UNITS || !rest.is_empty() {
            return None;
        }
        let table: Vec<u16> = units.iter().map(|&unit| u16::from_le_bytes(unit)).collect();
        Some(Self(table.into_boxed_slice()))
    }

    /// Returns the uppercase of `unit`.
    pub(crate) fn upcase(&self, unit: u16) -> u16 {
        self.0.get(usize::from(unit)).copied().unwrap_or(unit)
    }

    /// Returns `true` if `a` and `b` are the same name, ignoring case.
    pub(crate) fn eq_ignore_case(&self, a: &[u16], b: &[u16]) -> bool {
        a.len() == b.len()
            && a.iter()
                .zip(b)
                .all(|(&a, &b)| self.upcase(a) == self.upcase(b))
    }
}

impl core::fmt::Debug for UpCase {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("UpCase(..)")
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use alloc::vec::Vec;

    /// A table uppercasing ASCII and the Latin-1 letters, as a volume's
    /// own table does among much else.
    pub(crate) fn table() -> Vec<u8> {
        (0..=u16::MAX)
            .map(|unit| match unit {
                0x61..=0x7A | 0xE0..=0xFE if unit != 0xF7 => unit - 0x20,
                _ => unit,
            })
            .flat_map(u16::to_le_bytes)
            .collect()
    }

    #[test]
    fn matches_names_ignoring_case() {
        let upcase = UpCase::parse(&table()).unwrap();
        let units = |s: &str| s.encode_utf16().collect::<Vec<_>>();
        assert!(upcase.eq_ignore_case(&units("Ünïcödé.TXT"), &units("üNÏCÖDÉ.txt")));
        assert!(!upcase.eq_ignore_case(&units("a"), &units("ab")));
        assert!(!upcase.eq_ignore_case(&units("÷"), &units("×")));
        assert_eq!(UpCase::parse(&table()[..100]), None);
        assert_eq!(UpCase::parse(&[0; 131_073]), None);
    }
}

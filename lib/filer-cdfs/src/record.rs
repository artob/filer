// This is free and unencumbered software released into the public domain.

//! Directory records: each names a file or directory, its extent, its
//! size, its date, and its flags, then has room for System Use entries.
//! Numbers are recorded twice, little-endian then big-endian; only the
//! first is read, as the big-endian halves are often wrong in practice.

use filer_fs::le;

/// The flag that a file is hidden; it is shown all the same, marked.
pub(crate) const HIDDEN: u8 = 0x01;

/// The flag that a record is a directory's.
pub(crate) const DIRECTORY: u8 = 0x02;

/// The flag of an associated file, a resource of another of its name,
/// which is not shown.
pub(crate) const ASSOCIATED: u8 = 0x04;

/// The flag that a file's next extent follows in the next record.
pub(crate) const MULTI_EXTENT: u8 = 0x80;

/// The fixed fields' length, before the name.
const FIXED: usize = 33;

/// A directory record, borrowed from the sector holding it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Record<'a> {
    /// The first logical block of its extent.
    pub(crate) extent: u32,
    /// The length of its extended attribute record, in logical blocks,
    /// which precedes its data in its extent.
    pub(crate) attributes: u8,
    /// The length of its data in bytes.
    pub(crate) len: u32,
    /// Its date, in seven bytes.
    pub(crate) date: &'a [u8],
    pub(crate) flags: u8,
    /// Whether it is interleaved: in units separated by gaps.
    pub(crate) interleaved: bool,
    pub(crate) name: &'a [u8],
    pub(crate) system_use: &'a [u8],
}

impl<'a> Record<'a> {
    /// Parses the record at the start of `bytes`, of the length its first
    /// byte gives, or returns `None` if it is malformed.
    pub(crate) fn parse(bytes: &'a [u8]) -> Option<Self> {
        let len = usize::from(*bytes.first()?);
        let bytes = bytes.get(..len)?;
        let name_len = usize::from(*bytes.get(32)?);
        let name_end = FIXED.checked_add(name_len)?;
        let name = bytes.get(FIXED..name_end).filter(|name| !name.is_empty())?;
        // A padding byte after a name of even length starts the System
        // Use field at an even offset.
        let system_use = name_end.saturating_add(usize::from(name_len.is_multiple_of(2)));
        Some(Self {
            extent: le::u32_at(bytes, 2)?,
            attributes: *bytes.get(1)?,
            len: le::u32_at(bytes, 10)?,
            date: bytes.get(18..25)?,
            flags: *bytes.get(25)?,
            interleaved: bytes.get(26..28)? != [0, 0],
            name,
            system_use: bytes.get(system_use..).unwrap_or_default(),
        })
    }

    /// Returns `true` for the records of `.` and `..`, which begin every
    /// directory, named by a single byte 0 and 1.
    pub(crate) fn is_dot(&self) -> bool {
        matches!(self.name, [0 | 1])
    }

    /// Returns the position of its data, after its extended attribute
    /// record, in logical blocks of `block` bytes.
    pub(crate) fn data(&self, block: u32) -> Option<u64> {
        u64::from(self.extent)
            .checked_add(u64::from(self.attributes))?
            .checked_mul(u64::from(block))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{vec, vec::Vec};

    /// Returns a record of `name`, with `system_use` after it.
    fn record(name: &[u8], system_use: &[u8]) -> Vec<u8> {
        let pad = usize::from(name.len().is_multiple_of(2));
        let len = FIXED + name.len() + pad + system_use.len();
        let mut bytes = vec![0; len];
        bytes[0] = len as u8;
        bytes[2..6].copy_from_slice(&20_u32.to_le_bytes());
        bytes[10..14].copy_from_slice(&2048_u32.to_le_bytes());
        bytes[18..25].copy_from_slice(&[124, 1, 2, 3, 4, 6, 0]);
        bytes[25] = DIRECTORY;
        bytes[32] = name.len() as u8;
        bytes[FIXED..FIXED + name.len()].copy_from_slice(name);
        bytes[FIXED + name.len() + pad..].copy_from_slice(system_use);
        bytes
    }

    #[test]
    fn parses_records() {
        let bytes = record(b"NAME.TXT;1", b"SU");
        let parsed = Record::parse(&bytes).unwrap();
        assert_eq!(
            (parsed.extent, parsed.len, parsed.flags),
            (20, 2048, DIRECTORY)
        );
        assert_eq!(
            (parsed.name, parsed.system_use),
            (&b"NAME.TXT;1"[..], &b"SU"[..])
        );
        assert_eq!(parsed.data(2048), Some(20 * 2048));
        assert!(!parsed.is_dot() && !parsed.interleaved);
        let odd = record(b"ABC", b"SU");
        assert_eq!(Record::parse(&odd).unwrap().system_use, b"SU");
        assert!(Record::parse(&record(&[0], &[])).unwrap().is_dot());
        assert!(Record::parse(&record(&[1], &[])).unwrap().is_dot());
    }

    #[test]
    fn refuses_malformed_records() {
        let bytes = record(b"NAME", &[]);
        assert!(Record::parse(&bytes[..bytes.len() - 1]).is_none());
        let mut empty = record(b"N", &[]);
        empty[32] = 0;
        assert!(Record::parse(&empty).is_none());
        let mut long = record(b"N", &[]);
        long[32] = 9;
        assert!(Record::parse(&long).is_none());
        assert!(Record::parse(&[]).is_none());
    }
}

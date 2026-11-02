// This is free and unencumbered software released into the public domain.

//! Directory entries as `NtQueryDirectoryFile` returns them, per
//! [MS-FSCC] §2.4: records chained by the offset of the next, each with
//! the entry's times, sizes, attributes, the length of its UTF-16 name,
//! then the class's own fields, and the name.
//!
//! `FileIdExtdDirectoryInformation` has a reparse tag and a 128-bit file
//! id; `FileIdBothDirectoryInformation`, for file systems without it
//! (FAT), has a 64-bit id and keeps a reparse point's tag where it would
//! keep the size of the extended attributes.
//!
//! [MS-FSCC]: https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-fscc/

use filer_fs::le;
use std::vec::Vec;

/// Where every class keeps an entry's attributes and its name's length.
const ATTRIBUTES: usize = 56;
const NAME_LEN: usize = 60;

/// The information class entries are read in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Class {
    /// `FileIdExtdDirectoryInformation`.
    Extd,
    /// `FileIdBothDirectoryInformation`.
    Both,
}

impl Class {
    /// Where an entry of this class keeps its reparse tag, its file id and
    /// the id's size, and its name.
    const fn layout(self) -> (usize, usize, usize, usize) {
        match self {
            Self::Extd => (68, 72, 16, 88),
            Self::Both => (64, 96, 8, 104),
        }
    }
}

/// A directory entry as listed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RawEntry {
    /// The name, in UTF-16, which need not be well-formed.
    pub(crate) name: Vec<u16>,
    pub(crate) attributes: u32,
    /// The reparse tag, meaningful only for a reparse point.
    pub(crate) tag: u32,
    pub(crate) id: u128,
}

impl RawEntry {
    /// Returns `true` for the entries `.` and `..`.
    pub(crate) fn is_dot(&self) -> bool {
        matches!(self.name.as_slice(), [0x2E] | [0x2E, 0x2E])
    }
}

/// Parses the entries of class `class` in `buf`, as one call returned
/// them, or returns `None` if one does not fit or its name is not whole
/// UTF-16 units.
pub(crate) fn parse(buf: &[u8], class: Class) -> Option<Vec<RawEntry>> {
    let (tag_at, id_at, id_len, name_at) = class.layout();
    let mut entries = Vec::new();
    if buf.is_empty() {
        return Some(entries);
    }
    let mut at = 0usize;
    loop {
        let entry = buf.get(at..)?;
        let next = usize::try_from(le::u32_at(entry, 0)?).ok()?;
        let name_len = usize::try_from(le::u32_at(entry, NAME_LEN)?).ok()?;
        let mut id = [0; 16];
        id.get_mut(..id_len)?
            .copy_from_slice(entry.get(id_at..)?.get(..id_len)?);
        let (units, rest) = entry.get(name_at..)?.get(..name_len)?.as_chunks::<2>();
        if !rest.is_empty() {
            return None;
        }
        entries.push(RawEntry {
            name: units.iter().map(|&unit| u16::from_le_bytes(unit)).collect(),
            attributes: le::u32_at(entry, ATTRIBUTES)?,
            tag: le::u32_at(entry, tag_at)?,
            id: u128::from_le_bytes(id),
        });
        if next == 0 {
            return Some(entries);
        }
        // Offsets only go forward, so the walk ends.
        at = at.checked_add(next)?;
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::{string::String, vec};

    /// Encodes entries of `class`, as `(name, attributes, tag, id)`.
    pub(crate) fn encode(class: Class, entries: &[(&str, u32, u32, u128)]) -> Vec<u8> {
        let (tag_at, id_at, id_len, name_at) = class.layout();
        let mut buf = Vec::new();
        for (i, &(name, attributes, tag, id)) in entries.iter().enumerate() {
            let name: Vec<u8> = name.encode_utf16().flat_map(u16::to_le_bytes).collect();
            let len = (name_at + name.len()).next_multiple_of(8);
            let mut entry = vec![0; len];
            if i + 1 < entries.len() {
                entry[..4].copy_from_slice(&u32::try_from(len).unwrap().to_le_bytes());
            }
            entry[ATTRIBUTES..][..4].copy_from_slice(&attributes.to_le_bytes());
            entry[NAME_LEN..][..4]
                .copy_from_slice(&u32::try_from(name.len()).unwrap().to_le_bytes());
            entry[tag_at..][..4].copy_from_slice(&tag.to_le_bytes());
            entry[id_at..][..id_len].copy_from_slice(&id.to_le_bytes()[..id_len]);
            entry[name_at..][..name.len()].copy_from_slice(&name);
            buf.extend(entry);
        }
        buf
    }

    #[test]
    fn parses_both_classes() {
        let listed = [
            (".", 0x10, 0, 5),
            ("café.txt", 0x20, 0, 0x1234_5678_9ABC),
            ("link", 0x420, 0xA000_000C, 7),
        ];
        for class in [Class::Extd, Class::Both] {
            let entries = parse(&encode(class, &listed), class).unwrap();
            assert_eq!(entries.len(), 3);
            assert!(entries[0].is_dot());
            assert_eq!(String::from_utf16(&entries[1].name).unwrap(), "café.txt");
            assert_eq!(entries[1].id, 0x1234_5678_9ABC);
            assert_eq!(
                (entries[2].attributes, entries[2].tag),
                (0x420, 0xA000_000C)
            );
        }
        assert_eq!(parse(&[], Class::Extd), Some(Vec::new()));
        // A 128-bit id survives only where the class holds one.
        let id = 1u128 << 100;
        assert_eq!(
            parse(&encode(Class::Extd, &[("x", 0, 0, id)]), Class::Extd).unwrap()[0].id,
            id
        );
    }

    #[test]
    fn rejects_entries_that_do_not_fit() {
        let good = encode(Class::Extd, &[("ab", 0, 0, 1), ("cd", 0, 0, 2)]);
        // Cut short, an odd name length, and a next entry past the end.
        let mut odd = good.clone();
        odd[NAME_LEN] = 3;
        let mut past = good.clone();
        past[0] = 0xF0;
        for buf in [&good[..good.len() - 8], &good[..40], &odd, &past] {
            assert_eq!(parse(buf, Class::Extd), None);
        }
    }
}

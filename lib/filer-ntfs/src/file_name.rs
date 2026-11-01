// This is free and unencumbered software released into the public domain.

//! `$FILE_NAME` values: a name of a file in a directory.

use crate::record::FileRef;
use alloc::{boxed::Box, vec::Vec};

/// The namespace of a name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Namespace {
    /// Case-sensitive, any characters but `/` and NUL.
    Posix,
    /// A long name, Windows rules.
    Win32,
    /// A short (8.3) name, accompanying a `Win32` one.
    Dos,
    /// A name that is both a valid long and short name.
    Win32AndDos,
}

/// A `$FILE_NAME` value.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct FileName {
    /// The directory holding the name.
    pub(crate) parent: FileRef,
    pub(crate) namespace: Namespace,
    /// The name: 1 to 255 UTF-16 units.
    pub(crate) name: Box<[u16]>,
    /// The file attributes, as of the last change to this name.
    pub(crate) attributes: u32,
}

impl FileName {
    /// The file attribute marking a directory in `$FILE_NAME`.
    pub(crate) const DIRECTORY: u32 = 0x1000_0000;

    /// Parses a `$FILE_NAME` value, or returns `None` if it does not fit
    /// or its name or namespace is invalid.
    pub(crate) fn parse(value: &[u8]) -> Option<Self> {
        let parent = u64::from_le_bytes(*value.first_chunk()?);
        let attributes = u32::from_le_bytes(*value.get(0x38..)?.first_chunk()?);
        let len = usize::from(*value.get(0x40)?);
        let namespace = match *value.get(0x41)? {
            0 => Namespace::Posix,
            1 => Namespace::Win32,
            2 => Namespace::Dos,
            3 => Namespace::Win32AndDos,
            _ => return None,
        };
        let bytes = value.get(0x42..)?.get(..len.checked_mul(2)?)?;
        let name: Vec<u16> = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&unit| u16::from_le_bytes(unit))
            .collect();
        if name.is_empty() {
            return None;
        }
        Some(Self {
            parent: FileRef::from_u64(parent),
            namespace,
            name: name.into_boxed_slice(),
            attributes,
        })
    }

    /// Returns `true` for a name only DOS uses, which another entry of the
    /// same file shadows with its long name.
    pub(crate) fn is_dos_only(&self) -> bool {
        self.namespace == Namespace::Dos
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use alloc::vec;

    /// Encodes a `$FILE_NAME` value.
    pub(crate) fn file_name(parent: u64, name: &str, namespace: u8, attributes: u32) -> Vec<u8> {
        let units: Vec<u16> = name.encode_utf16().collect();
        let mut value = vec![0u8; 0x42 + units.len() * 2];
        value[0..8].copy_from_slice(&parent.to_le_bytes());
        value[0x38..0x3C].copy_from_slice(&attributes.to_le_bytes());
        value[0x40] = u8::try_from(units.len()).unwrap();
        value[0x41] = namespace;
        for (i, unit) in units.iter().enumerate() {
            value[0x42 + i * 2..0x44 + i * 2].copy_from_slice(&unit.to_le_bytes());
        }
        value
    }

    #[test]
    fn parses_file_names() {
        let value = file_name(5 | 1 << 48, "Notes.txt", 3, 0x20);
        let name = FileName::parse(&value).unwrap();
        assert_eq!(
            name.parent,
            FileRef {
                record: 5,
                sequence: 1
            }
        );
        assert_eq!(name.namespace, Namespace::Win32AndDos);
        assert_eq!(&*name.name, "Notes.txt".encode_utf16().collect::<Vec<_>>());
        assert!(!name.is_dos_only());
        assert!(
            FileName::parse(&file_name(5, "NOTES~1.TXT", 2, 0))
                .unwrap()
                .is_dos_only()
        );
    }

    #[test]
    fn rejects_malformed_file_names() {
        assert_eq!(FileName::parse(&file_name(5, "", 1, 0)), None);
        assert_eq!(FileName::parse(&file_name(5, "x", 4, 0)), None);
        let mut short = file_name(5, "abc", 1, 0);
        short.truncate(short.len() - 1);
        assert_eq!(FileName::parse(&short), None);
    }
}

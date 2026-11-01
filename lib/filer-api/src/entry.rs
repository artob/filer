// This is free and unencumbered software released into the public domain.

use crate::{EntryName, FileId, FileKind};

/// One entry of a directory listing.
///
/// `.` and `..` are never listed. The kind and id come for free with most
/// listing calls but are optional: some filesystems report an unknown type
/// (POSIX `DT_UNKNOWN`), and callers needing it must stat the entry.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct Entry {
    /// The entry's name.
    pub name: EntryName,

    /// The entry's type, if the listing reported it.
    pub kind: Option<FileKind>,

    /// The entry's identity, if the listing reported it.
    pub id: Option<FileId>,
}

impl Entry {
    /// Returns an entry with only its name known.
    #[must_use]
    pub fn new(name: EntryName) -> Self {
        Self {
            name,
            kind: None,
            id: None,
        }
    }
}

// This is free and unencumbered software released into the public domain.

use filer_api::{Error, ErrorKind, OpKind};

/// What is wrong with a UDF volume: the source of the [`Error`] returned
/// when a volume cannot be opened or read.
///
/// Each variant names one inconsistency and carries where it was found,
/// so that callers can match on it and messages can say exactly what was
/// found. Positions are byte offsets in the volume.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum UdfError {
    /// There is no anchor volume descriptor pointer at sector 256, or at
    /// the volume's end, of any block size from 512 bytes to 4 KiB: this
    /// is not a UDF volume.
    NoAnchor,

    /// The volume descriptor sequences have no logical volume descriptor,
    /// or no partition descriptor, or are malformed.
    BadDescriptors,

    /// The logical volume's block size is not the one its anchor was
    /// found at.
    BadBlockSize(u32),

    /// A partition map is of a kind this driver does not read.
    UnsupportedPartition,

    /// A partition is malformed: a map naming no partition, or a virtual
    /// allocation table, sparing table, or metadata file that is missing
    /// or invalid.
    BadPartition {
        /// The partition's reference number, its map's index.
        reference: u16,
    },

    /// A block lies outside its partition.
    BadBlock {
        /// The partition's reference number.
        reference: u16,
        /// The block, in the partition.
        block: u32,
    },

    /// The file set descriptor is missing or invalid.
    BadFileSet,

    /// The root directory's file entry is invalid, or names no directory.
    BadRoot,

    /// A file entry is malformed: an invalid tag, an unknown kind, or
    /// allocation descriptors past its block.
    BadEntry {
        /// The entry's position.
        at: u64,
    },

    /// A file's allocation descriptors are malformed: too many, of an
    /// unknown kind, or continued endlessly.
    BadExtents {
        /// The position of its file entry.
        at: u64,
    },

    /// A directory's file identifiers are malformed: an invalid tag, an
    /// identifier past the directory's end, or an invalid name.
    BadDirectory {
        /// The position of its file entry.
        at: u64,
    },

    /// A symlink's path components are malformed.
    BadSymlink {
        /// The position of its file entry.
        at: u64,
    },
}

impl UdfError {
    /// Returns the [`ErrorKind`] this inconsistency is reported as:
    /// [`Unsupported`](ErrorKind::Unsupported) for a partition this driver
    /// does not read, and [`Corrupt`](ErrorKind::Corrupt) for everything
    /// else.
    #[must_use]
    pub const fn kind(&self) -> ErrorKind {
        match self {
            Self::UnsupportedPartition => ErrorKind::Unsupported,
            _ => ErrorKind::Corrupt,
        }
    }

    /// Returns an [`Error`] of [`kind`](Self::kind) from `op`, with this
    /// as its source.
    #[must_use]
    pub fn into_error(self, op: OpKind) -> Error {
        Error::new(self.kind(), op).with_source(self)
    }
}

impl core::fmt::Display for UdfError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("UDF volume: ")?;
        match self {
            Self::NoAnchor => f.write_str("no anchor volume descriptor pointer"),
            Self::BadDescriptors => f.write_str("bad volume descriptor sequence"),
            Self::BadBlockSize(size) => write!(f, "bad logical block size {size}"),
            Self::UnsupportedPartition => f.write_str("unsupported partition map"),
            Self::BadPartition { reference } => write!(f, "bad partition {reference}"),
            Self::BadBlock { reference, block } => {
                write!(f, "block {block} outside partition {reference}")
            },
            Self::BadFileSet => f.write_str("bad file set descriptor"),
            Self::BadRoot => f.write_str("bad root directory"),
            Self::BadEntry { at } => write!(f, "bad file entry at {at}"),
            Self::BadExtents { at } => write!(f, "bad allocation descriptors of the entry at {at}"),
            Self::BadDirectory { at } => write!(f, "bad file identifiers in the directory at {at}"),
            Self::BadSymlink { at } => write!(f, "bad symlink target of the entry at {at}"),
        }
    }
}

impl core::error::Error for UdfError {}

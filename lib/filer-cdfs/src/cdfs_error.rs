// This is free and unencumbered software released into the public domain.

use filer_api::{Error, ErrorKind, OpKind};

/// What is wrong with an ISO 9660 image: the source of the [`Error`]
/// returned when an image cannot be opened or read.
///
/// Each variant names one inconsistency and carries where it was found,
/// so that callers can match on it and messages can say exactly what was
/// found. Positions are byte offsets in the image: of a directory's
/// extent, or of a directory record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CdfsError {
    /// There is no volume descriptor at sector 16, identified by `CD001`:
    /// this is not an ISO 9660 image.
    NotIso9660,

    /// The volume descriptor set has no primary volume descriptor before
    /// its terminator, or its hundredth sector.
    NoPrimary,

    /// The logical block size is not 512, 1024, or 2048 bytes.
    BadBlockSize(u16),

    /// The root directory is malformed: its record, or the `.` record it
    /// begins with, is invalid or names no directory.
    BadRoot,

    /// A directory's records are malformed: a record crossing a sector,
    /// shorter than its fields, an invalid name, or a file of several
    /// extents whose parts disagree.
    BadDirectory {
        /// The position of the directory's extent.
        at: u64,
    },

    /// A directory record's fields disagree with one another: a
    /// directory's flag and its Rock Ridge mode, or an extent past the
    /// image's end.
    BadRecord {
        /// The record's position.
        at: u64,
    },

    /// A record's System Use entries, as Rock Ridge records them, are
    /// malformed: an entry shorter than its fields, too many continuation
    /// areas, or a name longer than any file system's.
    BadSystemUse {
        /// The record's position.
        at: u64,
    },

    /// A Rock Ridge symlink's components are malformed, or its target is
    /// longer than 4 KiB.
    BadSymlink {
        /// The record's position.
        at: u64,
    },

    /// A file that zisofs compresses is malformed: its header, its block
    /// pointers, or a block that does not decompress to its size.
    BadZisofs {
        /// The record's position.
        at: u64,
    },

    /// A file is interleaved, in units separated by gaps, which no current
    /// tool writes.
    Interleaved {
        /// The record's position.
        at: u64,
    },

    /// A file is sparse, as Rock Ridge 1.12 records it, which no current
    /// tool writes.
    Sparse {
        /// The record's position.
        at: u64,
    },

    /// A file is compressed otherwise than by zisofs's first version, or
    /// by zisofs while the `zisofs` feature is off.
    UnsupportedCompression {
        /// The record's position.
        at: u64,
    },
}

impl CdfsError {
    /// Returns the [`ErrorKind`] this inconsistency is reported as:
    /// [`Unsupported`](ErrorKind::Unsupported) for a file stored in a way
    /// this driver does not read, and [`Corrupt`](ErrorKind::Corrupt) for
    /// everything else.
    #[must_use]
    pub const fn kind(&self) -> ErrorKind {
        match self {
            Self::Interleaved { .. }
            | Self::Sparse { .. }
            | Self::UnsupportedCompression { .. } => ErrorKind::Unsupported,
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

impl core::fmt::Display for CdfsError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("ISO 9660 image: ")?;
        match self {
            Self::NotIso9660 => f.write_str("no volume descriptor at sector 16"),
            Self::NoPrimary => f.write_str("no primary volume descriptor"),
            Self::BadBlockSize(size) => write!(f, "bad logical block size {size}"),
            Self::BadRoot => f.write_str("bad root directory"),
            Self::BadDirectory { at } => write!(f, "bad records in the directory at {at}"),
            Self::BadRecord { at } => write!(f, "bad directory record at {at}"),
            Self::BadSystemUse { at } => write!(f, "bad System Use entries of the record at {at}"),
            Self::BadSymlink { at } => write!(f, "bad symlink target of the record at {at}"),
            Self::BadZisofs { at } => write!(f, "bad zisofs data of the record at {at}"),
            Self::Interleaved { at } => write!(f, "interleaved file at {at}"),
            Self::Sparse { at } => write!(f, "sparse file at {at}"),
            Self::UnsupportedCompression { at } => {
                write!(f, "unsupported compression of the file at {at}")
            },
        }
    }
}

impl core::error::Error for CdfsError {}

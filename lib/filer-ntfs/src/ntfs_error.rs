// This is free and unencumbered software released into the public domain.

use filer_api::{Error, ErrorKind, OpKind};

/// What is wrong with an NTFS volume: the source of the [`Error`] returned
/// when a volume cannot be opened or read.
///
/// Each variant names one inconsistency and carries the values involved,
/// so that callers can match on it and messages can say exactly what was
/// found.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum NtfsError {
    /// The source is shorter than a 512-byte boot sector.
    ShortBootSector,

    /// Bytes 510 and 511 of the boot sector are not `0x55 0xAA`.
    BadSignature,

    /// The boot sector's OEM identifier is not `NTFS    `.
    NotNtfs,

    /// The bytes per sector are not 512, 1024, 2048, or 4096.
    BadSectorSize(u16),

    /// The sectors per cluster do not give a power-of-two cluster of at
    /// most 2 MiB.
    BadClusterSize(u8),

    /// The volume has fewer sectors than one cluster.
    NoClusters,

    /// The cluster of `$MFT` lies outside the volume.
    BadMftCluster(u64),

    /// The cluster of `$MFTMirr` lies outside the volume.
    BadMftMirrorCluster(u64),

    /// The encoded size of an MFT record does not give a power of two
    /// from 512 bytes to 64 KiB.
    BadRecordSize(i8),

    /// The encoded size of an index record does not give a power of two
    /// from 512 bytes to 64 KiB.
    BadIndexRecordSize(i8),

    /// An MFT record is not one: its signature, header, or own number is
    /// wrong.
    BadMftRecord {
        /// The record's number.
        record: u64,
    },

    /// An MFT record's update sequence does not match: it was torn.
    TornRecord {
        /// The record's number.
        record: u64,
    },

    /// An attribute of an MFT record does not fit it.
    BadAttribute {
        /// The record's number.
        record: u64,
        /// The attribute's offset in the record.
        offset: u32,
    },

    /// An attribute's data runs are malformed, lie outside the volume, or
    /// do not cover its clusters.
    BadRuns {
        /// The number of the file's base record.
        record: u64,
    },

    /// A non-resident attribute's sizes are inconsistent: initialized past
    /// its size, or its size past what it has allocated or mapped.
    BadDataSize {
        /// The number of the file's base record.
        record: u64,
    },

    /// A file's `$ATTRIBUTE_LIST` is malformed.
    BadAttributeList {
        /// The number of the file's base record.
        record: u64,
    },

    /// An extension record of a file is not one of its own.
    BadExtension {
        /// The number of the file's base record.
        record: u64,
        /// The number of the extension record.
        extension: u64,
    },

    /// A reference to an MFT record names no file: past the MFT, not in
    /// use, an extension record, or of another sequence number.
    BadReference {
        /// The record's number.
        record: u64,
    },

    /// A file lacks an attribute every file of its kind has.
    MissingAttribute {
        /// The record's number.
        record: u64,
        /// The attribute's type.
        kind: u32,
    },

    /// A directory's index root is malformed.
    BadIndex {
        /// The directory's record number.
        record: u64,
    },

    /// An index record (`INDX`) of a directory is malformed, torn, or not
    /// the one its VCN names.
    BadIndexRecord {
        /// The directory's record number.
        record: u64,
        /// The index record's VCN.
        vcn: u64,
    },

    /// A directory's index reaches one of its nodes twice: it loops.
    IndexLoop {
        /// The directory's record number.
        record: u64,
        /// The VCN reached twice.
        vcn: u64,
    },

    /// A file's `$REPARSE_POINT` is malformed, or too large.
    BadReparse {
        /// The record's number.
        record: u64,
    },

    /// A `$FILE_NAME` attribute is malformed.
    BadFileName {
        /// The record's number.
        record: u64,
    },

    /// A named data stream's name contains NUL.
    BadStreamName {
        /// The record's number.
        record: u64,
    },

    /// `$UpCase` is not a table of 65,536 units.
    BadUpCase,

    /// The volume is marked dirty: its `$LogFile` was not replayed, so it
    /// may be inconsistent. Opening it needs
    /// [`Options::allow_dirty`](crate::Options::allow_dirty).
    Dirty,

    /// The volume's NTFS version is not 1.x or 3.x.
    UnsupportedVersion {
        /// The major version.
        major: u8,
        /// The minor version.
        minor: u8,
    },

    /// The volume is larger than its source.
    Truncated {
        /// The volume's size in bytes, per its boot sector.
        volume: u64,
        /// The source's size in bytes.
        source: u64,
    },
}

impl NtfsError {
    /// Returns the [`ErrorKind`] this inconsistency is reported as:
    /// [`Unsupported`](ErrorKind::Unsupported) for a dirty volume or an
    /// unknown version, and [`Corrupt`](ErrorKind::Corrupt) for everything
    /// else.
    #[must_use]
    pub const fn kind(&self) -> ErrorKind {
        match self {
            Self::Dirty | Self::UnsupportedVersion { .. } => ErrorKind::Unsupported,
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

impl core::fmt::Display for NtfsError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("NTFS volume: ")?;
        match self {
            Self::ShortBootSector => f.write_str("shorter than a boot sector"),
            Self::BadSignature => f.write_str("no boot sector signature"),
            Self::NotNtfs => f.write_str("not NTFS"),
            Self::BadSectorSize(size) => write!(f, "invalid sector size {size}"),
            Self::BadClusterSize(code) => write!(f, "invalid cluster size code {code:#04X}"),
            Self::NoClusters => f.write_str("no clusters"),
            Self::BadMftCluster(lcn) => write!(f, "$MFT at invalid cluster {lcn}"),
            Self::BadMftMirrorCluster(lcn) => write!(f, "$MFTMirr at invalid cluster {lcn}"),
            Self::BadRecordSize(code) => write!(f, "invalid MFT record size code {code}"),
            Self::BadIndexRecordSize(code) => {
                write!(f, "invalid index record size code {code}")
            },
            Self::BadMftRecord { record } => write!(f, "MFT record {record} is invalid"),
            Self::TornRecord { record } => write!(f, "MFT record {record} is torn"),
            Self::BadAttribute { record, offset } => {
                write!(
                    f,
                    "MFT record {record} has an invalid attribute at {offset}"
                )
            },
            Self::BadRuns { record } => write!(f, "file {record} has invalid data runs"),
            Self::BadDataSize { record } => {
                write!(f, "file {record} has inconsistent data sizes")
            },
            Self::BadAttributeList { record } => {
                write!(f, "file {record} has an invalid attribute list")
            },
            Self::BadExtension { record, extension } => {
                write!(
                    f,
                    "file {record} lists record {extension}, not its extension"
                )
            },
            Self::BadReference { record } => {
                write!(f, "reference to record {record}, which is no file")
            },
            Self::MissingAttribute { record, kind } => {
                write!(f, "file {record} lacks attribute {kind:#X}")
            },
            Self::BadIndex { record } => write!(f, "directory {record} has an invalid index"),
            Self::BadIndexRecord { record, vcn } => {
                write!(
                    f,
                    "directory {record} has an invalid index record at VCN {vcn}"
                )
            },
            Self::IndexLoop { record, vcn } => {
                write!(f, "directory {record}'s index loops at VCN {vcn}")
            },
            Self::BadFileName { record } => write!(f, "file {record} has an invalid name"),
            Self::BadStreamName { record } => {
                write!(f, "file {record} has a stream with an invalid name")
            },
            Self::BadReparse { record } => {
                write!(f, "file {record} has an invalid reparse point")
            },
            Self::BadUpCase => f.write_str("invalid $UpCase table"),
            Self::Dirty => f.write_str("marked dirty: its log was not replayed"),
            Self::UnsupportedVersion { major, minor } => {
                write!(f, "unsupported NTFS version {major}.{minor}")
            },
            Self::Truncated { volume, source } => {
                write!(f, "volume of {volume} bytes in a source of {source}")
            },
        }
    }
}

impl core::error::Error for NtfsError {}

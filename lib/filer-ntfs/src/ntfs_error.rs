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
    /// [`Corrupt`](ErrorKind::Corrupt).
    #[must_use]
    pub const fn kind(&self) -> ErrorKind {
        ErrorKind::Corrupt
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
            Self::Truncated { volume, source } => {
                write!(f, "volume of {volume} bytes in a source of {source}")
            },
        }
    }
}

impl core::error::Error for NtfsError {}

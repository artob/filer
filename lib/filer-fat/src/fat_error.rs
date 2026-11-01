// This is free and unencumbered software released into the public domain.

use filer_api::{Error, ErrorKind, OpKind};

/// What is wrong with a FAT volume: the source of the [`Error`] returned
/// when a volume cannot be opened or read.
///
/// Each variant names one inconsistency and carries the values involved,
/// so that callers can match on it and messages can say exactly what was
/// found.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FatError {
    /// The source is shorter than a 512-byte boot sector.
    ShortBootSector,

    /// Bytes 510 and 511 of the boot sector are not `0x55 0xAA`.
    BadSignature,

    /// The bytes per sector are not 512, 1024, 2048, or 4096.
    BadSectorSize(u16),

    /// The sectors per cluster are not a power of two up to 128.
    BadClusterSize(u8),

    /// The volume has no reserved sectors; the boot sector is one.
    NoReservedSectors,

    /// The volume has no FAT.
    NoFats,

    /// The media descriptor is not `0xF0` or `0xF8` to `0xFF`.
    BadMedia(u8),

    /// Both total-sector counts are zero.
    NoTotalSectors,

    /// Both FAT-size fields are zero.
    NoFatSize,

    /// The reserved sectors, FATs, and root directory do not fit in the
    /// volume, or their sizes overflow.
    BadLayout,

    /// The data region holds no cluster.
    NoClusters,

    /// The FAT is too small for the volume's clusters.
    FatTooSmall {
        /// The bytes the clusters need.
        needed: u64,
        /// The bytes each FAT has.
        actual: u64,
    },

    /// A FAT12/16 volume has no root directory entries, or a FAT32 volume
    /// has some, or a FAT32 volume has a 16-bit FAT size.
    BadRootEntries,

    /// The FAT32 root directory's first cluster is not a data cluster.
    BadRootCluster(u32),

    /// The FAT32 active FAT does not exist.
    BadActiveFat(u8),

    /// The FAT32 version is not 0.0, the only one defined.
    UnsupportedVersion(u16),

    /// The volume is larger than its source.
    Truncated {
        /// The volume's size in bytes, per its boot sector.
        volume: u64,
        /// The source's size in bytes.
        source: u64,
    },
}

impl FatError {
    /// Returns the [`ErrorKind`] this inconsistency is reported as:
    /// [`Unsupported`](ErrorKind::Unsupported) for an unknown FAT32
    /// version, and [`Corrupt`](ErrorKind::Corrupt) for everything else.
    #[must_use]
    pub const fn kind(&self) -> ErrorKind {
        match self {
            Self::UnsupportedVersion(_) => ErrorKind::Unsupported,
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

impl core::fmt::Display for FatError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("FAT volume: ")?;
        match self {
            Self::ShortBootSector => f.write_str("shorter than a boot sector"),
            Self::BadSignature => f.write_str("no boot sector signature"),
            Self::BadSectorSize(size) => write!(f, "invalid sector size {size}"),
            Self::BadClusterSize(sectors) => {
                write!(f, "invalid cluster size of {sectors} sectors")
            },
            Self::NoReservedSectors => f.write_str("no reserved sectors"),
            Self::NoFats => f.write_str("no FAT"),
            Self::BadMedia(media) => write!(f, "invalid media descriptor {media:#04X}"),
            Self::NoTotalSectors => f.write_str("no sector count"),
            Self::NoFatSize => f.write_str("no FAT size"),
            Self::BadLayout => f.write_str("regions do not fit in the volume"),
            Self::NoClusters => f.write_str("no data clusters"),
            Self::FatTooSmall { needed, actual } => {
                write!(f, "FAT of {actual} bytes too small, needs {needed}")
            },
            Self::BadRootEntries => f.write_str("root directory size inconsistent with FAT type"),
            Self::BadRootCluster(cluster) => write!(f, "invalid root cluster {cluster}"),
            Self::BadActiveFat(fat) => write!(f, "active FAT {fat} does not exist"),
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported FAT32 version {version:#06X}")
            },
            Self::Truncated { volume, source } => {
                write!(f, "volume of {volume} bytes in a source of {source}")
            },
        }
    }
}

impl core::error::Error for FatError {}

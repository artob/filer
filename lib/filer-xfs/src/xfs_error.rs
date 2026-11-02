// This is free and unencumbered software released into the public domain.

use filer_api::{Error, ErrorKind, OpKind};

/// What is wrong with an XFS volume: the source of the [`Error`] returned
/// when a volume cannot be opened or read.
///
/// Each variant names one inconsistency and carries the values involved,
/// so that callers can match on it and messages can say exactly what was
/// found.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum XfsError {
    /// The source is too short to hold a superblock, which fills the first
    /// sector.
    ShortSuperblock,

    /// The superblock's magic number is not `XFSB` (`0x58465342`): this is
    /// not an XFS volume.
    BadMagic(u32),

    /// The version in the low four bits of `sb_versionnum` is neither 4
    /// nor 5: version 1 to 3 volumes predate every tool that still runs.
    UnsupportedVersion(u8),

    /// The block size is not a power of two from 512 bytes (1 KiB for
    /// version 5) to 64 KiB, or disagrees with its logarithm.
    BadBlockSize(u32),

    /// The sector size is not a power of two from 512 bytes to the block
    /// size, or disagrees with its logarithm.
    BadSectorSize(u16),

    /// The inode size is not a power of two from 256 bytes (512 for
    /// version 5) to 2 KiB and the block size, or disagrees with its
    /// logarithm or the inodes per block.
    BadInodeSize(u16),

    /// The allocation group size is zero, more than 2^31 blocks, or
    /// disagrees with its logarithm.
    BadAgSize(u32),

    /// The allocation group count is zero, or too large for inode and
    /// block numbers to address.
    BadAgCount(u32),

    /// The volume's block count is not within its last allocation group.
    BadBlockCount(u64),

    /// The directory block size, 2^`sb_dirblklog` blocks, exceeds 64 KiB.
    BadDirBlockSize(u8),

    /// The internal log lies outside the volume.
    BadLog,

    /// A reference to an inode is out of range: its allocation group,
    /// block, or slot does not exist.
    BadInodeNumber(u64),

    /// A reference to an inode names one that is not in use.
    UnusedInode {
        /// The inode's number.
        inode: u64,
    },

    /// An inode is malformed: a wrong magic number or version, an unknown
    /// file type, a negative size, a time's nanoseconds of a second or
    /// more, a fork format that does not fit its kind, or a fork larger
    /// than the inode.
    BadInode {
        /// The inode's number.
        inode: u64,
    },

    /// A file's map names a block outside the volume.
    BadBlockNumber {
        /// The file's inode.
        inode: u64,
        /// The block named, its AG above the AG block.
        block: u64,
    },

    /// A file's map is malformed: its extents are empty, out of order,
    /// overlapping, past the largest file, or cross an allocation group;
    /// or a B+ tree block has the wrong magic number, level, or record
    /// count, or keys out of order.
    BadMap {
        /// The file's inode.
        inode: u64,
    },

    /// A file's data is on the realtime device, which this driver does not
    /// read.
    RealtimeData {
        /// The file's inode.
        inode: u64,
    },

    /// A directory's entry is malformed: its name is empty or holds `/` or
    /// NUL, its type is unknown, it runs past its block or the inode, its
    /// tag is not its offset, its offset does not ascend, or it names inode
    /// 0.
    BadDirEntry {
        /// The directory's inode.
        dir: u64,
        /// The entry's directory offset.
        offset: u64,
    },

    /// A directory's data block is malformed: the wrong magic number, more
    /// hash leaves than fit, or entries not filling it.
    BadDirBlock {
        /// The directory's inode.
        dir: u64,
        /// The block's directory offset.
        offset: u64,
    },

    /// A directory's size is not a whole number of directory blocks, or
    /// more than its data blocks or the volume can hold.
    BadDirectorySize {
        /// The directory's inode.
        dir: u64,
        /// Its size in bytes.
        size: u64,
    },

    /// A symlink's target is empty, longer than 1024 bytes, or holds NUL.
    BadSymlink {
        /// The symlink's inode.
        inode: u64,
    },

    /// `mkfs.xfs` did not finish making the volume.
    Unfinished,

    /// The volume lacks features every volume this driver reads has:
    /// 32-bit link counts, unwritten extent flags, and version 2
    /// directories (`NLINK`, `EXTFLG`, and `DIRV2` in `sb_versionnum`).
    MissingFeatures(u16),

    /// The volume uses features of `set` this driver does not read.
    UnsupportedFeatures {
        /// The set the features belong to.
        set: FeatureSet,
        /// The feature flags not understood.
        bits: u32,
    },

    /// The volume is larger than its source.
    Truncated {
        /// The volume's size in bytes, per its superblock.
        volume: u64,
        /// The source's size in bytes.
        source: u64,
    },
}

/// A set of feature flags in the superblock, as named in
/// [`XfsError::UnsupportedFeatures`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FeatureSet {
    /// Version 4 features: the bits of `sb_versionnum` above the version.
    Version,
    /// More version 4 features, in `sb_features2`.
    Features2,
    /// Version 5 incompatible features, in `sb_features_incompat`.
    Incompat,
    /// Version 5 log-incompatible features, in
    /// `sb_features_log_incompat`.
    LogIncompat,
}

impl XfsError {
    /// Returns the [`ErrorKind`] this inconsistency is reported as:
    /// [`Unsupported`](ErrorKind::Unsupported) for an unknown version or
    /// feature, a missing one, or data on the realtime device, and
    /// [`Corrupt`](ErrorKind::Corrupt) for everything else.
    #[must_use]
    pub const fn kind(&self) -> ErrorKind {
        match self {
            Self::UnsupportedVersion(_)
            | Self::MissingFeatures(_)
            | Self::UnsupportedFeatures { .. }
            | Self::RealtimeData { .. } => ErrorKind::Unsupported,
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

impl core::fmt::Display for XfsError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("XFS volume: ")?;
        match self {
            Self::ShortSuperblock => f.write_str("shorter than a superblock"),
            Self::BadMagic(magic) => write!(f, "not XFS (magic {magic:#010X})"),
            Self::UnsupportedVersion(version) => write!(f, "unsupported version {version}"),
            Self::BadBlockSize(size) => write!(f, "invalid block size {size}"),
            Self::BadSectorSize(size) => write!(f, "invalid sector size {size}"),
            Self::BadInodeSize(size) => write!(f, "invalid inode size {size}"),
            Self::BadAgSize(blocks) => {
                write!(f, "invalid allocation group size of {blocks} blocks")
            },
            Self::BadAgCount(count) => write!(f, "invalid allocation group count {count}"),
            Self::BadBlockCount(blocks) => write!(f, "invalid block count {blocks}"),
            Self::BadDirBlockSize(log) => {
                write!(f, "invalid directory block size exponent {log}")
            },
            Self::BadLog => f.write_str("its log lies outside the volume"),
            Self::BadInodeNumber(inode) => write!(f, "reference to invalid inode {inode}"),
            Self::UnusedInode { inode } => {
                write!(f, "reference to inode {inode}, which is not in use")
            },
            Self::BadInode { inode } => write!(f, "inode {inode} is invalid"),
            Self::BadBlockNumber { inode, block } => {
                write!(f, "inode {inode} maps block {block}, outside the volume")
            },
            Self::BadMap { inode } => write!(f, "inode {inode} has an invalid map"),
            Self::RealtimeData { inode } => {
                write!(f, "inode {inode} has its data on the realtime device")
            },
            Self::BadDirEntry { dir, offset } => {
                write!(f, "directory {dir} has an invalid entry at {offset}")
            },
            Self::BadDirBlock { dir, offset } => {
                write!(f, "directory {dir} has an invalid block at {offset}")
            },
            Self::BadDirectorySize { dir, size } => {
                write!(f, "directory {dir} has an invalid size {size}")
            },
            Self::BadSymlink { inode } => write!(f, "symlink {inode} has an invalid target"),
            Self::Unfinished => f.write_str("mkfs.xfs did not finish making it"),
            Self::MissingFeatures(bits) => {
                write!(f, "lacks required version features {bits:#06X}")
            },
            Self::UnsupportedFeatures { set, bits } => {
                let set = match set {
                    FeatureSet::Version => "version",
                    FeatureSet::Features2 => "features2",
                    FeatureSet::Incompat => "incompatible",
                    FeatureSet::LogIncompat => "log-incompatible",
                };
                write!(f, "unsupported {set} features {bits:#X}")
            },
            Self::Truncated { volume, source } => {
                write!(f, "volume of {volume} bytes in a source of {source}")
            },
        }
    }
}

impl core::error::Error for XfsError {}

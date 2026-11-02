// This is free and unencumbered software released into the public domain.

use filer_api::{Error, ErrorKind, OpKind};

/// What is wrong with an ext2, ext3, or ext4 volume: the source of the
/// [`Error`] returned when a volume cannot be opened or read.
///
/// Each variant names one inconsistency and carries the values involved,
/// so that callers can match on it and messages can say exactly what was
/// found.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Ext4Error {
    /// The source is too short to hold a superblock, which ends at byte
    /// 2048.
    ShortSuperblock,

    /// The superblock's magic number is not `0xEF53`: this is not an ext2,
    /// ext3, or ext4 volume.
    BadMagic(u16),

    /// The revision level is neither 0 (original) nor 1 (dynamic).
    UnsupportedRevision(u32),

    /// The block size exponent does not give a block of 1 KiB to 64 KiB.
    BadBlockSize(u32),

    /// The first data block is not the superblock's block: 1 for 1 KiB
    /// blocks, else 0.
    BadFirstDataBlock(u32),

    /// The blocks per group are zero, or more than one block's bitmap
    /// tracks.
    BadBlocksPerGroup(u32),

    /// The inodes per group are zero, or more than one block's bitmap
    /// tracks.
    BadInodesPerGroup(u32),

    /// The volume has no blocks past its first data block.
    NoBlocks,

    /// The inode count is below the root directory's number, or more than
    /// the groups hold.
    BadInodeCount(u32),

    /// The inode size is not a power of two from 128 bytes to the block
    /// size.
    BadInodeSize(u16),

    /// The group descriptors of this many groups do not fit in the volume.
    BadGroupCount(u32),

    /// A group's descriptor is malformed: its inode table lies outside the
    /// volume, or the group does not exist.
    BadGroupDescriptor {
        /// The group's number.
        group: u32,
    },

    /// A reference to an inode is out of range: 0, or past the inode count.
    BadInodeNumber(u32),

    /// A reference to an inode names one that is not in use.
    UnusedInode {
        /// The inode's number.
        inode: u32,
    },

    /// An inode is malformed: an unknown file type, extra fields larger
    /// than its record, or a time's nanoseconds of a second or more.
    BadInode {
        /// The inode's number.
        inode: u32,
    },

    /// A file's map names a block outside the volume, or the superblock's.
    BadBlockNumber {
        /// The file's inode.
        inode: u32,
        /// The block named.
        block: u64,
    },

    /// A file's size is more than its map can address.
    FileTooLarge {
        /// The file's inode.
        inode: u32,
    },

    /// A directory's entry is malformed: its record length is not a
    /// multiple of four, too short for its name, or past its block; its
    /// name is empty, longer than 255 bytes, or holds `/` or NUL; or its
    /// kind is unknown.
    BadDirEntry {
        /// The directory's inode.
        dir: u32,
        /// The entry's offset in the directory.
        offset: u64,
    },

    /// A directory's size is not a whole number of blocks.
    BadDirectorySize {
        /// The directory's inode.
        dir: u32,
        /// Its size in bytes.
        size: u64,
    },

    /// A symlink's target is empty, longer than a block, or holds NUL.
    BadSymlink {
        /// The symlink's inode.
        inode: u32,
    },

    /// A file's extended attributes are malformed: an entry or value lies
    /// past its region, a POSIX ACL is invalid, or the block's header is
    /// wrong.
    BadXattr {
        /// The file's inode.
        inode: u32,
    },

    /// The volume uses incompatible features this driver does not read.
    UnsupportedFeatures {
        /// The incompatible feature flags not understood.
        incompat: u32,
    },

    /// The volume's journal holds changes not yet written to it, so it may
    /// be inconsistent as it is on disk.
    NeedsRecovery,

    /// The volume is larger than its source.
    Truncated {
        /// The volume's size in bytes, per its superblock.
        volume: u64,
        /// The source's size in bytes.
        source: u64,
    },
}

impl Ext4Error {
    /// Returns the [`ErrorKind`] this inconsistency is reported as:
    /// [`Unsupported`](ErrorKind::Unsupported) for an unknown revision or
    /// feature, or a journal needing recovery, and
    /// [`Corrupt`](ErrorKind::Corrupt) for everything else.
    #[must_use]
    pub const fn kind(&self) -> ErrorKind {
        match self {
            Self::UnsupportedRevision(_)
            | Self::UnsupportedFeatures { .. }
            | Self::NeedsRecovery => ErrorKind::Unsupported,
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

impl core::fmt::Display for Ext4Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("ext2/3/4 volume: ")?;
        match self {
            Self::ShortSuperblock => f.write_str("shorter than a superblock"),
            Self::BadMagic(magic) => write!(f, "not ext2/3/4 (magic {magic:#06X})"),
            Self::UnsupportedRevision(level) => write!(f, "unsupported revision {level}"),
            Self::BadBlockSize(log) => write!(f, "invalid block size exponent {log}"),
            Self::BadFirstDataBlock(block) => write!(f, "invalid first data block {block}"),
            Self::BadBlocksPerGroup(count) => write!(f, "invalid blocks per group {count}"),
            Self::BadInodesPerGroup(count) => write!(f, "invalid inodes per group {count}"),
            Self::NoBlocks => f.write_str("no data blocks"),
            Self::BadInodeCount(count) => write!(f, "invalid inode count {count}"),
            Self::BadInodeSize(size) => write!(f, "invalid inode size {size}"),
            Self::BadGroupCount(groups) => {
                write!(f, "descriptors of {groups} groups do not fit the volume")
            },
            Self::BadGroupDescriptor { group } => {
                write!(f, "group {group} has an invalid descriptor")
            },
            Self::BadInodeNumber(inode) => write!(f, "reference to invalid inode {inode}"),
            Self::UnusedInode { inode } => {
                write!(f, "reference to inode {inode}, which is not in use")
            },
            Self::BadInode { inode } => write!(f, "inode {inode} is invalid"),
            Self::BadBlockNumber { inode, block } => {
                write!(f, "inode {inode} maps block {block}, outside the volume")
            },
            Self::FileTooLarge { inode } => {
                write!(f, "inode {inode} is larger than its map addresses")
            },
            Self::BadDirEntry { dir, offset } => {
                write!(f, "directory {dir} has an invalid entry at {offset}")
            },
            Self::BadDirectorySize { dir, size } => {
                write!(f, "directory {dir} has an invalid size {size}")
            },
            Self::BadSymlink { inode } => write!(f, "symlink {inode} has an invalid target"),
            Self::BadXattr { inode } => {
                write!(f, "inode {inode} has invalid extended attributes")
            },
            Self::UnsupportedFeatures { incompat } => {
                write!(f, "unsupported incompatible features {incompat:#X}")
            },
            Self::NeedsRecovery => f.write_str("its journal needs recovery"),
            Self::Truncated { volume, source } => {
                write!(f, "volume of {volume} bytes in a source of {source}")
            },
        }
    }
}

impl core::error::Error for Ext4Error {}

// This is free and unencumbered software released into the public domain.

use filer_api::{Error, ErrorKind, OpKind};

/// What is wrong with a SquashFS image: the source of the [`Error`]
/// returned when an image cannot be opened or read.
///
/// Each variant names one inconsistency and carries the values involved,
/// so that callers can match on it and messages can say exactly what was
/// found. Positions are byte offsets in the image; inodes are named by
/// their numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SquashfsError {
    /// The source is too short to hold a superblock, of 96 bytes.
    ShortSuperblock,

    /// The superblock's magic number is not `hsqs`: this is not a
    /// SquashFS image, or not a little-endian one.
    BadMagic(u32),

    /// The version is not 4.0, the only one the Linux kernel still reads.
    UnsupportedVersion {
        /// The major version.
        major: u16,
        /// The minor version.
        minor: u16,
    },

    /// The compressor is not one this driver knows, or one its features
    /// leave out (`gzip`, `lzma`, `lzo`, `xz`, `lz4`, and `zstd`, by
    /// their ids 1 to 6).
    UnsupportedCompressor(u16),

    /// The block size is not a power of two from 4 KiB to 1 MiB, or not
    /// the one its logarithm gives.
    BadBlockSize {
        /// The block size.
        size: u32,
        /// Its base-2 logarithm, as the superblock gives it.
        log: u16,
    },

    /// The superblock's tables are out of order, or past the image's end.
    BadTables,

    /// The image is larger than the source holding it.
    Truncated {
        /// The image's size in bytes, per its superblock.
        image: u64,
        /// The source's size in bytes.
        source: u64,
    },

    /// A metadata block is malformed: of no bytes or more than 8 KiB, past
    /// the end of its table, or not decompressing to at most 8 KiB.
    BadMetadataBlock {
        /// The block's position.
        at: u64,
    },

    /// The root directory's reference is out of range, or names an inode
    /// that is not a directory.
    BadRoot,

    /// An inode is malformed: of an unknown type, or with a field out of
    /// range, such as a directory listing past the directory table.
    BadInode {
        /// Its reference: its metadata block's position in the inode
        /// table, shifted 16 bits up, and its offset in that block.
        reference: u64,
    },

    /// A directory's listing is malformed: a header of more than 256
    /// entries, an entry of an unknown type or an invalid name, or a
    /// listing that ends inside an entry.
    BadDirectory {
        /// The directory's inode number.
        inode: u32,
    },

    /// A symlink's target is empty or longer than 4096 bytes.
    BadSymlink {
        /// The symlink's inode number.
        inode: u32,
    },

    /// A block of a file is malformed: its size has unknown bits set,
    /// lies past the image's end, or does not decompress to the block's
    /// length.
    BadDataBlock {
        /// The file's inode number.
        inode: u32,
        /// The block's index in the file.
        block: u64,
    },

    /// A fragment's entry is out of range, or its block is malformed.
    BadFragment {
        /// The fragment's index.
        index: u32,
    },

    /// A file's extended attributes are malformed: an index past the
    /// table, an unknown namespace, or an invalid name or value.
    BadXattr {
        /// The file's inode number.
        inode: u32,
    },
}

impl SquashfsError {
    /// Returns the [`ErrorKind`] this inconsistency is reported as:
    /// [`Unsupported`](ErrorKind::Unsupported) for an unknown version or
    /// compressor, and [`Corrupt`](ErrorKind::Corrupt) for everything
    /// else.
    #[must_use]
    pub const fn kind(&self) -> ErrorKind {
        match self {
            Self::UnsupportedVersion { .. } | Self::UnsupportedCompressor(_) => {
                ErrorKind::Unsupported
            },
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

impl core::fmt::Display for SquashfsError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("SquashFS image: ")?;
        match self {
            Self::ShortSuperblock => f.write_str("too short for a superblock"),
            Self::BadMagic(magic) => write!(f, "bad magic number {magic:#010x}"),
            Self::UnsupportedVersion { major, minor } => {
                write!(f, "unsupported version {major}.{minor}")
            },
            Self::UnsupportedCompressor(id) => write!(f, "unsupported compressor {id}"),
            Self::BadBlockSize { size, log } => {
                write!(f, "bad block size {size} (logarithm {log})")
            },
            Self::BadTables => f.write_str("tables out of order or past the end"),
            Self::Truncated { image, source } => {
                write!(f, "image of {image} bytes in a source of {source}")
            },
            Self::BadMetadataBlock { at } => write!(f, "bad metadata block at {at}"),
            Self::BadRoot => f.write_str("bad root directory"),
            Self::BadInode { reference } => write!(f, "bad inode at {reference:#x}"),
            Self::BadDirectory { inode } => write!(f, "bad listing of directory {inode}"),
            Self::BadSymlink { inode } => write!(f, "bad target of symlink {inode}"),
            Self::BadDataBlock { inode, block } => {
                write!(f, "bad block {block} of file {inode}")
            },
            Self::BadFragment { index } => write!(f, "bad fragment {index}"),
            Self::BadXattr { inode } => write!(f, "bad extended attributes of inode {inode}"),
        }
    }
}

impl core::error::Error for SquashfsError {}

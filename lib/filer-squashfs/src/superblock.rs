// This is free and unencumbered software released into the public domain.

//! The superblock: the image's first 96 bytes, little-endian, which say
//! how blocks are compressed and where each table is.

use crate::{SquashfsError, codec::Compressor};
use filer_api::{Error, OpKind, ReadAt};
use filer_fs::le;

/// The superblock's size.
const SIZE: usize = 96;

/// `hsqs`, little-endian.
const MAGIC: u32 = 0x7371_7368;

/// What marks a table as absent.
pub(crate) const NONE: u64 = u64::MAX;

/// The flag that the image has no extended attributes.
const NO_XATTRS: u16 = 0x0200;

/// A validated superblock.
#[derive(Clone, Debug)]
pub(crate) struct Superblock {
    pub(crate) inodes: u32,
    pub(crate) mkfs_time: u32,
    pub(crate) block_size: u32,
    pub(crate) fragments: u32,
    pub(crate) compressor: Compressor,
    pub(crate) root: u64,
    /// The image's size, all of which the source holds.
    pub(crate) bytes_used: u64,
    pub(crate) xattr_table: u64,
    pub(crate) inode_table: u64,
    pub(crate) dir_table: u64,
    pub(crate) fragment_table: u64,
}

impl Superblock {
    /// Returns the id of the volume, for its files' ids: SquashFS has no
    /// UUID, so its time of making, size, and inode count, folded.
    pub(crate) fn volume_id(&self) -> u64 {
        let time = u64::from(self.mkfs_time) << 32;
        time ^ self.bytes_used ^ u64::from(self.inodes).rotate_left(16)
    }

    /// Returns where the extended attributes' id table is, if the image
    /// has one.
    pub(crate) fn xattrs(&self) -> Option<u64> {
        (self.xattr_table != NONE).then_some(self.xattr_table)
    }
}

/// Reads and validates the superblock of the image in `source`.
pub(crate) async fn read<S: ReadAt>(source: &S) -> Result<Superblock, Error> {
    let op = OpKind::Open;
    let size = source.size().await?;
    let bad = |error: SquashfsError| error.into_error(op);
    if size < 96 {
        return Err(bad(SquashfsError::ShortSuperblock));
    }
    let raw = filer_fs::read_exact(source, 0, SIZE, op).await?;
    let sb = parse(&raw).map_err(bad)?;
    if sb.bytes_used > size {
        return Err(bad(SquashfsError::Truncated {
            image: sb.bytes_used,
            source: size,
        }));
    }
    Ok(sb)
}

/// Parses and validates a superblock.
pub(crate) fn parse(raw: &[u8]) -> Result<Superblock, SquashfsError> {
    // `read` reads it whole; a shorter one reads as zeros past its end.
    let u16_at = |at| le::u16_at(raw, at).unwrap_or(0);
    let u32_at = |at| le::u32_at(raw, at).unwrap_or(0);
    let u64_at = |at| le::u64_at(raw, at).unwrap_or(0);
    let magic = u32_at(0);
    if magic != MAGIC {
        return Err(SquashfsError::BadMagic(magic));
    }
    let (major, minor) = (u16_at(28), u16_at(30));
    if (major, minor) != (4, 0) {
        return Err(SquashfsError::UnsupportedVersion { major, minor });
    }
    let id = u16_at(20);
    let compressor = Compressor::from_id(id).ok_or(SquashfsError::UnsupportedCompressor(id))?;
    let (block_size, log) = (u32_at(12), u16_at(22));
    let valid = (12..=20).contains(&log) && 1_u32.checked_shl(u32::from(log)) == Some(block_size);
    if !valid {
        return Err(SquashfsError::BadBlockSize {
            size: block_size,
            log,
        });
    }
    let flags = u16_at(24);
    let sb = Superblock {
        inodes: u32_at(4),
        mkfs_time: u32_at(8),
        block_size,
        fragments: u32_at(16),
        compressor,
        root: u64_at(32),
        bytes_used: u64_at(40),
        xattr_table: if flags & NO_XATTRS != 0 {
            NONE
        } else {
            u64_at(56)
        },
        inode_table: u64_at(64),
        dir_table: u64_at(72),
        fragment_table: u64_at(80),
    };
    // The inode table comes before the directory table, and both, and
    // every table present, after the superblock and within the image.
    let id_table = u64_at(48);
    let within = |at: u64| (96..sb.bytes_used).contains(&at);
    let present = [sb.fragment_table, sb.xattr_table, u64_at(88)]
        .into_iter()
        .all(|at| at == NONE || within(at));
    let ordered = sb.inode_table < sb.dir_table && within(sb.inode_table) && within(sb.dir_table);
    if !(present && ordered && within(id_table) && u16_at(26) > 0) {
        return Err(SquashfsError::BadTables);
    }
    if sb.fragments > 0 && sb.fragment_table == NONE {
        return Err(SquashfsError::BadTables);
    }
    Ok(sb)
}

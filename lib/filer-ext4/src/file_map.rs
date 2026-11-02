// This is free and unencumbered software released into the public domain.

//! A file's map of its blocks: a block map, as ext2 and ext3 keep, or an
//! extent tree, as ext4 does for inodes with the extents flag.

use crate::{
    Ext4Error, block_map::BlockMap, extent::ExtentTree, inode::Inode, superblock::Superblock,
};
use alloc::vec::Vec;
use core::num::NonZeroU64;
use filer_api::{Error, OpKind, ReadAt};
use filer_fs::map::{self, Mapping, Run};

/// The map of a file's blocks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum FileMap {
    Blocks(BlockMap),
    Extents(ExtentTree),
}

impl FileMap {
    /// Returns the map of `inode`, which must address every block of its
    /// size.
    pub(crate) fn new(inode: &Inode, sb: &Superblock) -> Result<Self, Ext4Error> {
        if inode.has_extents() {
            ExtentTree::new(inode, sb).map(Self::Extents)
        } else {
            BlockMap::new(inode, sb).map(Self::Blocks)
        }
    }

    /// Returns the inode whose map this is.
    pub(crate) const fn inode(&self) -> u32 {
        match self {
            Self::Blocks(map) => map.inode(),
            Self::Extents(tree) => tree.inode(),
        }
    }

    /// Maps the run of at most `max` blocks (at least 1) from file block
    /// `logical`.
    pub(crate) async fn map<S: ReadAt + ?Sized>(
        &self,
        source: &S,
        sb: &Superblock,
        logical: u64,
        max: u64,
        op: OpKind,
    ) -> Result<Run, Error> {
        match self {
            Self::Blocks(map) => map.map(source, sb, logical, max, op).await,
            Self::Extents(tree) => tree.map(source, sb, logical, max, op).await,
        }
    }

    /// Reads up to `len` bytes of the file's `size` from `off`: fewer only
    /// at its end. Holes, and extents not yet written, read as zeros.
    pub(crate) async fn read<S: ReadAt + ?Sized>(
        &self,
        source: &S,
        sb: &Superblock,
        size: u64,
        off: u64,
        len: usize,
        op: OpKind,
    ) -> Result<Vec<u8>, Error> {
        map::read(&OnVolume { map: self, sb }, source, size, off, len, op).await
    }
}

/// A file's map on its volume, as [`map::read`] reads through it.
struct OnVolume<'a> {
    map: &'a FileMap,
    sb: &'a Superblock,
}

impl Mapping for OnVolume<'_> {
    fn block_size(&self) -> NonZeroU64 {
        NonZeroU64::new(u64::from(self.sb.block_size())).unwrap_or(NonZeroU64::MIN)
    }

    fn offset(&self, block: u64) -> Option<u64> {
        self.sb.block_offset(block)
    }

    async fn map<S: ReadAt + ?Sized>(
        &self,
        source: &S,
        logical: u64,
        max: u64,
        op: OpKind,
    ) -> Result<Run, Error> {
        self.map.map(source, self.sb, logical, max, op).await
    }

    fn corrupt(&self, op: OpKind) -> Error {
        Ext4Error::FileTooLarge {
            inode: self.map.inode(),
        }
        .into_error(op)
    }
}

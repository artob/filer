// This is free and unencumbered software released into the public domain.

//! The superblock, and the volume layout it describes.

#![allow(dead_code, reason = "used by the driver, which comes later")]

use crate::Ext4Error;
use alloc::vec::Vec;
use filer_api::{BufResult, Error, ErrorKind, OpKind, ReadAt};

/// Where the superblock starts, and its size.
pub(crate) const SUPERBLOCK_OFFSET: u64 = 1024;
pub(crate) const SUPERBLOCK_SIZE: usize = 1024;

const MAGIC: u16 = 0xEF53;

/// The original revision, with 128-byte inodes and no feature sets.
const GOOD_OLD_REV: u32 = 0;
/// The dynamic revision, with `s_inode_size`, `s_first_ino`, and features.
const DYNAMIC_REV: u32 = 1;

/// The inode size of revision 0, and the smallest of revision 1.
pub(crate) const GOOD_OLD_INODE_SIZE: u16 = 128;

/// The root directory's inode.
pub(crate) const ROOT_INODE: u32 = 2;

/// The largest block size exponent: 2^(10 + 6) bytes, or 64 KiB.
const MAX_LOG_BLOCK_SIZE: u32 = 6;

/// The size of a group descriptor without the `64bit` feature.
pub(crate) const DESC_SIZE: u16 = 32;

/// The feature flags of the three sets, as far as this driver knows them.
pub(crate) mod feature {
    /// Incompatible: directory entries record the file type.
    pub(crate) const INCOMPAT_FILETYPE: u32 = 0x2;
    /// Incompatible: the journal holds changes not yet applied.
    pub(crate) const INCOMPAT_RECOVER: u32 = 0x4;
}

/// The incompatible features this driver reads.
const SUPPORTED_INCOMPAT: u32 = feature::INCOMPAT_FILETYPE;

/// A volume's feature sets. Revision 0 volumes have none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Features {
    /// Features a reader or writer may ignore.
    pub(crate) compat: u32,
    /// Features a reader must understand.
    pub(crate) incompat: u32,
    /// Features a reader may ignore, but a writer must not.
    pub(crate) ro_compat: u32,
}

/// A volume's layout and identity, from a validated superblock. Immutable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Superblock {
    revision: u32,
    log_block_size: u32,
    /// The number of blocks in the volume, including those before the
    /// first data block.
    blocks: u64,
    first_data_block: u32,
    blocks_per_group: u32,
    inodes: u32,
    inodes_per_group: u32,
    groups: u32,
    inode_size: u16,
    features: Features,
    uuid: [u8; 16],
}

fn u16_at(bytes: &[u8], at: usize) -> Result<u16, Ext4Error> {
    bytes
        .get(at..)
        .and_then(<[u8]>::first_chunk)
        .map(|b| u16::from_le_bytes(*b))
        .ok_or(Ext4Error::ShortSuperblock)
}

fn u32_at(bytes: &[u8], at: usize) -> Result<u32, Ext4Error> {
    bytes
        .get(at..)
        .and_then(<[u8]>::first_chunk)
        .map(|b| u32::from_le_bytes(*b))
        .ok_or(Ext4Error::ShortSuperblock)
}

impl Superblock {
    /// Parses and validates a superblock, of which the first 1024 bytes
    /// are read: its magic number, revision, geometry, and features,
    /// refusing incompatible features it does not know and a journal that
    /// needs recovery.
    pub(crate) fn parse(bytes: &[u8]) -> Result<Self, Ext4Error> {
        if bytes.len() < SUPERBLOCK_SIZE {
            return Err(Ext4Error::ShortSuperblock);
        }
        let magic = u16_at(bytes, 0x38)?;
        if magic != MAGIC {
            return Err(Ext4Error::BadMagic(magic));
        }
        let revision = u32_at(bytes, 0x4C)?;
        if revision > DYNAMIC_REV {
            return Err(Ext4Error::UnsupportedRevision(revision));
        }
        let log_block_size = u32_at(bytes, 0x18)?;
        if log_block_size > MAX_LOG_BLOCK_SIZE {
            return Err(Ext4Error::BadBlockSize(log_block_size));
        }
        let block_size = 1024u32 << log_block_size;
        // The superblock is at byte 1024, which is block 1 of 1 KiB blocks
        // and in block 0 of larger ones; data starts with its block.
        let first_data_block = u32_at(bytes, 0x14)?;
        if first_data_block != u32::from(log_block_size == 0) {
            return Err(Ext4Error::BadFirstDataBlock(first_data_block));
        }
        // One block's bitmap tracks each group's blocks and inodes.
        let bits = block_size.saturating_mul(8);
        let blocks_per_group = u32_at(bytes, 0x20)?;
        if blocks_per_group == 0 || blocks_per_group > bits {
            return Err(Ext4Error::BadBlocksPerGroup(blocks_per_group));
        }
        let inodes_per_group = u32_at(bytes, 0x28)?;
        if inodes_per_group == 0 || inodes_per_group > bits {
            return Err(Ext4Error::BadInodesPerGroup(inodes_per_group));
        }
        let blocks = u64::from(u32_at(bytes, 0x4)?);
        let data_blocks = blocks
            .checked_sub(u64::from(first_data_block))
            .filter(|&n| n > 0)
            .ok_or(Ext4Error::NoBlocks)?;
        let groups = data_blocks.div_ceil(u64::from(blocks_per_group));
        // At most 2^32 blocks, in groups of at least one block.
        let groups = u32::try_from(groups).map_err(|_| Ext4Error::NoBlocks)?;
        let inodes = u32_at(bytes, 0x0)?;
        let capacity = u64::from(groups).saturating_mul(u64::from(inodes_per_group));
        if inodes < ROOT_INODE || u64::from(inodes) > capacity {
            return Err(Ext4Error::BadInodeCount(inodes));
        }
        let (inode_size, features) = if revision == GOOD_OLD_REV {
            (GOOD_OLD_INODE_SIZE, Features::default())
        } else {
            let features = Features {
                compat: u32_at(bytes, 0x5C)?,
                incompat: u32_at(bytes, 0x60)?,
                ro_compat: u32_at(bytes, 0x64)?,
            };
            (u16_at(bytes, 0x58)?, features)
        };
        if !inode_size.is_power_of_two()
            || inode_size < GOOD_OLD_INODE_SIZE
            || u32::from(inode_size) > block_size
        {
            return Err(Ext4Error::BadInodeSize(inode_size));
        }
        let unknown = features.incompat & !(SUPPORTED_INCOMPAT | feature::INCOMPAT_RECOVER);
        if unknown != 0 {
            return Err(Ext4Error::UnsupportedFeatures { incompat: unknown });
        }
        if features.incompat & feature::INCOMPAT_RECOVER != 0 {
            return Err(Ext4Error::NeedsRecovery);
        }
        let mut uuid = [0; 16];
        if let Some(from) = bytes.get(0x68..0x78) {
            uuid.copy_from_slice(from);
        }
        Ok(Self {
            revision,
            log_block_size,
            blocks,
            first_data_block,
            blocks_per_group,
            inodes,
            inodes_per_group,
            groups,
            inode_size,
            features,
            uuid,
        })
    }

    /// Returns the revision level: 0 (original) or 1 (dynamic).
    pub(crate) const fn revision(&self) -> u32 {
        self.revision
    }

    /// Returns the block size in bytes: a power of two from 1 KiB to
    /// 64 KiB.
    pub(crate) const fn block_size(&self) -> u32 {
        1024 << self.log_block_size
    }

    /// Returns the number of blocks in the volume.
    pub(crate) const fn blocks(&self) -> u64 {
        self.blocks
    }

    /// Returns the first data block: the superblock's block.
    pub(crate) const fn first_data_block(&self) -> u32 {
        self.first_data_block
    }

    pub(crate) const fn blocks_per_group(&self) -> u32 {
        self.blocks_per_group
    }

    /// Returns the number of inodes, numbered from 1.
    pub(crate) const fn inodes(&self) -> u32 {
        self.inodes
    }

    pub(crate) const fn inodes_per_group(&self) -> u32 {
        self.inodes_per_group
    }

    /// Returns the number of block groups.
    pub(crate) const fn groups(&self) -> u32 {
        self.groups
    }

    /// Returns the size of an inode's record in the inode table.
    pub(crate) const fn inode_size(&self) -> u16 {
        self.inode_size
    }

    pub(crate) const fn features(&self) -> Features {
        self.features
    }

    /// Returns `true` if directory entries record the file type.
    pub(crate) const fn has_filetype(&self) -> bool {
        self.features.incompat & feature::INCOMPAT_FILETYPE != 0
    }

    pub(crate) const fn uuid(&self) -> [u8; 16] {
        self.uuid
    }

    /// Returns a 64-bit volume identifier: the UUID's halves, folded.
    pub(crate) fn volume_id(&self) -> u64 {
        let [a, b] = [&self.uuid[..8], &self.uuid[8..]]
            .map(|half| half.try_into().map_or(0, u64::from_le_bytes));
        a ^ b
    }

    /// Returns the offset of block `block`, if it is in the volume.
    pub(crate) fn block_offset(&self, block: u64) -> Option<u64> {
        (block < self.blocks).then(|| block.checked_mul(u64::from(self.block_size())))?
    }

    /// Returns the volume's size in bytes, per its superblock.
    pub(crate) fn volume_len(&self) -> u64 {
        // Below 2^32 blocks of at most 2^16 bytes.
        self.blocks.saturating_mul(u64::from(self.block_size()))
    }
}

/// Reads and validates the superblock of the volume in `source`, which
/// must hold the whole volume.
pub(crate) async fn read<S: ReadAt + ?Sized>(source: &S) -> Result<Superblock, Error> {
    let op = OpKind::Open;
    let size = source.size().await?;
    let BufResult(result, bytes) = source
        .read_exact_at(Vec::with_capacity(SUPERBLOCK_SIZE), SUPERBLOCK_OFFSET)
        .await;
    match result {
        Ok(()) => {},
        Err(error) if error.kind() == ErrorKind::UnexpectedEof => {
            return Err(Ext4Error::ShortSuperblock.into_error(op));
        },
        Err(error) => return Err(error),
    }
    let superblock = Superblock::parse(&bytes).map_err(|error| error.into_error(op))?;
    let volume = superblock.volume_len();
    if volume > size {
        return Err(Ext4Error::Truncated {
            volume,
            source: size,
        }
        .into_error(op));
    }
    Ok(superblock)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use alloc::vec;
    use filer_spec::block_on;

    /// The fields of a superblock, for building test volumes.
    #[derive(Clone, Copy)]
    pub(crate) struct Fields {
        pub(crate) magic: u16,
        pub(crate) revision: u32,
        pub(crate) log_block_size: u32,
        pub(crate) blocks: u32,
        pub(crate) first_data_block: u32,
        pub(crate) blocks_per_group: u32,
        pub(crate) inodes: u32,
        pub(crate) inodes_per_group: u32,
        pub(crate) inode_size: u16,
        pub(crate) features: Features,
    }

    impl Fields {
        /// A 4 MiB volume of 1 KiB blocks in one group, as `mke2fs -t
        /// ext2` makes it.
        pub(crate) fn new() -> Self {
            Self {
                magic: MAGIC,
                revision: DYNAMIC_REV,
                log_block_size: 0,
                blocks: 4096,
                first_data_block: 1,
                blocks_per_group: 8192,
                inodes: 1024,
                inodes_per_group: 1024,
                inode_size: 256,
                features: Features {
                    compat: 0x38,
                    incompat: feature::INCOMPAT_FILETYPE,
                    ro_compat: 0x3,
                },
            }
        }

        pub(crate) fn bytes(&self) -> [u8; SUPERBLOCK_SIZE] {
            let mut b = [0_u8; SUPERBLOCK_SIZE];
            let mut put = |at: usize, bytes: &[u8]| b[at..at + bytes.len()].copy_from_slice(bytes);
            put(0x0, &self.inodes.to_le_bytes());
            put(0x4, &self.blocks.to_le_bytes());
            put(0x14, &self.first_data_block.to_le_bytes());
            put(0x18, &self.log_block_size.to_le_bytes());
            put(0x1C, &self.log_block_size.to_le_bytes());
            put(0x20, &self.blocks_per_group.to_le_bytes());
            put(0x24, &self.blocks_per_group.to_le_bytes());
            put(0x28, &self.inodes_per_group.to_le_bytes());
            put(0x38, &self.magic.to_le_bytes());
            put(0x3A, &1u16.to_le_bytes());
            put(0x4C, &self.revision.to_le_bytes());
            put(0x54, &11u32.to_le_bytes());
            put(0x58, &self.inode_size.to_le_bytes());
            put(0x5C, &self.features.compat.to_le_bytes());
            put(0x60, &self.features.incompat.to_le_bytes());
            put(0x64, &self.features.ro_compat.to_le_bytes());
            put(0x68, &[0x11; 8]);
            put(0x70, &[0x22; 8]);
            b
        }

        fn parse(&self) -> Result<Superblock, Ext4Error> {
            Superblock::parse(&self.bytes())
        }
    }

    #[test]
    fn parses_a_volume() {
        let s = Fields::new().parse().unwrap();
        assert_eq!(s.revision(), 1);
        assert_eq!(
            (s.block_size(), s.blocks(), s.first_data_block()),
            (1024, 4096, 1)
        );
        assert_eq!((s.groups(), s.blocks_per_group()), (1, 8192));
        assert_eq!((s.inodes(), s.inodes_per_group()), (1024, 1024));
        assert_eq!(s.inode_size(), 256);
        assert!(s.has_filetype());
        assert_eq!(s.features().compat, 0x38);
        assert_eq!(s.volume_id(), 0x3333_3333_3333_3333);
        assert_eq!(s.volume_len(), 4096 * 1024);
        assert_eq!(s.block_offset(3), Some(3 * 1024));
        assert_eq!(s.block_offset(4096), None);
    }

    #[test]
    fn parses_every_block_size() {
        for log in 0..=MAX_LOG_BLOCK_SIZE {
            let mut f = Fields::new();
            f.log_block_size = log;
            f.first_data_block = u32::from(log == 0);
            let s = f.parse().unwrap();
            assert_eq!(s.block_size(), 1024 << log);
        }
    }

    #[test]
    fn counts_partial_groups() {
        let mut f = Fields::new();
        f.blocks_per_group = 1000;
        f.inodes_per_group = 256;
        // Blocks 1 to 4095: four whole groups and one of 95 blocks.
        assert_eq!(f.parse().unwrap().groups(), 5);
        f.blocks = 4001;
        assert_eq!(f.parse().unwrap().groups(), 4);
    }

    #[test]
    fn reads_revision_0_as_original() {
        let mut f = Fields::new();
        f.revision = 0;
        // Revision 0 has no inode size or features: they are ignored.
        f.inode_size = 0;
        f.features.incompat = 0xFFFF_FFFF;
        let s = f.parse().unwrap();
        assert_eq!(s.inode_size(), GOOD_OLD_INODE_SIZE);
        assert_eq!(s.features(), Features::default());
        assert!(!s.has_filetype());
    }

    #[test]
    fn rejects_bad_fields() {
        let check = |change: fn(&mut Fields), expected: Ext4Error| {
            let mut f = Fields::new();
            change(&mut f);
            assert_eq!(f.parse(), Err(expected));
        };
        check(|f| f.magic = 0xEF51, Ext4Error::BadMagic(0xEF51));
        check(|f| f.revision = 2, Ext4Error::UnsupportedRevision(2));
        check(|f| f.log_block_size = 7, Ext4Error::BadBlockSize(7));
        check(|f| f.first_data_block = 0, Ext4Error::BadFirstDataBlock(0));
        check(
            |f| {
                f.log_block_size = 2;
                f.first_data_block = 1;
            },
            Ext4Error::BadFirstDataBlock(1),
        );
        check(|f| f.blocks_per_group = 0, Ext4Error::BadBlocksPerGroup(0));
        check(
            |f| f.blocks_per_group = 8193,
            Ext4Error::BadBlocksPerGroup(8193),
        );
        check(|f| f.inodes_per_group = 0, Ext4Error::BadInodesPerGroup(0));
        check(
            |f| f.inodes_per_group = 8193,
            Ext4Error::BadInodesPerGroup(8193),
        );
        check(|f| f.blocks = 1, Ext4Error::NoBlocks);
        check(|f| f.blocks = 0, Ext4Error::NoBlocks);
        check(|f| f.inodes = 1, Ext4Error::BadInodeCount(1));
        check(|f| f.inodes = 1025, Ext4Error::BadInodeCount(1025));
        check(|f| f.inode_size = 64, Ext4Error::BadInodeSize(64));
        check(|f| f.inode_size = 384, Ext4Error::BadInodeSize(384));
        check(|f| f.inode_size = 2048, Ext4Error::BadInodeSize(2048));
    }

    #[test]
    fn refuses_unknown_incompatible_features() {
        let mut f = Fields::new();
        f.features.incompat |= 0x40 | 0x8_0000;
        assert_eq!(
            f.parse(),
            Err(Ext4Error::UnsupportedFeatures { incompat: 0x8_0040 })
        );
        assert_eq!(
            Ext4Error::UnsupportedFeatures { incompat: 0x40 }.kind(),
            ErrorKind::Unsupported
        );
        // Compatible and read-only compatible features need no support.
        let mut f = Fields::new();
        f.features.compat = 0xFFFF_FFFF;
        f.features.ro_compat = 0xFFFF_FFFF;
        assert!(f.parse().is_ok());
    }

    #[test]
    fn refuses_a_journal_needing_recovery() {
        let mut f = Fields::new();
        f.features.incompat |= feature::INCOMPAT_RECOVER;
        assert_eq!(f.parse(), Err(Ext4Error::NeedsRecovery));
        assert_eq!(Ext4Error::NeedsRecovery.kind(), ErrorKind::Unsupported);
    }

    #[test]
    fn reads_from_a_source() {
        let mut image = vec![0; 4096 * 1024];
        image[1024..2048].copy_from_slice(&Fields::new().bytes());
        let s = block_on(read(&image)).unwrap();
        assert_eq!(s.blocks(), 4096);

        // A source too short for the volume, or the superblock.
        let error = block_on(read(&image[..4095 * 1024])).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Corrupt);
        let error = block_on(read(&image[..2000])).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Corrupt);
        let source = core::error::Error::source(&error)
            .and_then(|source| source.downcast_ref::<Ext4Error>());
        assert_eq!(source, Some(&Ext4Error::ShortSuperblock));
    }
}

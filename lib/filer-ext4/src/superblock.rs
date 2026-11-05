// This is free and unencumbered software released into the public domain.

//! The superblock, and the volume layout it describes.

use crate::{
    Ext4Error,
    csum::{self, Checksums},
};
use filer_api::{Error, ErrorKind, OpKind, ReadAt, checksum::Crc32c};
use filer_fs::{le, read_exact};

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

/// The most blocks a cluster holds under `bigalloc`, as a power of two.
const MAX_CLUSTER_SHIFT: u32 = 16;

/// The least and most descriptor sizes under `64bit`.
const MIN_DESC_SIZE_64: u16 = 64;
const MAX_DESC_SIZE: u16 = 1024;

/// The feature flags of the three sets, as far as this driver knows them.
pub(crate) mod feature {
    /// Compatible: the volume has a journal.
    pub(crate) const COMPAT_HAS_JOURNAL: u32 = 0x4;
    /// Compatible: directories may be hashed (htree) and indexed.
    pub(crate) const COMPAT_DIR_INDEX: u32 = 0x20;
    /// Compatible: superblock backups only in the two groups named.
    pub(crate) const COMPAT_SPARSE_SUPER2: u32 = 0x200;
    /// Read-only compatible: superblock backups only in groups 0, 1, and
    /// powers of 3, 5, and 7.
    pub(crate) const RO_COMPAT_SPARSE_SUPER: u32 = 0x1;
    /// Read-only compatible: block counts of 48 bits, and in blocks for
    /// inodes with the huge file flag.
    pub(crate) const RO_COMPAT_HUGE_FILE: u32 = 0x8;
    /// Read-only compatible: group descriptors checksummed by CRC-16.
    pub(crate) const RO_COMPAT_GDT_CSUM: u32 = 0x10;
    /// Read-only compatible: blocks allocated in clusters.
    pub(crate) const RO_COMPAT_BIGALLOC: u32 = 0x200;
    /// Read-only compatible: metadata checksummed by CRC-32C.
    pub(crate) const RO_COMPAT_METADATA_CSUM: u32 = 0x400;
    /// Incompatible: directory entries record the file type.
    pub(crate) const INCOMPAT_FILETYPE: u32 = 0x2;
    /// Incompatible: the journal holds changes not yet applied.
    pub(crate) const INCOMPAT_RECOVER: u32 = 0x4;
    /// Incompatible: group descriptors in each metagroup's first group.
    pub(crate) const INCOMPAT_META_BG: u32 = 0x10;
    /// Incompatible: a block that writers update to say they have the
    /// volume mounted, which a reader need not.
    pub(crate) const INCOMPAT_MMP: u32 = 0x100;
    /// Incompatible: extended attribute values may be kept in inodes of
    /// their own.
    pub(crate) const INCOMPAT_EA_INODE: u32 = 0x400;
    /// Incompatible: files may be mapped by extent trees.
    pub(crate) const INCOMPAT_EXTENTS: u32 = 0x40;
    /// Incompatible: block numbers of 64 bits, and larger descriptors.
    pub(crate) const INCOMPAT_64BIT: u32 = 0x80;
    /// Incompatible: groups' metadata may lie in other groups.
    pub(crate) const INCOMPAT_FLEX_BG: u32 = 0x200;
    /// Incompatible: the seed of `metadata_csum` checksums is kept in the
    /// superblock, not derived from the UUID.
    pub(crate) const INCOMPAT_CSUM_SEED: u32 = 0x2000;
    /// Incompatible: hashed directories' indexes three levels deep, and
    /// directories past 2 GiB.
    pub(crate) const INCOMPAT_LARGEDIR: u32 = 0x4000;
    /// Incompatible: small files and directories may be kept in their
    /// inodes.
    pub(crate) const INCOMPAT_INLINE_DATA: u32 = 0x8000;
}

/// The incompatible features this driver reads.
const SUPPORTED_INCOMPAT: u32 = feature::INCOMPAT_FILETYPE
    | feature::INCOMPAT_META_BG
    | feature::INCOMPAT_MMP
    | feature::INCOMPAT_EA_INODE
    | feature::INCOMPAT_EXTENTS
    | feature::INCOMPAT_64BIT
    | feature::INCOMPAT_FLEX_BG
    | feature::INCOMPAT_CSUM_SEED
    | feature::INCOMPAT_LARGEDIR
    | feature::INCOMPAT_INLINE_DATA;

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
    /// The size of a group descriptor: 32, or with `64bit` at least 64.
    desc_size: u16,
    /// The first metagroup whose descriptors are in it, under `meta_bg`.
    first_meta_bg: u32,
    /// The groups holding superblock backups, under `sparse_super2`.
    backup_groups: [u32; 2],
    features: Features,
    uuid: [u8; 16],
    /// The journal's inode, or 0 if there is none in the volume.
    journal_inode: u32,
    /// The directory hashes' seed, and whether they read bytes unsigned.
    hash_seed: [u32; 4],
    unsigned_hash: bool,
    checksums: Checksums,
}

fn u16_at(bytes: &[u8], at: usize) -> Result<u16, Ext4Error> {
    le::u16_at(bytes, at).ok_or(Ext4Error::ShortSuperblock)
}

fn u32_at(bytes: &[u8], at: usize) -> Result<u32, Ext4Error> {
    le::u32_at(bytes, at).ok_or(Ext4Error::ShortSuperblock)
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
        let ro_compat = if revision == GOOD_OLD_REV {
            0
        } else {
            u32_at(bytes, 0x64)?
        };
        let bigalloc = ro_compat & feature::RO_COMPAT_BIGALLOC != 0;
        // The superblock is at byte 1024, which is block 1 of 1 KiB blocks
        // and in block 0 of larger ones; data starts with its block, or
        // under `bigalloc` with block 0 always.
        let first_data_block = u32_at(bytes, 0x14)?;
        let sb_block = u32::from(log_block_size == 0);
        if first_data_block != sb_block && !(bigalloc && first_data_block == 0) {
            return Err(Ext4Error::BadFirstDataBlock(first_data_block));
        }
        // One block's bitmap tracks each group's blocks, or under
        // `bigalloc` its clusters of 2^n blocks, and its inodes.
        let bits = block_size.saturating_mul(8);
        let blocks_per_group = u32_at(bytes, 0x20)?;
        let tracked = if !bigalloc {
            blocks_per_group
        } else {
            let log_cluster_size = u32_at(bytes, 0x1C)?;
            let ratio = log_cluster_size
                .checked_sub(log_block_size)
                .filter(|&shift| shift <= MAX_CLUSTER_SHIFT)
                .and_then(|shift| 1u32.checked_shl(shift))
                .ok_or(Ext4Error::BadClusterSize(log_cluster_size))?;
            let clusters_per_group = u32_at(bytes, 0x24)?;
            if clusters_per_group.checked_mul(ratio) != Some(blocks_per_group) {
                return Err(Ext4Error::BadBlocksPerGroup(blocks_per_group));
            }
            clusters_per_group
        };
        if blocks_per_group == 0 || tracked > bits {
            return Err(Ext4Error::BadBlocksPerGroup(blocks_per_group));
        }
        let inodes_per_group = u32_at(bytes, 0x28)?;
        if inodes_per_group == 0 || inodes_per_group > bits {
            return Err(Ext4Error::BadInodesPerGroup(inodes_per_group));
        }
        let features = if revision == GOOD_OLD_REV {
            Features::default()
        } else {
            Features {
                compat: u32_at(bytes, 0x5C)?,
                incompat: u32_at(bytes, 0x60)?,
                ro_compat: u32_at(bytes, 0x64)?,
            }
        };
        let wide = features.incompat & feature::INCOMPAT_64BIT != 0;
        let [l0, l1, l2, l3] = u32_at(bytes, 0x4)?.to_le_bytes();
        let [h0, h1, h2, h3] = if wide {
            u32_at(bytes, 0x150)?.to_le_bytes()
        } else {
            [0; 4]
        };
        let blocks = u64::from_le_bytes([l0, l1, l2, l3, h0, h1, h2, h3]);
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
        let inode_size = if revision == GOOD_OLD_REV {
            GOOD_OLD_INODE_SIZE
        } else {
            u16_at(bytes, 0x58)?
        };
        if !inode_size.is_power_of_two()
            || inode_size < GOOD_OLD_INODE_SIZE
            || u32::from(inode_size) > block_size
        {
            return Err(Ext4Error::BadInodeSize(inode_size));
        }
        let desc_size = if wide {
            let size = u16_at(bytes, 0xFE)?;
            if !size.is_power_of_two()
                || !(MIN_DESC_SIZE_64..=MAX_DESC_SIZE).contains(&size)
                || u32::from(size) > block_size
            {
                return Err(Ext4Error::BadDescriptorSize(size));
            }
            size
        } else {
            DESC_SIZE
        };
        // The descriptors follow the superblock's block, but for those of
        // metagroups from the first under `meta_bg`, each in its own.
        let block = u64::from(block_size);
        let meta_bg = features.incompat & feature::INCOMPAT_META_BG != 0;
        let first_meta_bg = if meta_bg { u32_at(bytes, 0x104)? } else { 0 };
        let per_block = block.checked_div(u64::from(desc_size)).unwrap_or(1);
        let contiguous = if meta_bg {
            u64::from(first_meta_bg).saturating_mul(per_block)
        } else {
            u64::from(groups)
        };
        let descriptors = contiguous
            .min(u64::from(groups))
            .saturating_mul(u64::from(desc_size))
            .saturating_add(u64::from(sb_block).saturating_add(1).saturating_mul(block));
        if descriptors > blocks.saturating_mul(block) {
            return Err(Ext4Error::BadGroupCount(groups));
        }
        let unknown = features.incompat & !(SUPPORTED_INCOMPAT | feature::INCOMPAT_RECOVER);
        if unknown != 0 {
            return Err(Ext4Error::UnsupportedFeatures { incompat: unknown });
        }
        let mut uuid = [0; 16];
        if let Some(from) = bytes.get(0x68..0x78) {
            uuid.copy_from_slice(from);
        }
        let mut hash_seed = [0; 4];
        for (i, word) in hash_seed.iter_mut().enumerate() {
            *word = u32_at(bytes, i.saturating_mul(4).saturating_add(0xEC))?;
        }
        // `s_flags`: 1 for signed directory hashes, 2 for unsigned.
        let unsigned_hash = u32_at(bytes, 0x160)? & 0x2 != 0;
        let backup_groups = [u32_at(bytes, 0x24C)?, u32_at(bytes, 0x250)?];
        let checksums = if features.ro_compat & feature::RO_COMPAT_METADATA_CSUM != 0 {
            // Type 1 is CRC-32C, the only one.
            if bytes.get(0x175) != Some(&1) || !csum::superblock_ok(bytes) {
                return Err(Ext4Error::BadSuperblockChecksum);
            }
            let seed = if features.incompat & feature::INCOMPAT_CSUM_SEED != 0 {
                u32_at(bytes, 0x270)?
            } else {
                Crc32c::new().chain(&uuid).state()
            };
            Checksums::Metadata { seed }
        } else if features.ro_compat & feature::RO_COMPAT_GDT_CSUM != 0 {
            Checksums::Descriptors { uuid }
        } else {
            Checksums::None
        };
        let journal_inode = if features.compat & feature::COMPAT_HAS_JOURNAL != 0 {
            u32_at(bytes, 0xE0)?
        } else {
            0
        };
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
            desc_size,
            first_meta_bg,
            backup_groups,
            features,
            uuid,
            journal_inode,
            hash_seed,
            unsigned_hash,
            checksums,
        })
    }

    /// Returns the revision level: 0 (original) or 1 (dynamic).
    #[allow(dead_code, reason = "used by tests")]
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

    #[allow(dead_code, reason = "used by tests")]
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

    /// Returns the number of blocks each group's inode table spans.
    pub(crate) fn inode_table_blocks(&self) -> u64 {
        // Below 2^32 inodes of at most 2^16 bytes.
        let bytes = u64::from(self.inodes_per_group).saturating_mul(u64::from(self.inode_size));
        bytes.div_ceil(u64::from(self.block_size()))
    }

    /// Returns the size of a group descriptor.
    pub(crate) const fn desc_size(&self) -> u16 {
        self.desc_size
    }

    /// Returns the offset of group `group`'s descriptor: in the table of
    /// blocks after the superblock's, or under `meta_bg`, from its first
    /// metagroup on, in the first block of its metagroup's first group,
    /// past any superblock backup there. Each metagroup is the groups whose
    /// descriptors one block holds.
    pub(crate) fn descriptor_offset(&self, group: u32) -> Option<u64> {
        let size = u64::from(self.desc_size);
        let per_block = u64::from(self.block_size()).checked_div(size)?;
        let index = u64::from(group).checked_div(per_block)?;
        let within = u64::from(group).checked_rem(per_block)?;
        let meta_bg = self.features.incompat & feature::INCOMPAT_META_BG != 0;
        let block = if meta_bg && index >= u64::from(self.first_meta_bg) {
            // The first block of the metagroup's first group, past its
            // superblock backup.
            let first = u32::try_from(index.checked_mul(per_block)?).ok()?;
            let mut backup = u64::from(self.has_superblock(first));
            // In 1 KiB blocks from block 0, group 0's first block holds
            // no superblock: the one after it does.
            if first == 0 && self.first_data_block == 0 && self.log_block_size == 0 {
                backup = backup.saturating_add(1);
            }
            self.group_first_block(first)?.checked_add(backup)?
        } else {
            // The table follows the superblock's block, 0 or 1.
            u64::from(self.log_block_size == 0)
                .checked_add(1)?
                .checked_add(index)?
        };
        self.block_offset(block)?
            .checked_add(within.checked_mul(size)?)
    }

    /// Returns the first block of group `group`.
    fn group_first_block(&self, group: u32) -> Option<u64> {
        u64::from(group)
            .checked_mul(u64::from(self.blocks_per_group))?
            .checked_add(u64::from(self.first_data_block))
    }

    /// Returns `true` if group `group` holds the superblock or a backup.
    fn has_superblock(&self, group: u32) -> bool {
        let Features {
            compat, ro_compat, ..
        } = self.features;
        if group == 0 {
            true
        } else if compat & feature::COMPAT_SPARSE_SUPER2 != 0 {
            self.backup_groups.contains(&group)
        } else if ro_compat & feature::RO_COMPAT_SPARSE_SUPER != 0 {
            group == 1 || [3, 5, 7].iter().any(|&base| is_power_of(group, base))
        } else {
            true
        }
    }

    /// Returns `true` if block numbers have 64 bits, and inodes' attribute
    /// blocks 48.
    pub(crate) const fn is_64bit(&self) -> bool {
        self.features.incompat & feature::INCOMPAT_64BIT != 0
    }

    /// Returns `true` if attribute values may be kept in inodes.
    pub(crate) const fn has_ea_inodes(&self) -> bool {
        self.features.incompat & feature::INCOMPAT_EA_INODE != 0
    }

    /// Returns `true` if inodes may count their blocks in 48 bits.
    pub(crate) const fn has_huge_files(&self) -> bool {
        self.features.ro_compat & feature::RO_COMPAT_HUGE_FILE != 0
    }

    #[allow(dead_code, reason = "used by tests")]
    pub(crate) const fn features(&self) -> Features {
        self.features
    }

    /// Returns `true` if directory entries record the file type.
    pub(crate) const fn has_filetype(&self) -> bool {
        self.features.incompat & feature::INCOMPAT_FILETYPE != 0
    }

    /// Returns `true` if the journal holds changes not yet written to the
    /// volume, as when it was not unmounted cleanly.
    pub(crate) const fn needs_recovery(&self) -> bool {
        self.features.incompat & feature::INCOMPAT_RECOVER != 0
    }

    /// Returns how the volume's metadata is checksummed.
    pub(crate) const fn checksums(&self) -> Checksums {
        self.checksums
    }

    /// Returns `true` if directories may be indexed by hash.
    pub(crate) const fn has_dir_index(&self) -> bool {
        self.features.compat & feature::COMPAT_DIR_INDEX != 0
    }

    /// Returns the seed of directory hashes, zero for MD4's own.
    pub(crate) const fn hash_seed(&self) -> [u32; 4] {
        self.hash_seed
    }

    /// Returns `true` if directory hashes read bytes as unsigned chars,
    /// rather than signed.
    pub(crate) const fn unsigned_hash(&self) -> bool {
        self.unsigned_hash
    }

    /// Returns the journal's inode, if the journal is in the volume; a
    /// journal on another device has none.
    pub(crate) const fn journal_inode(&self) -> Option<u32> {
        match self.journal_inode {
            0 => None,
            inode => Some(inode),
        }
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

/// Returns `true` if `n` is a power of `base`: `base`, its square, and so
/// on.
fn is_power_of(n: u32, base: u32) -> bool {
    let mut power = base;
    while power < n {
        let Some(next) = power.checked_mul(base) else {
            return false;
        };
        power = next;
    }
    power == n
}

/// Reads and validates the superblock of the volume in `source`, which
/// must hold the whole volume.
pub(crate) async fn read<S: ReadAt + ?Sized>(source: &S) -> Result<Superblock, Error> {
    let op = OpKind::Open;
    let size = source.size().await?;
    let bytes = match read_exact(source, SUPERBLOCK_OFFSET, SUPERBLOCK_SIZE, op).await {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == ErrorKind::UnexpectedEof => {
            return Err(Ext4Error::ShortSuperblock.into_error(op));
        },
        Err(error) => return Err(error),
    };
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
        // A volume of two 1 KiB blocks has no room for descriptors.
        check(|f| f.blocks = 2, Ext4Error::BadGroupCount(1));
        check(|f| f.inode_size = 64, Ext4Error::BadInodeSize(64));
        check(|f| f.inode_size = 384, Ext4Error::BadInodeSize(384));
        check(|f| f.inode_size = 2048, Ext4Error::BadInodeSize(2048));
    }

    /// `64bit` volumes: block counts of 64 bits, and descriptors of a
    /// power of two from 64 bytes to the block size.
    #[test]
    fn reads_64bit_counts_and_descriptor_sizes() {
        let mut f = Fields::new();
        f.features.incompat |= feature::INCOMPAT_64BIT;
        let parse = |desc: u16, high: u32| {
            let mut b = f.bytes();
            b[0xFE..0x100].copy_from_slice(&desc.to_le_bytes());
            b[0x150..0x154].copy_from_slice(&high.to_le_bytes());
            Superblock::parse(&b)
        };
        let sb = parse(64, 0).unwrap();
        assert_eq!((sb.blocks(), sb.desc_size()), (4096, 64));
        assert_eq!(parse(64, 1).unwrap().blocks(), (1 << 32) + 4096);
        for size in [0, 32, 48, 96, 2048] {
            assert_eq!(parse(size, 0), Err(Ext4Error::BadDescriptorSize(size)));
        }
        // Without `64bit`, the fields are not read.
        let mut b = Fields::new().bytes();
        b[0xFE] = 64;
        b[0x150] = 1;
        let sb = Superblock::parse(&b).unwrap();
        assert_eq!((sb.blocks(), sb.desc_size()), (4096, 32));
    }

    /// Under `meta_bg`, each metagroup's descriptors are in its first
    /// group's first block, past a superblock backup there, from the
    /// first metagroup the superblock names; before it, they are in the
    /// table after the superblock.
    #[test]
    fn locates_descriptors_of_metagroups() {
        let mut f = Fields::new();
        f.blocks = 8192 * 40 + 1;
        f.inodes_per_group = 128;
        f.inodes = 128 * 40;
        f.features.incompat |= feature::INCOMPAT_META_BG | feature::INCOMPAT_64BIT;
        let parse = |first_meta_bg: u32| {
            let mut b = f.bytes();
            b[0xFE..0x100].copy_from_slice(&64u16.to_le_bytes());
            b[0x104..0x108].copy_from_slice(&first_meta_bg.to_le_bytes());
            Superblock::parse(&b).unwrap()
        };
        // Sixteen descriptors a block, and groups of 8192 blocks from 1.
        let sb = parse(1);
        let block = |group: u64| (1 + group * 8192) * 1024;
        assert_eq!(sb.descriptor_offset(0), Some(2048));
        assert_eq!(sb.descriptor_offset(5), Some(2048 + 5 * 64));
        assert_eq!(sb.descriptor_offset(16), Some(block(16)));
        assert_eq!(sb.descriptor_offset(33), Some(block(32) + 64));
        // Group 0 holds the superblock, so its descriptors follow it.
        let sb = parse(0);
        assert_eq!(sb.descriptor_offset(1), Some(2048 + 64));
        // Past the volume.
        assert_eq!(sb.descriptor_offset(48), None);
    }

    #[test]
    fn tells_groups_with_superblock_backups() {
        let sb = Superblock::parse(&Fields::new().bytes()).unwrap();
        let backups: alloc::vec::Vec<u32> = (0..60).filter(|&g| sb.has_superblock(g)).collect();
        assert_eq!(backups, [0, 1, 3, 5, 7, 9, 25, 27, 49]);
        // Without `sparse_super`, every group; with `sparse_super2`, the
        // two the superblock names.
        let mut f = Fields::new();
        f.features.ro_compat = 0;
        let sb = Superblock::parse(&f.bytes()).unwrap();
        assert!((0..60).all(|g| sb.has_superblock(g)));
        f.features.compat |= feature::COMPAT_SPARSE_SUPER2;
        let mut b = f.bytes();
        b[0x24C..0x250].copy_from_slice(&1u32.to_le_bytes());
        b[0x250..0x254].copy_from_slice(&30u32.to_le_bytes());
        let sb = Superblock::parse(&b).unwrap();
        let backups: alloc::vec::Vec<u32> = (0..60).filter(|&g| sb.has_superblock(g)).collect();
        assert_eq!(backups, [0, 1, 30]);
    }

    /// Under `bigalloc`, a group's blocks are its clusters' blocks, its
    /// bitmap tracks clusters, and data may start with block 0.
    #[test]
    fn reads_clustered_geometry() {
        let mut f = Fields::new();
        f.features.ro_compat |= feature::RO_COMPAT_BIGALLOC;
        f.first_data_block = 0;
        f.blocks_per_group = 8192 * 16;
        f.blocks = 8192 * 16;
        let parse = |f: &Fields, log_cluster: u32, clusters: u32| {
            let mut b = f.bytes();
            b[0x1C..0x20].copy_from_slice(&log_cluster.to_le_bytes());
            b[0x24..0x28].copy_from_slice(&clusters.to_le_bytes());
            Superblock::parse(&b)
        };
        // Clusters of 16 blocks, 8192 a group, which a bitmap tracks.
        assert!(parse(&f, 4, 8192).is_ok());
        let groups = Err(Ext4Error::BadBlocksPerGroup(8192 * 16));
        assert_eq!(parse(&f, 4, 4096), groups);
        assert_eq!(parse(&f, 3, 8192), groups);
        assert_eq!(parse(&f, 17, 8192), Err(Ext4Error::BadClusterSize(17)));
        // Without `bigalloc`, a group of 2^17 blocks is past its bitmap,
        // and data starts with the superblock's block.
        f.features.ro_compat &= !feature::RO_COMPAT_BIGALLOC;
        assert_eq!(parse(&f, 0, 0), Err(Ext4Error::BadFirstDataBlock(0)));
        f.first_data_block = 1;
        assert_eq!(parse(&f, 0, 0), groups);
    }

    #[test]
    fn refuses_unknown_incompatible_features() {
        let mut f = Fields::new();
        f.features.incompat |= 0x4000_0000 | 0x8_0000 | feature::INCOMPAT_EXTENTS;
        assert_eq!(
            f.parse(),
            Err(Ext4Error::UnsupportedFeatures {
                incompat: 0x4008_0000
            })
        );
        assert_eq!(
            Ext4Error::UnsupportedFeatures { incompat: 0x40 }.kind(),
            ErrorKind::Unsupported
        );
        // Compatible and read-only compatible features need no support,
        // but `metadata_csum`'s checksum must match.
        let mut f = Fields::new();
        f.features.compat = 0xFFFF_FFFF;
        f.features.ro_compat = !feature::RO_COMPAT_METADATA_CSUM;
        assert!(f.parse().is_ok());
    }

    #[test]
    fn tells_a_journal_needing_recovery() {
        let mut f = Fields::new();
        assert!(!f.parse().unwrap().needs_recovery());
        f.features.incompat |= feature::INCOMPAT_RECOVER;
        assert!(f.parse().unwrap().needs_recovery());
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

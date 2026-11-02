// This is free and unencumbered software released into the public domain.

//! The superblock, and the volume layout it describes.

use crate::{FeatureSet, XfsError, be, crc, io};
use alloc::vec::Vec;
use filer_api::{BufResult, Error, ErrorKind, OpKind, ReadAt};

/// The superblock's magic number, `XFSB`.
const MAGIC: u32 = 0x5846_5342;

/// The bytes of the superblock that are read: its fields end before them,
/// in the smallest sector.
pub(crate) const SUPERBLOCK_SIZE: usize = 512;

/// Where a version 5 superblock keeps its checksum.
const CRC_AT: usize = 224;

/// The smallest and largest block sizes; version 5 needs 1 KiB.
const MIN_BLOCK_SIZE: u32 = 512;
const MIN_BLOCK_SIZE_V5: u32 = 1024;
const MAX_BLOCK_SIZE: u32 = 65536;

/// The smallest sector.
const MIN_SECTOR_SIZE: u16 = 512;

/// The smallest and largest inode sizes; version 5 needs 512 bytes.
const MIN_INODE_SIZE: u16 = 256;
const MIN_INODE_SIZE_V5: u16 = 512;
const MAX_INODE_SIZE: u16 = 2048;

/// The largest allocation group, in blocks.
const MAX_AG_BLOCKS: u32 = 1 << 31;

/// The largest directory block, in bytes.
const MAX_DIR_BLOCK_SIZE: u32 = 65536;

/// The feature flags of the superblock, as far as this driver knows them.
/// Version 4 keeps them in `sb_versionnum` and `sb_features2`; version 5
/// adds sets of its own.
pub(crate) mod feature {
    /// The bits of `sb_versionnum` holding the version.
    pub(crate) const VERSION_MASK: u16 = 0xF;
    /// Inodes have 32-bit link counts (version 2 inodes).
    pub(crate) const NLINK: u16 = 0x20;
    /// Extents may be unwritten.
    pub(crate) const EXTFLG: u16 = 0x1000;
    /// Directories are version 2.
    pub(crate) const DIRV2: u16 = 0x2000;
    /// Directories match names case-insensitively.
    pub(crate) const ASCII_CI: u16 = 0x4000;
    /// `sb_features2` holds more features.
    pub(crate) const MOREBITS: u16 = 0x8000;

    /// The version features this driver reads: `ATTR` (0x10), `NLINK`,
    /// `QUOTA` (0x40), `ALIGN` (0x80), `DALIGN` (0x100), `LOGV2` (0x400),
    /// `SECTOR` (0x800), `EXTFLG`, `DIRV2`, `ASCII_CI`, and `MOREBITS`.
    /// Not `SHARED` (0x200).
    pub(crate) const SUPPORTED_VERSION: u16 = 0xFDF0;
    /// The version features every volume this driver reads has.
    pub(crate) const REQUIRED_VERSION: u16 = NLINK | EXTFLG | DIRV2;

    /// `sb_features2`: directory entries record the file type.
    pub(crate) const FTYPE: u32 = 0x200;
    /// The `sb_features2` features this driver reads: `LAZYSBCOUNT`
    /// (0x2), `ATTR2` (0x8), `PROJID32BIT` (0x80), and `FTYPE`. `CRC`
    /// (0x100) is version 5's.
    pub(crate) const SUPPORTED_FEATURES2: u32 = 0x28A;

    /// `sb_features2`: metadata has checksums, which version 5 sets.
    pub(crate) const CRC: u32 = 0x100;

    /// Version 5 incompatible features: directory entries record the file
    /// type; inode chunks may be sparse, which changes nothing for inodes
    /// found by number; metadata carries `sb_meta_uuid`, not the volume's
    /// UUID; inodes may have 64-bit times; and wider extent counts.
    pub(crate) const INCOMPAT_FTYPE: u32 = 0x1;
    pub(crate) const SPINODES: u32 = 0x2;
    pub(crate) const META_UUID: u32 = 0x4;
    pub(crate) const BIGTIME: u32 = 0x8;
    pub(crate) const NREXT64: u32 = 0x20;
    /// Version 5 incompatible features that change nothing a reader of
    /// files sees: the log may hold exchanges of file ranges; files have
    /// parent pointers, attributes that are hidden; and metadata files
    /// are in a directory tree of their own, apart from the root's, with
    /// the realtime device, if any, in groups.
    pub(crate) const EXCHANGE: u32 = 0x40;
    pub(crate) const PARENT: u32 = 0x80;
    pub(crate) const METADIR: u32 = 0x100;
    /// The version 5 incompatible features this driver reads: all of the
    /// above. Not `NEEDSREPAIR` (0x10), which Linux refuses to mount, nor
    /// `ZONED` (0x200), a realtime section within the data device, whose
    /// files' contents, as all realtime files', are not read.
    pub(crate) const SUPPORTED_INCOMPAT: u32 =
        INCOMPAT_FTYPE | SPINODES | META_UUID | BIGTIME | NREXT64 | EXCHANGE | PARENT | METADIR;
}

/// A volume's on-disk format version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Version {
    /// Version 4: no metadata checksums.
    V4,
    /// Version 5: self-describing metadata with CRC32C checksums.
    V5,
}

/// A volume's feature flags.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Features {
    /// `sb_versionnum` without the version.
    pub(crate) version: u16,
    /// `sb_features2`, with `sb_bad_features2` folded in; zero without
    /// `MOREBITS`.
    pub(crate) features2: u32,
    /// Version 5 features a reader may ignore, but a writer must not.
    pub(crate) ro_compat: u32,
    /// Version 5 features a reader must understand.
    pub(crate) incompat: u32,
}

/// A volume's layout and identity, from a validated superblock. Immutable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Superblock {
    version: Version,
    block_log: u8,
    sector_log: u8,
    inode_log: u8,
    /// The logarithm of the inodes per block.
    inopb_log: u8,
    ag_blocks: u32,
    /// The bits of AG block numbers in inode and block numbers.
    ag_block_log: u8,
    ag_count: u32,
    /// The number of blocks in the data device.
    blocks: u64,
    dir_block_log: u8,
    root_inode: u64,
    /// The internal log's first block, linear; `None` for an external log.
    log_start: Option<u64>,
    log_blocks: u32,
    features: Features,
    uuid: [u8; 16],
    /// The UUID metadata carries: `sb_meta_uuid` with `META_UUID`, else
    /// `uuid`.
    meta_uuid: [u8; 16],
}

fn u16_at(bytes: &[u8], at: usize) -> Result<u16, XfsError> {
    be::u16_at(bytes, at).ok_or(XfsError::ShortSuperblock)
}

fn u32_at(bytes: &[u8], at: usize) -> Result<u32, XfsError> {
    be::u32_at(bytes, at).ok_or(XfsError::ShortSuperblock)
}

fn u64_at(bytes: &[u8], at: usize) -> Result<u64, XfsError> {
    be::u64_at(bytes, at).ok_or(XfsError::ShortSuperblock)
}

fn u8_at(bytes: &[u8], at: usize) -> Result<u8, XfsError> {
    bytes.get(at).copied().ok_or(XfsError::ShortSuperblock)
}

/// Returns `true` if `value` is a power of two whose logarithm is `log`.
fn is_power(value: u32, log: u8) -> bool {
    value.is_power_of_two() && value.trailing_zeros() == u32::from(log)
}

/// Returns the low `bits` bits of `value`.
fn low_bits(value: u64, bits: u8) -> u64 {
    value
        & 1u64
            .checked_shl(u32::from(bits))
            .map_or(u64::MAX, |n| n.wrapping_sub(1))
}

impl Superblock {
    /// Parses and validates a superblock, of which the first 512 bytes are
    /// read: its magic number, version, features, and geometry, refusing
    /// features it does not read. A version 5 superblock's checksum covers
    /// its whole sector, which [`check_crc`](Self::check_crc) checks.
    pub(crate) fn parse(bytes: &[u8]) -> Result<Self, XfsError> {
        if bytes.len() < SUPERBLOCK_SIZE {
            return Err(XfsError::ShortSuperblock);
        }
        let magic = u32_at(bytes, 0)?;
        if magic != MAGIC {
            return Err(XfsError::BadMagic(magic));
        }
        let versionnum = u16_at(bytes, 100)?;
        let version = match versionnum & feature::VERSION_MASK {
            4 => Version::V4,
            5 => Version::V5,
            other => {
                return Err(XfsError::UnsupportedVersion(
                    u8::try_from(other).unwrap_or(u8::MAX),
                ));
            },
        };
        let features = Self::parse_features(bytes, version, versionnum)?;

        let block_size = u32_at(bytes, 4)?;
        let block_log = u8_at(bytes, 120)?;
        let min_block = match version {
            Version::V4 => MIN_BLOCK_SIZE,
            Version::V5 => MIN_BLOCK_SIZE_V5,
        };
        if !(min_block..=MAX_BLOCK_SIZE).contains(&block_size) || !is_power(block_size, block_log) {
            return Err(XfsError::BadBlockSize(block_size));
        }
        let sector_size = u16_at(bytes, 102)?;
        let sector_log = u8_at(bytes, 121)?;
        if sector_size < MIN_SECTOR_SIZE
            || u32::from(sector_size) > block_size
            || !is_power(u32::from(sector_size), sector_log)
        {
            return Err(XfsError::BadSectorSize(sector_size));
        }
        let inode_size = u16_at(bytes, 104)?;
        let inode_log = u8_at(bytes, 122)?;
        let min_inode = match version {
            Version::V4 => MIN_INODE_SIZE,
            Version::V5 => MIN_INODE_SIZE_V5,
        };
        let inopb_log = block_log.checked_sub(inode_log);
        let inopb = u32::from(u16_at(bytes, 106)?);
        if !(min_inode..=MAX_INODE_SIZE).contains(&inode_size)
            || u32::from(inode_size) > block_size
            || !is_power(u32::from(inode_size), inode_log)
            || inopb_log != Some(u8_at(bytes, 123)?)
            || Some(inopb) != block_size.checked_div(u32::from(inode_size))
        {
            return Err(XfsError::BadInodeSize(inode_size));
        }
        // Checked above: the inode is no larger than the block.
        let inopb_log = inopb_log.unwrap_or(0);

        // AG-relative inode numbers fit in 32 bits.
        let ag_blocks = u32_at(bytes, 84)?;
        let ag_block_log = u8_at(bytes, 124)?;
        if ag_blocks == 0
            || ag_blocks > MAX_AG_BLOCKS
            || ag_blocks.next_power_of_two().trailing_zeros() != u32::from(ag_block_log)
            || ag_block_log.saturating_add(inopb_log) > 32
        {
            return Err(XfsError::BadAgSize(ag_blocks));
        }
        // With at most 2^32 AGs of at most 2^32 inodes, inode numbers fit
        // in 64 bits.
        let ag_count = u32_at(bytes, 88)?;
        if ag_count == 0 {
            return Err(XfsError::BadAgCount(ag_count));
        }
        // Every AG but the last is whole, and the volume's bytes fit in
        // 64 bits.
        let blocks = u64_at(bytes, 8)?;
        let full = u64::from(ag_count.wrapping_sub(1)).saturating_mul(u64::from(ag_blocks));
        if blocks <= full
            || blocks > full.saturating_add(u64::from(ag_blocks))
            || blocks.checked_mul(u64::from(block_size)).is_none()
        {
            return Err(XfsError::BadBlockCount(blocks));
        }
        let dir_block_log = u8_at(bytes, 192)?;
        if block_size
            .checked_shl(u32::from(dir_block_log))
            .is_none_or(|size| size > MAX_DIR_BLOCK_SIZE)
        {
            return Err(XfsError::BadDirBlockSize(dir_block_log));
        }
        if u8_at(bytes, 126)? != 0 {
            return Err(XfsError::Unfinished);
        }
        let uuid_at = |at: usize| {
            bytes
                .get(at..at.saturating_add(16))
                .and_then(|uuid| <[u8; 16]>::try_from(uuid).ok())
                .ok_or(XfsError::ShortSuperblock)
        };
        let uuid = uuid_at(32)?;
        let meta_uuid = if features.incompat & feature::META_UUID != 0 {
            uuid_at(248)?
        } else {
            uuid
        };
        let mut sb = Self {
            version,
            block_log,
            sector_log,
            inode_log,
            inopb_log,
            ag_blocks,
            ag_block_log,
            ag_count,
            blocks,
            dir_block_log,
            root_inode: u64_at(bytes, 56)?,
            log_start: None,
            log_blocks: u32_at(bytes, 96)?,
            features,
            uuid,
            meta_uuid,
        };
        if sb.inode_offset(sb.root_inode).is_none() {
            return Err(XfsError::BadInodeNumber(sb.root_inode));
        }
        // An internal log lies within one AG; an external one starts at 0.
        let log_start = u64_at(bytes, 48)?;
        if log_start != 0 {
            let last = log_start
                .checked_add(u64::from(sb.log_blocks))
                .and_then(|end| end.checked_sub(1));
            let ag = |block: u64| block.checked_shr(u32::from(ag_block_log));
            let in_one_ag = last.is_some_and(|last| ag(last) == ag(log_start));
            match (
                sb.linear_block(log_start),
                last.and_then(|last| sb.linear_block(last)),
            ) {
                (Some(first), Some(_)) if sb.log_blocks > 0 && in_one_ag => {
                    sb.log_start = Some(first);
                },
                _ => return Err(XfsError::BadLog),
            }
        }
        Ok(sb)
    }

    /// Reads the feature flags of a version `version` superblock, whose
    /// `sb_versionnum` is `versionnum`, refusing those this driver does
    /// not read, and volumes lacking those it requires.
    fn parse_features(
        bytes: &[u8],
        version: Version,
        versionnum: u16,
    ) -> Result<Features, XfsError> {
        let bits = versionnum & !feature::VERSION_MASK;
        let missing = feature::REQUIRED_VERSION & !bits;
        if missing != 0 {
            return Err(XfsError::MissingFeatures(missing));
        }
        let unknown = bits & !feature::SUPPORTED_VERSION;
        if unknown != 0 {
            return Err(XfsError::UnsupportedFeatures {
                set: FeatureSet::Version,
                bits: u32::from(unknown),
            });
        }
        // Old kernels wrote `sb_features2` at `sb_bad_features2`'s place.
        let features2 = if bits & feature::MOREBITS == 0 {
            0
        } else {
            u32_at(bytes, 200)? | u32_at(bytes, 204)?
        };
        let mut features = Features {
            version: bits,
            features2,
            ..Features::default()
        };
        let supported = match version {
            Version::V4 => feature::SUPPORTED_FEATURES2,
            Version::V5 => feature::SUPPORTED_FEATURES2 | feature::CRC,
        };
        let unknown = features2 & !supported;
        if unknown != 0 {
            return Err(XfsError::UnsupportedFeatures {
                set: FeatureSet::Features2,
                bits: unknown,
            });
        }
        if version == Version::V5 {
            // Read-only compatible features change nothing a reader sees.
            features.ro_compat = u32_at(bytes, 212)?;
            features.incompat = u32_at(bytes, 216)?;
            let unsupported = features.incompat & !feature::SUPPORTED_INCOMPAT;
            if unsupported != 0 {
                return Err(XfsError::UnsupportedFeatures {
                    set: FeatureSet::Incompat,
                    bits: unsupported,
                });
            }
        }
        Ok(features)
    }

    /// Returns the format version.
    pub(crate) const fn version(&self) -> Version {
        self.version
    }

    /// Returns the block size in bytes: a power of two from 512 bytes to
    /// 64 KiB.
    pub(crate) const fn block_size(&self) -> u32 {
        1 << self.block_log
    }

    /// Returns the sector size in bytes: a power of two from 512 bytes to
    /// the block size.
    pub(crate) const fn sector_size(&self) -> u32 {
        1 << self.sector_log
    }

    /// Returns the size of an inode in bytes: a power of two from 256
    /// bytes to 2 KiB, at most a block.
    pub(crate) const fn inode_size(&self) -> u32 {
        1 << self.inode_log
    }

    /// Returns the size of a directory block in bytes, at most 64 KiB.
    pub(crate) const fn dir_block_size(&self) -> u32 {
        1 << self.block_log.saturating_add(self.dir_block_log)
    }

    /// Returns the number of blocks in an allocation group, but perhaps the
    /// last.
    #[allow(dead_code, reason = "used by tests")]
    pub(crate) const fn ag_blocks(&self) -> u32 {
        self.ag_blocks
    }

    /// Returns the number of allocation groups.
    #[allow(dead_code, reason = "used by tests")]
    pub(crate) const fn ag_count(&self) -> u32 {
        self.ag_count
    }

    /// Returns the number of blocks in the volume.
    #[allow(dead_code, reason = "used by tests")]
    pub(crate) const fn blocks(&self) -> u64 {
        self.blocks
    }

    /// Returns the root directory's inode number.
    pub(crate) const fn root_inode(&self) -> u64 {
        self.root_inode
    }

    /// Returns the internal log's first block and length in blocks, or
    /// `None` for an external log.
    pub(crate) fn internal_log(&self) -> Option<(u64, u32)> {
        self.log_start.map(|start| (start, self.log_blocks))
    }

    #[allow(dead_code, reason = "used by tests")]
    pub(crate) const fn features(&self) -> Features {
        self.features
    }

    /// Returns `true` if directory entries record the file type.
    pub(crate) const fn has_ftype(&self) -> bool {
        match self.version {
            Version::V4 => self.features.features2 & feature::FTYPE != 0,
            Version::V5 => self.features.incompat & feature::INCOMPAT_FTYPE != 0,
        }
    }

    /// Returns `true` if metadata is self-describing, with checksums.
    pub(crate) fn is_v5(&self) -> bool {
        self.version == Version::V5
    }

    /// Returns the UUID metadata carries.
    pub(crate) const fn meta_uuid(&self) -> &[u8; 16] {
        &self.meta_uuid
    }

    /// Checks a version 5 superblock's checksum over its `sector`, whose
    /// size is the sector size; a version 4 one has none.
    pub(crate) fn check_crc(&self, sector: &[u8]) -> Result<(), XfsError> {
        if self.is_v5() && !crc::is_valid(sector, CRC_AT) {
            return Err(XfsError::BadChecksum { offset: 0 });
        }
        Ok(())
    }

    /// Returns `true` if directories match names case-insensitively.
    pub(crate) const fn ascii_ci(&self) -> bool {
        self.features.version & feature::ASCII_CI != 0
    }

    /// Returns a 64-bit volume identifier: the UUID's halves, folded.
    pub(crate) fn volume_id(&self) -> u64 {
        let [a, b] = [&self.uuid[..8], &self.uuid[8..]]
            .map(|half| half.try_into().map_or(0, u64::from_be_bytes));
        a ^ b
    }

    /// Returns the linear number of the block `block` names (its AG in the
    /// bits above `sb_agblklog`, then its AG block), if it is in the
    /// volume.
    pub(crate) fn linear_block(&self, block: u64) -> Option<u64> {
        let ag = block.checked_shr(u32::from(self.ag_block_log))?;
        let ag_block = low_bits(block, self.ag_block_log);
        if ag >= u64::from(self.ag_count) || ag_block >= u64::from(self.ag_blocks) {
            return None;
        }
        // At most 2^32 AGs of at most 2^31 blocks.
        let linear = ag
            .checked_mul(u64::from(self.ag_blocks))?
            .checked_add(ag_block)?;
        (linear < self.blocks).then_some(linear)
    }

    /// Returns the linear number of the first of `len` blocks from `block`
    /// (numbered as for [`linear_block`](Self::linear_block)), if they all
    /// lie in one AG of the volume.
    pub(crate) fn linear_extent(&self, block: u64, len: u64) -> Option<u64> {
        let last = block.checked_add(len.checked_sub(1)?)?;
        let shift = u32::from(self.ag_block_log);
        if block.checked_shr(shift)? != last.checked_shr(shift)? {
            return None;
        }
        self.linear_block(last)?;
        self.linear_block(block)
    }

    /// Returns the offset of linear block `block`, if it is in the volume.
    pub(crate) fn block_offset(&self, block: u64) -> Option<u64> {
        (block < self.blocks).then(|| block.checked_mul(u64::from(self.block_size())))?
    }

    /// Returns the offset of inode `inode` (its AG, AG block, and slot in
    /// the block, from the top bits down), if it is in the volume.
    pub(crate) fn inode_offset(&self, inode: u64) -> Option<u64> {
        let block = self.linear_block(inode.checked_shr(u32::from(self.inopb_log))?)?;
        let slot = low_bits(inode, self.inopb_log);
        self.block_offset(block)?
            .checked_add(slot.checked_mul(u64::from(self.inode_size()))?)
    }

    /// Returns the volume's size in bytes, per its superblock.
    pub(crate) fn volume_len(&self) -> u64 {
        // Checked when parsed.
        self.blocks.saturating_mul(u64::from(self.block_size()))
    }
}

/// Reads and validates the superblock of the volume in `source`, which
/// must hold the whole data device.
pub(crate) async fn read<S: ReadAt + ?Sized>(source: &S) -> Result<Superblock, Error> {
    let op = OpKind::Open;
    let size = source.size().await?;
    let BufResult(result, bytes) = source
        .read_exact_at(Vec::with_capacity(SUPERBLOCK_SIZE), 0)
        .await;
    match result {
        Ok(()) => {},
        Err(error) if error.kind() == ErrorKind::UnexpectedEof => {
            return Err(XfsError::ShortSuperblock.into_error(op));
        },
        Err(error) => return Err(error),
    }
    let superblock = Superblock::parse(&bytes).map_err(|error| error.into_error(op))?;
    if superblock.is_v5() {
        let len = usize::try_from(superblock.sector_size()).unwrap_or(usize::MAX);
        let sector = io::read_exact(source, 0, len).await?;
        superblock
            .check_crc(&sector)
            .map_err(|error| error.into_error(op))?;
    }
    let volume = superblock.volume_len();
    if volume > size {
        return Err(XfsError::Truncated {
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
        pub(crate) magic: u32,
        pub(crate) versionnum: u16,
        pub(crate) features2: u32,
        pub(crate) bad_features2: u32,
        pub(crate) incompat: u32,
        pub(crate) block_size: u32,
        pub(crate) block_log: u8,
        pub(crate) blocks: u64,
        pub(crate) sector_size: u16,
        pub(crate) sector_log: u8,
        pub(crate) inode_size: u16,
        pub(crate) inode_log: u8,
        pub(crate) inopb: u16,
        pub(crate) inopb_log: u8,
        pub(crate) ag_blocks: u32,
        pub(crate) ag_block_log: u8,
        pub(crate) ag_count: u32,
        pub(crate) dir_block_log: u8,
        pub(crate) root_inode: u64,
        pub(crate) log_start: u64,
        pub(crate) log_blocks: u32,
        pub(crate) in_progress: u8,
    }

    impl Fields {
        /// A 4 MiB version 4 volume of 4 KiB blocks and 256-byte inodes in
        /// four AGs, with the features `mkfs.xfs -m crc=0` sets.
        pub(crate) fn new() -> Self {
            Self {
                magic: MAGIC,
                versionnum: 0xB4A4,
                features2: 0x28A,
                bad_features2: 0x28A,
                incompat: 0,
                block_size: 4096,
                block_log: 12,
                blocks: 1024,
                sector_size: 512,
                sector_log: 9,
                inode_size: 256,
                inode_log: 8,
                inopb: 16,
                inopb_log: 4,
                ag_blocks: 256,
                ag_block_log: 8,
                ag_count: 4,
                dir_block_log: 0,
                root_inode: 128,
                // AG 2, block 4.
                log_start: (2 << 8) | 4,
                log_blocks: 200,
                in_progress: 0,
            }
        }

        pub(crate) fn bytes(&self) -> [u8; SUPERBLOCK_SIZE] {
            let mut b = [0_u8; SUPERBLOCK_SIZE];
            let mut put = |at: usize, bytes: &[u8]| b[at..at + bytes.len()].copy_from_slice(bytes);
            put(0, &self.magic.to_be_bytes());
            put(4, &self.block_size.to_be_bytes());
            put(8, &self.blocks.to_be_bytes());
            put(32, &[0x11; 8]);
            put(40, &[0x22; 8]);
            put(48, &self.log_start.to_be_bytes());
            put(56, &self.root_inode.to_be_bytes());
            put(84, &self.ag_blocks.to_be_bytes());
            put(88, &self.ag_count.to_be_bytes());
            put(96, &self.log_blocks.to_be_bytes());
            put(100, &self.versionnum.to_be_bytes());
            put(102, &self.sector_size.to_be_bytes());
            put(104, &self.inode_size.to_be_bytes());
            put(106, &self.inopb.to_be_bytes());
            put(
                120,
                &[
                    self.block_log,
                    self.sector_log,
                    self.inode_log,
                    self.inopb_log,
                    self.ag_block_log,
                    0,
                    self.in_progress,
                    25,
                ],
            );
            put(192, &[self.dir_block_log]);
            put(200, &self.features2.to_be_bytes());
            put(204, &self.bad_features2.to_be_bytes());
            put(216, &self.incompat.to_be_bytes());
            put(248, &[0x44; 16]);
            if self.versionnum & 0xF == 5 {
                crate::crc::seal(&mut b, CRC_AT);
            }
            b
        }

        /// The same volume, version 5 with the features `mkfs.xfs` sets
        /// with all later ones off, and 512-byte inodes.
        pub(crate) fn v5() -> Self {
            let mut f = Self::new();
            f.versionnum = 0xB4A5;
            f.features2 = 0x18A;
            f.bad_features2 = 0x18A;
            f.incompat = 0x1;
            f.inode_size = 512;
            f.inode_log = 9;
            f.block_log(12);
            f
        }

        pub(crate) fn parse(&self) -> Result<Superblock, XfsError> {
            Superblock::parse(&self.bytes())
        }

        /// Sets the block size to 2^`log`, and the inodes per block.
        fn block_log(&mut self, log: u8) {
            self.block_size = 1 << log;
            self.block_log = log;
            self.inopb_log = log - self.inode_log;
            self.inopb = 1 << self.inopb_log;
        }
    }

    #[test]
    fn parses_a_volume() {
        let s = Fields::new().parse().unwrap();
        assert_eq!(s.version(), Version::V4);
        assert_eq!((s.block_size(), s.blocks()), (4096, 1024));
        assert_eq!((s.sector_size(), s.inode_size()), (512, 256));
        assert_eq!((s.ag_blocks(), s.ag_count()), (256, 4));
        assert_eq!(s.dir_block_size(), 4096);
        assert_eq!(s.root_inode(), 128);
        assert_eq!(s.internal_log(), Some((516, 200)));
        assert!(s.has_ftype());
        assert_eq!(
            s.features(),
            Features {
                version: 0xB4A0,
                features2: 0x28A,
                ro_compat: 0,
                incompat: 0,
            }
        );
        assert_eq!(s.volume_id(), 0x3333_3333_3333_3333);
        assert_eq!(s.volume_len(), 4096 * 1024);
        assert!(!s.ascii_ci());
        let mut f = Fields::new();
        f.versionnum |= 0x4000;
        assert!(f.parse().unwrap().ascii_ci());
    }

    #[test]
    fn maps_block_and_inode_numbers() {
        let s = Fields::new().parse().unwrap();
        // AG 1, block 3.
        assert_eq!(s.linear_block((1 << 8) | 3), Some(259));
        assert_eq!(s.block_offset(259), Some(259 * 4096));
        assert_eq!(s.linear_block(255), Some(255));
        // Past the last AG; past an AG's blocks.
        assert_eq!(s.linear_block(4 << 8), None);
        assert_eq!(s.block_offset(1024), None);
        // Inode 128 is slot 0 of block 8; inode 0x1235 is slot 5 of AG 1's
        // block 0x23.
        assert_eq!(s.inode_offset(128), Some(8 * 4096));
        assert_eq!(s.inode_offset(0x1235), Some((256 + 0x23) * 4096 + 5 * 256));
        assert_eq!(s.inode_offset(4 << 12), None);
        assert_eq!(s.inode_offset(u64::MAX), None);

        // A last AG of 100 blocks.
        let mut f = Fields::new();
        f.blocks = 868;
        let s = f.parse().unwrap();
        assert_eq!(s.linear_block((3 << 8) | 99), Some(867));
        assert_eq!(s.linear_block((3 << 8) | 100), None);
        // AGs whose size is not a power of two leave gaps in the numbers.
        let mut f = Fields::new();
        f.ag_blocks = 200;
        f.blocks = 800;
        f.log_start = (2 << 8) | 4;
        f.log_blocks = 100;
        let s = f.parse().unwrap();
        assert_eq!(s.linear_block((1 << 8) | 199), Some(399));
        assert_eq!(s.linear_block((1 << 8) | 200), None);
    }

    #[test]
    fn parses_every_block_and_inode_size() {
        for block_log in 9..=16 {
            for inode_log in 8..=11.min(block_log) {
                let mut f = Fields::new();
                f.inode_size = 1 << inode_log;
                f.inode_log = inode_log;
                f.block_log(block_log);
                f.root_inode = 1 << (f.inopb_log + 3);
                let s = f.parse().unwrap();
                assert_eq!(s.block_size(), 1 << block_log);
                assert_eq!(s.inode_size(), 1 << inode_log);
            }
        }
    }

    #[test]
    fn rejects_bad_fields() {
        let check = |change: fn(&mut Fields), expected: XfsError| {
            let mut f = Fields::new();
            change(&mut f);
            assert_eq!(f.parse(), Err(expected));
        };
        check(|f| f.magic = 0x5846_5341, XfsError::BadMagic(0x5846_5341));
        check(|f| f.versionnum = 0xB4A3, XfsError::UnsupportedVersion(3));
        check(|f| f.versionnum = 0xB4A6, XfsError::UnsupportedVersion(6));
        check(|f| f.block_size = 8192, XfsError::BadBlockSize(8192));
        check(|f| f.block_size = 4000, XfsError::BadBlockSize(4000));
        check(|f| f.block_log(17), XfsError::BadBlockSize(1 << 17));
        check(|f| f.sector_log = 10, XfsError::BadSectorSize(512));
        check(
            |f| {
                f.sector_size = 8192;
                f.sector_log = 13;
            },
            XfsError::BadSectorSize(8192),
        );
        check(|f| f.sector_size = 256, XfsError::BadSectorSize(256));
        check(
            |f| {
                f.inode_size = 128;
                f.inode_log = 7;
                f.inopb = 32;
                f.inopb_log = 5;
            },
            XfsError::BadInodeSize(128),
        );
        check(|f| f.inopb = 15, XfsError::BadInodeSize(256));
        check(|f| f.inopb_log = 3, XfsError::BadInodeSize(256));
        check(
            |f| {
                f.inode_size = 4096;
                f.inode_log = 12;
                f.inopb = 1;
                f.inopb_log = 0;
            },
            XfsError::BadInodeSize(4096),
        );
        check(|f| f.ag_blocks = 0, XfsError::BadAgSize(0));
        check(|f| f.ag_block_log = 9, XfsError::BadAgSize(256));
        check(
            |f| {
                f.ag_blocks = 1 << 30;
                f.ag_block_log = 30;
            },
            XfsError::BadAgSize(1 << 30),
        );
        check(|f| f.ag_count = 0, XfsError::BadAgCount(0));
        check(|f| f.blocks = 768, XfsError::BadBlockCount(768));
        check(|f| f.blocks = 1025, XfsError::BadBlockCount(1025));
        check(|f| f.dir_block_log = 5, XfsError::BadDirBlockSize(5));
        check(
            |f| f.root_inode = 4 << 12,
            XfsError::BadInodeNumber(4 << 12),
        );
        check(|f| f.log_start = 4 << 8, XfsError::BadLog);
        check(|f| f.log_blocks = 253, XfsError::BadLog);
        check(|f| f.log_blocks = 0, XfsError::BadLog);
        check(|f| f.in_progress = 1, XfsError::Unfinished);
    }

    #[test]
    fn reads_an_external_log() {
        let mut f = Fields::new();
        f.log_start = 0;
        assert_eq!(f.parse().unwrap().internal_log(), None);
    }

    #[test]
    fn refuses_missing_and_unknown_features() {
        let check = |change: fn(&mut Fields), expected: XfsError| {
            let mut f = Fields::new();
            change(&mut f);
            assert_eq!(f.parse(), Err(expected));
            assert_eq!(expected.kind(), ErrorKind::Unsupported);
        };
        check(|f| f.versionnum &= !0x20, XfsError::MissingFeatures(0x20));
        check(
            |f| f.versionnum &= !0x3000,
            XfsError::MissingFeatures(0x3000),
        );
        let version = |bits| XfsError::UnsupportedFeatures {
            set: FeatureSet::Version,
            bits,
        };
        check(|f| f.versionnum |= 0x200, version(0x200));
        let features2 = |bits| XfsError::UnsupportedFeatures {
            set: FeatureSet::Features2,
            bits,
        };
        check(|f| f.features2 |= 0x100, features2(0x100));
        check(|f| f.bad_features2 |= 0x1, features2(0x1));
        check(|f| f.features2 |= 0x8000_0000, features2(0x8000_0000));
    }

    #[test]
    fn reads_features2_only_with_morebits() {
        let mut f = Fields::new();
        f.versionnum &= !0x8000;
        f.features2 = 0xFFFF_FFFF;
        let s = f.parse().unwrap();
        assert_eq!(s.features().features2, 0);
        assert!(!s.has_ftype());
        // Either copy of `sb_features2` counts.
        let mut f = Fields::new();
        f.features2 = 0x8;
        assert!(f.parse().unwrap().has_ftype());
    }

    #[test]
    fn parses_version_5() {
        let f = Fields::v5();
        let s = f.parse().unwrap();
        assert!(s.is_v5() && s.has_ftype());
        assert_eq!(s.meta_uuid(), &[[0x11; 8], [0x22; 8]].concat()[..]);
        assert_eq!(s.check_crc(&f.bytes()), Ok(()));
        let mut bytes = f.bytes();
        bytes[300] ^= 1;
        assert_eq!(
            s.check_crc(&bytes),
            Err(XfsError::BadChecksum { offset: 0 })
        );
        // Version 4 has no checksum.
        let v4 = Fields::new();
        assert_eq!(v4.parse().unwrap().check_crc(&[0; 512]), Ok(()));

        // As `mkfs.xfs` 6.18 makes them by default, and with all its
        // features.
        for incompat in [0x2B, 0x1EB] {
            let mut d = Fields::v5();
            d.incompat = incompat;
            assert!(d.parse().is_ok(), "{incompat:#X}");
        }

        // Metadata carries `sb_meta_uuid` with `META_UUID`.
        let mut m = Fields::v5();
        m.incompat |= 0x4;
        assert_eq!(m.parse().unwrap().meta_uuid(), &[0x44; 16]);

        // Later incompatible features, and the checksum bit on version 4.
        let incompat = |bits| XfsError::UnsupportedFeatures {
            set: FeatureSet::Incompat,
            bits,
        };
        for bits in [0x10, 0x200, 0x400] {
            let mut f = Fields::v5();
            f.incompat |= bits;
            assert_eq!(f.parse(), Err(incompat(bits)), "{bits:#X}");
        }
        let mut f = Fields::new();
        f.features2 |= 0x100;
        assert_eq!(
            f.parse(),
            Err(XfsError::UnsupportedFeatures {
                set: FeatureSet::Features2,
                bits: 0x100
            })
        );
        // Version 5 needs larger blocks and inodes.
        let mut f = Fields::v5();
        f.block_log(9);
        assert_eq!(f.parse(), Err(XfsError::BadBlockSize(512)));
        let mut f = Fields::v5();
        f.inode_size = 256;
        f.inode_log = 8;
        f.block_log(12);
        assert_eq!(f.parse(), Err(XfsError::BadInodeSize(256)));
    }

    #[test]
    fn reads_from_a_source() {
        let mut image = vec![0; 4096 * 1024];
        image[..SUPERBLOCK_SIZE].copy_from_slice(&Fields::new().bytes());
        let s = block_on(read(&image)).unwrap();
        assert_eq!(s.blocks(), 1024);

        // A source too short for the volume, or the superblock.
        let error = block_on(read(&image[..4096 * 1023])).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Corrupt);
        let error = block_on(read(&image[..500])).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Corrupt);
        let source =
            core::error::Error::source(&error).and_then(|source| source.downcast_ref::<XfsError>());
        assert_eq!(source, Some(&XfsError::ShortSuperblock));
    }
}

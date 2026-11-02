// This is free and unencumbered software released into the public domain.

//! File maps: how a fork maps file blocks to volume blocks, by extent
//! records listed in the inode or held in the leaves of a B+ tree rooted
//! there. Gaps between extents are holes, and unwritten extents read as
//! zeros.
//!
//! Blocks are mapped lazily, a run at a time, reading the B+ tree's blocks
//! on the way down: no file's whole map is ever held.

use crate::{
    XfsError, crc,
    inode::{EXTENT_SIZE, Fork, Format, Inode},
    superblock::Superblock,
};
use alloc::vec::Vec;
use core::num::NonZeroU64;
use filer_api::{Error, OpKind, ReadAt};
use filer_fs::{
    be,
    map::{self, Mapping, Run},
    read_exact,
};

/// A B+ tree block's magic number: `BMAP`, or with version 5 `BMA3`.
const BMAP_MAGIC: u32 = 0x424D_4150;
const BMA3_MAGIC: u32 = 0x424D_4133;

/// The size of a `BMAP` block's header, of a `BMA3` block's, and of a
/// root's in the inode.
const BLOCK_HEADER: usize = 24;
const BLOCK_HEADER_V3: usize = 72;
const ROOT_HEADER: usize = 4;

/// The size of a B+ tree node's key, and of its pointer.
const KEY_SIZE: usize = 8;

/// The deepest B+ tree read: far more levels than 2^54 file blocks need
/// at the smallest fan-out.
const MAX_LEVELS: u16 = 16;

/// The first file block past the largest file: offsets have 54 bits.
const MAX_FILE_BLOCKS: u64 = 1 << 54;
const FILE_BLOCK_MASK: u64 = MAX_FILE_BLOCKS.wrapping_sub(1);

/// `di_flags`: the file's data is on the realtime device.
const REALTIME: u16 = 0x1;

/// An extent record: `len` file blocks from `logical`, stored from volume
/// block `physical` (its AG above `sb_agblklog` bits), or reading as zeros
/// if `unwritten`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Extent {
    pub(crate) logical: u64,
    pub(crate) physical: u64,
    pub(crate) len: u64,
    pub(crate) unwritten: bool,
}

impl Extent {
    /// Decodes a packed 128-bit record: from the top, the unwritten flag,
    /// a 54-bit file block, a 52-bit volume block, and a 21-bit length.
    pub(crate) fn decode(record: &[u8; EXTENT_SIZE]) -> Self {
        let (hi, lo) = record.split_at(8);
        let hi = be::u64_at(hi, 0).unwrap_or(0);
        let lo = be::u64_at(lo, 0).unwrap_or(0);
        Self {
            logical: (hi >> 9) & FILE_BLOCK_MASK,
            physical: ((hi & 0x1FF) << 43) | (lo >> 21),
            len: lo & 0x1F_FFFF,
            unwritten: hi >> 63 != 0,
        }
    }

    /// Returns the file block after the extent.
    fn end(&self) -> u64 {
        // At most 2^54 + 2^21, from the decoded widths.
        self.logical.saturating_add(self.len)
    }
}

/// Decodes the extent records of `bytes`.
fn decode_all(bytes: &[u8]) -> impl Iterator<Item = Extent> {
    bytes
        .as_chunks::<EXTENT_SIZE>()
        .0
        .iter()
        .map(Extent::decode)
}

/// Where a fork's records are.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Root {
    /// In the inode.
    List(Vec<Extent>),
    /// In the leaves of a B+ tree of `level` levels below these keys and
    /// pointers.
    Tree {
        level: u16,
        keys: Vec<u64>,
        pointers: Vec<u64>,
    },
}

/// The map of a fork.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FileMap {
    inode: u64,
    root: Root,
}

/// Where a B+ tree descent goes from a node: to a hole of known length,
/// or to the child at `pointer`, whose records end below `upper`.
enum Step {
    Hole(u64),
    Child { pointer: u64, upper: Option<u64> },
}

impl FileMap {
    /// Returns the map of `inode`'s data fork. Realtime files' data is on
    /// another device, which is not read.
    pub(crate) fn data(inode: &Inode, sb: &Superblock) -> Result<Self, XfsError> {
        if inode.flags() & REALTIME != 0 {
            return Err(XfsError::RealtimeData {
                inode: inode.number(),
            });
        }
        Self::new(inode.number(), inode.data(), sb)
    }

    /// Returns the map of `fork`, of inode `inode`, which must be an
    /// extent list or a B+ tree.
    pub(crate) fn new(inode: u64, fork: &Fork, sb: &Superblock) -> Result<Self, XfsError> {
        let bad = XfsError::BadMap { inode };
        let root = match fork.format {
            Format::Extents => {
                let extents: Vec<Extent> = decode_all(&fork.bytes).collect();
                check(&extents, None, sb).ok_or(bad)?;
                Root::List(extents)
            },
            Format::Btree => {
                let level = be::u16_at(&fork.bytes, 0).ok_or(bad)?;
                let count = be::u16_at(&fork.bytes, 2).ok_or(bad)?;
                if level == 0 || level > MAX_LEVELS {
                    return Err(bad);
                }
                let (keys, pointers) = node(&fork.bytes, ROOT_HEADER, count, sb).ok_or(bad)?;
                Root::Tree {
                    level,
                    keys,
                    pointers,
                }
            },
            Format::Local | Format::Device => return Err(bad),
        };
        Ok(Self { inode, root })
    }

    /// Maps the run of at most `max` blocks (at least 1) from file block
    /// `logical`, reading the B+ tree's blocks on the way.
    pub(crate) async fn map<S: ReadAt + ?Sized>(
        &self,
        source: &S,
        sb: &Superblock,
        logical: u64,
        max: u64,
        op: OpKind,
    ) -> Result<Run, Error> {
        let fail = |error: XfsError| error.into_error(op);
        let bad = || fail(XfsError::BadMap { inode: self.inode });
        let max = max.max(1);
        let (mut level, keys, pointers) = match &self.root {
            Root::List(extents) => return run(extents, logical, max, None, sb).ok_or_else(bad),
            Root::Tree {
                level,
                keys,
                pointers,
            } => (*level, keys, pointers),
        };
        let (mut pointer, mut upper) = match step(keys, pointers, logical, None) {
            Step::Hole(len) => return hole(len, max).ok_or_else(bad),
            Step::Child { pointer, upper } => (pointer, upper),
        };
        let block_len = usize::try_from(sb.block_size()).map_err(|_| bad())?;
        let (magic, header_len) = if sb.is_v5() {
            (BMA3_MAGIC, BLOCK_HEADER_V3)
        } else {
            (BMAP_MAGIC, BLOCK_HEADER)
        };
        loop {
            level = level.checked_sub(1).ok_or_else(bad)?;
            let at = sb
                .linear_block(pointer)
                .and_then(|block| sb.block_offset(block))
                .ok_or_else(|| {
                    fail(XfsError::BadBlockNumber {
                        inode: self.inode,
                        block: pointer,
                    })
                })?;
            let block = read_exact(source, at, block_len, op).await?;
            let header = (
                be::u32_at(&block, 0),
                be::u16_at(&block, 4),
                be::u16_at(&block, 6),
            );
            let (Some(found_magic), Some(found), Some(count)) = header else {
                return Err(bad());
            };
            if found_magic != magic || found != level {
                return Err(bad());
            }
            if sb.is_v5() {
                crc::check(&block, crc::BMAP, sb, at, self.inode).map_err(fail)?;
            }
            if level == 0 {
                let len = usize::from(count).checked_mul(EXTENT_SIZE);
                let records = len
                    .and_then(|len| block.get(header_len..header_len.checked_add(len)?))
                    .ok_or_else(bad)?;
                let extents: Vec<Extent> = decode_all(records).collect();
                check(&extents, upper, sb).ok_or_else(bad)?;
                return run(&extents, logical, max, upper, sb).ok_or_else(bad);
            }
            let (keys, pointers) = node(&block, header_len, count, sb).ok_or_else(bad)?;
            match step(&keys, &pointers, logical, upper) {
                Step::Hole(len) => return hole(len, max).ok_or_else(bad),
                Step::Child {
                    pointer: next,
                    upper: next_upper,
                } => {
                    pointer = next;
                    upper = next_upper;
                },
            }
        }
    }

    /// Reads up to `len` bytes of the file's `size` from `off`: fewer only
    /// at its end. Holes and unwritten extents read as zeros.
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

/// A file map on its volume, as [`map::read`] reads through it. Its runs'
/// blocks are linear blocks of the volume.
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
        XfsError::BadMap {
            inode: self.map.inode,
        }
        .into_error(op)
    }
}

/// Decodes the `count` keys and pointers of a B+ tree node in `bytes`,
/// whose header is `header` bytes: the keys follow it, and the pointers
/// start where as many keys as the node can hold would end. Keys must
/// ascend, and pointers name blocks in the volume.
fn node(bytes: &[u8], header: usize, count: u16, sb: &Superblock) -> Option<(Vec<u64>, Vec<u64>)> {
    let capacity = bytes
        .len()
        .checked_sub(header)?
        .checked_div(KEY_SIZE.checked_mul(2)?)?;
    let count = usize::from(count);
    if count == 0 || count > capacity {
        return None;
    }
    let field = |base: usize, i: usize| be::u64_at(bytes, base.checked_add(i.checked_mul(8)?)?);
    let pointers_at = header.checked_add(capacity.checked_mul(KEY_SIZE)?)?;
    let mut keys = Vec::with_capacity(count);
    let mut pointers = Vec::with_capacity(count);
    for i in 0..count {
        let key = field(header, i)?;
        if keys.last().is_some_and(|&last| key <= last) || key >= MAX_FILE_BLOCKS {
            return None;
        }
        let pointer = field(pointers_at, i)?;
        sb.linear_block(pointer)?;
        keys.push(key);
        pointers.push(pointer);
    }
    Some((keys, pointers))
}

/// Chooses where to go from a node of `keys` and `pointers` to find file
/// block `logical`, given that the node's records end below `upper`.
fn step(keys: &[u64], pointers: &[u64], logical: u64, upper: Option<u64>) -> Step {
    // The last key at or below `logical`; before the first, a hole.
    let index = keys.partition_point(|&key| key <= logical);
    let Some(chosen) = index.checked_sub(1) else {
        let first = keys.first().copied().unwrap_or(logical);
        return Step::Hole(first.saturating_sub(logical));
    };
    let pointer = pointers.get(chosen).copied().unwrap_or(0);
    let upper = keys.get(index).copied().or(upper);
    Step::Child { pointer, upper }
}

/// Returns the run of a hole of `len` blocks, at most `max`, if it is not
/// empty.
fn hole(len: u64, max: u64) -> Option<Run> {
    (len > 0).then_some(Run {
        len: len.min(max),
        physical: None,
    })
}

/// Checks that `extents` ascend without overlapping, below `upper` if
/// given, and that each is a run of blocks within one AG of the volume.
fn check(extents: &[Extent], upper: Option<u64>, sb: &Superblock) -> Option<()> {
    let mut next = 0;
    for extent in extents {
        if extent.len == 0 || extent.logical < next || extent.end() > MAX_FILE_BLOCKS {
            return None;
        }
        sb.linear_extent(extent.physical, extent.len)?;
        next = extent.end();
    }
    upper.is_none_or(|upper| next <= upper).then_some(())
}

/// Returns the run of at most `max` blocks from file block `logical` in
/// the checked `extents`, past which the next extent is at `upper`, if
/// any.
fn run(
    extents: &[Extent],
    logical: u64,
    max: u64,
    upper: Option<u64>,
    sb: &Superblock,
) -> Option<Run> {
    let Some(extent) = extents.iter().find(|extent| extent.end() > logical) else {
        return hole(
            upper.map_or(max, |upper| upper.saturating_sub(logical)),
            max,
        );
    };
    if extent.logical > logical {
        return hole(extent.logical.saturating_sub(logical), max);
    }
    let skip = logical.checked_sub(extent.logical)?;
    let len = extent.end().saturating_sub(logical).min(max);
    let physical = if extent.unwritten {
        None
    } else {
        let start = sb.linear_extent(extent.physical, extent.len)?;
        Some(start.checked_add(skip)?)
    };
    Some(Run { len, physical })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::{
        inode::tests::Raw,
        superblock::{Version, tests::Fields},
    };
    use alloc::vec;
    use filer_api::ErrorKind;
    use filer_spec::block_on;

    /// Encodes an extent record.
    pub(crate) fn record(logical: u64, physical: u64, len: u64, unwritten: bool) -> [u8; 16] {
        let hi = u64::from(unwritten) << 63 | logical << 9 | physical >> 43;
        let lo = (physical & ((1 << 43) - 1)) << 21 | len;
        let mut b = [0; 16];
        b[..8].copy_from_slice(&hi.to_be_bytes());
        b[8..].copy_from_slice(&lo.to_be_bytes());
        b
    }

    fn superblock() -> Superblock {
        Superblock::parse(&Fields::new().bytes()).unwrap()
    }

    /// A file of `size` bytes mapped by `records` in the inode.
    fn listed(size: u64, records: &[[u8; 16]]) -> Inode {
        let mut raw = Raw::file(size);
        raw.extents = u32::try_from(records.len()).unwrap();
        raw.literal = records.concat();
        Inode::parse(&raw.bytes(256), 131, Version::V4).unwrap()
    }

    /// An image of `Fields::new()`'s volume whose block `n` is filled with
    /// `n`.
    fn image() -> Vec<u8> {
        let mut image = vec![0; 4096 * 1024];
        for (n, block) in image.chunks_mut(4096).enumerate() {
            block.fill(u8::try_from(n % 251).unwrap());
        }
        image
    }

    #[test]
    fn decodes_extent_records() {
        // The book's example: 2025 blocks at file block 4050, volume block
        // 35481.
        let bytes = [
            0, 0, 0, 0, 0, 0x1F, 0xA4, 0, 0, 0, 0, 0x11, 0x53, 0x20, 0x07, 0xE9,
        ];
        let extent = Extent::decode(&bytes);
        assert_eq!(
            extent,
            Extent {
                logical: 4050,
                physical: 35481,
                len: 2025,
                unwritten: false,
            }
        );
        let wide = record((1 << 54) - 1, (1 << 52) - 1, (1 << 21) - 1, true);
        assert_eq!(
            Extent::decode(&wide),
            Extent {
                logical: (1 << 54) - 1,
                physical: (1 << 52) - 1,
                len: (1 << 21) - 1,
                unwritten: true,
            }
        );
    }

    #[test]
    fn maps_an_extent_list() {
        let sb = superblock();
        // Blocks 0-1 at 10, a hole, 4-5 unwritten, 6 in AG 1's block 3.
        let inode = listed(
            8 * 4096,
            &[
                record(0, 10, 2, false),
                record(4, 20, 2, true),
                record(6, (1 << 8) | 3, 1, false),
            ],
        );
        let map = FileMap::data(&inode, &sb).unwrap();
        let at = |logical, max| block_on(map.map(&[0u8; 0][..], &sb, logical, max, OpKind::Read));
        let data = |len, physical| Run {
            len,
            physical: Some(physical),
        };
        let zeros = |len| Run {
            len,
            physical: None,
        };
        assert_eq!(at(0, 10).unwrap(), data(2, 10));
        assert_eq!(at(1, 10).unwrap(), data(1, 11));
        assert_eq!(at(0, 1).unwrap(), data(1, 10));
        assert_eq!(at(2, 10).unwrap(), zeros(2));
        assert_eq!(at(4, 10).unwrap(), zeros(2));
        assert_eq!(at(6, 10).unwrap(), data(1, 259));
        // Past the last extent, holes to the end.
        assert_eq!(at(7, 10).unwrap(), zeros(10));
    }

    #[test]
    fn reads_through_holes_and_unwritten_extents() {
        let sb = superblock();
        let image = image();
        let inode = listed(
            5 * 4096 + 100,
            &[
                record(0, 10, 2, false),
                record(3, 20, 1, true),
                record(4, 30, 2, false),
            ],
        );
        let map = FileMap::data(&inode, &sb).unwrap();
        let read = |off, len| {
            block_on(map.read(&image, &sb, inode.size(), off, len, OpKind::Read)).unwrap()
        };
        let all = read(0, 1 << 20);
        assert_eq!(all.len(), 5 * 4096 + 100);
        assert!(all[..4096].iter().all(|&b| b == 10));
        assert!(all[4096..8192].iter().all(|&b| b == 11));
        assert!(all[8192..4 * 4096].iter().all(|&b| b == 0));
        assert!(all[4 * 4096..5 * 4096].iter().all(|&b| b == 30));
        assert!(all[5 * 4096..].iter().all(|&b| b == 31));
        assert_eq!(read(4000, 200)[..96], [10; 96]);
        assert_eq!(read(4000, 200)[96..], [11; 104]);
        assert_eq!(read(5 * 4096 + 99, 10), [31]);
        assert!(read(5 * 4096 + 100, 10).is_empty());
    }

    /// Writes a `BMAP` block at linear block `at` of `image`: a leaf of
    /// `records`, or a node of `keys` and `pointers`.
    fn bmap_block(
        image: &mut [u8],
        at: usize,
        level: u16,
        entries: &[(u64, u64)],
        leaf: &[[u8; 16]],
    ) {
        let block = &mut image[at * 4096..(at + 1) * 4096];
        block.fill(0);
        block[..4].copy_from_slice(&BMAP_MAGIC.to_be_bytes());
        block[4..6].copy_from_slice(&level.to_be_bytes());
        let count = if level == 0 {
            leaf.len()
        } else {
            entries.len()
        };
        block[6..8].copy_from_slice(&u16::try_from(count).unwrap().to_be_bytes());
        block[8..24].fill(0xFF);
        if level == 0 {
            block[24..24 + 16 * leaf.len()].copy_from_slice(&leaf.concat());
        } else {
            let capacity = (4096 - 24) / 16;
            for (i, (key, pointer)) in entries.iter().enumerate() {
                block[24 + 8 * i..32 + 8 * i].copy_from_slice(&key.to_be_bytes());
                let p = 24 + 8 * capacity + 8 * i;
                block[p..p + 8].copy_from_slice(&pointer.to_be_bytes());
            }
        }
    }

    /// A file of `size` bytes whose map is a B+ tree of `level` levels
    /// below a root of `entries`, in a 156-byte data fork.
    fn tree(size: u64, level: u16, entries: &[(u64, u64)]) -> Inode {
        let mut raw = Raw::file(size);
        raw.format = 3;
        raw.extents = 1;
        let mut root = vec![0; 156];
        root[..2].copy_from_slice(&level.to_be_bytes());
        root[2..4].copy_from_slice(&u16::try_from(entries.len()).unwrap().to_be_bytes());
        // Nine entries fit: (156 - 4) / 16.
        for (i, (key, pointer)) in entries.iter().enumerate() {
            root[4 + 8 * i..12 + 8 * i].copy_from_slice(&key.to_be_bytes());
            root[76 + 8 * i..84 + 8 * i].copy_from_slice(&pointer.to_be_bytes());
        }
        raw.literal = root;
        Inode::parse(&raw.bytes(256), 131, Version::V4).unwrap()
    }

    #[test]
    fn maps_a_b_tree() {
        let sb = superblock();
        let mut image = image();
        // Two levels: a node at block 100 over leaves at 101 and 102.
        bmap_block(&mut image, 100, 1, &[(0, 101), (50, 102)], &[]);
        bmap_block(
            &mut image,
            101,
            0,
            &[],
            &[record(0, 10, 2, false), record(10, 20, 5, false)],
        );
        bmap_block(&mut image, 102, 0, &[], &[record(60, 40, 4, false)]);
        let inode = tree(100 * 4096, 2, &[(0, 100)]);
        let map = FileMap::data(&inode, &sb).unwrap();
        let at = |logical, max| block_on(map.map(&image, &sb, logical, max, OpKind::Read)).unwrap();
        let data = |len, physical| Run {
            len,
            physical: Some(physical),
        };
        let zeros = |len| Run {
            len,
            physical: None,
        };
        assert_eq!(at(1, 100), data(1, 11));
        assert_eq!(at(2, 100), zeros(8));
        assert_eq!(at(12, 100), data(3, 22));
        // The hole past a leaf's last extent ends at the next leaf's key.
        assert_eq!(at(15, 100), zeros(35));
        assert_eq!(at(50, 100), zeros(10));
        assert_eq!(at(61, 100), data(3, 41));
        assert_eq!(at(64, 100), zeros(100));

        // A file starting with a hole, before the root's first key.
        let inode = tree(100 * 4096, 2, &[(5, 100)]);
        let map = FileMap::data(&inode, &sb).unwrap();
        let run = block_on(map.map(&image, &sb, 0, 100, OpKind::Read)).unwrap();
        assert_eq!(run, zeros(5));
    }

    #[test]
    fn rejects_bad_b_trees() {
        let sb = superblock();
        let image = image();
        let corrupt = |image: &[u8], inode: &Inode| {
            let map = FileMap::data(inode, &sb).unwrap();
            let error = block_on(map.map(image, &sb, 0, 1, OpKind::Read)).unwrap_err();
            assert_eq!(error.kind(), ErrorKind::Corrupt);
        };
        // No magic.
        corrupt(&image, &tree(4096, 1, &[(0, 100)]));
        // The wrong level.
        let mut bad = image.clone();
        bmap_block(&mut bad, 100, 1, &[(0, 101)], &[]);
        corrupt(&bad, &tree(4096, 1, &[(0, 100)]));
        // Unsorted records.
        let mut bad = image.clone();
        bmap_block(
            &mut bad,
            100,
            0,
            &[],
            &[record(5, 10, 2, false), record(0, 20, 2, false)],
        );
        corrupt(&bad, &tree(4096, 1, &[(0, 100)]));
        // Records past the next key.
        let mut bad = image.clone();
        bmap_block(&mut bad, 100, 0, &[], &[record(0, 10, 20, false)]);
        let inode = tree(4096, 1, &[(0, 100), (10, 101)]);
        corrupt(&bad, &inode);

        // Roots that cannot be: level 0, too deep, unsorted keys, too many
        // entries, or a pointer outside the volume.
        let check = |inode: Inode| {
            assert_eq!(
                FileMap::data(&inode, &sb),
                Err(XfsError::BadMap { inode: 131 })
            );
        };
        check(tree(4096, 0, &[(0, 100)]));
        check(tree(4096, 17, &[(0, 100)]));
        check(tree(4096, 1, &[(5, 100), (5, 101)]));
        check(tree(4096, 1, &[(0, 4 << 8)]));
        check(tree(4096, 1, &[]));
        let mut raw = Raw::file(4096);
        raw.format = 3;
        raw.literal = vec![0, 1, 0, 10];
        check(Inode::parse(&raw.bytes(256), 131, Version::V4).unwrap());
    }

    #[test]
    fn rejects_bad_extent_lists() {
        let sb = superblock();
        let check = |records: &[[u8; 16]]| {
            assert_eq!(
                FileMap::data(&listed(4096, records), &sb),
                Err(XfsError::BadMap { inode: 131 })
            );
        };
        check(&[record(0, 10, 0, false)]);
        check(&[record(0, 10, 2, false), record(1, 20, 1, false)]);
        check(&[record(5, 10, 1, false), record(0, 20, 1, false)]);
        // Outside the volume, crossing AGs, or past an AG's blocks.
        check(&[record(0, 4 << 8, 1, false)]);
        check(&[record(0, 255, 2, false)]);
        let mut f = Fields::new();
        f.ag_blocks = 200;
        f.blocks = 800;
        f.log_blocks = 100;
        let sb = Superblock::parse(&f.bytes()).unwrap();
        assert!(FileMap::data(&listed(4096, &[record(0, 199, 1, false)]), &sb).is_ok());
        assert_eq!(
            FileMap::data(&listed(4096, &[record(0, 199, 2, false)]), &sb),
            Err(XfsError::BadMap { inode: 131 })
        );
    }

    #[test]
    fn refuses_realtime_data() {
        let mut raw = Raw::file(4096);
        raw.flags = REALTIME;
        let inode = Inode::parse(&raw.bytes(256), 131, Version::V4).unwrap();
        let error = FileMap::data(&inode, &superblock()).unwrap_err();
        assert_eq!(error, XfsError::RealtimeData { inode: 131 });
        assert_eq!(error.kind(), ErrorKind::Unsupported);
    }
}

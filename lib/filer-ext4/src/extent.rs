// This is free and unencumbered software released into the public domain.

//! Extent trees: how ext4 maps a file's blocks, as runs of contiguous
//! blocks rather than a number per block.
//!
//! The tree's root is `i_block`: a header (the magic `0xF30A`, the count
//! and limit of entries, and the depth below), then four entries; its
//! other nodes are blocks of the same form, with a checksum after their
//! entries under `metadata_csum`. Entries of a leaf, at depth 0, are
//! extents: a first file block, a length, and a first volume block (48
//! bits), a length past 32768 marking an extent allocated but not yet
//! written, which reads as zeros. Entries of an interior node are
//! indexes: the first file block below, and the node holding it. Both are
//! in order of file block, and file blocks no extent holds are holes.
//!
//! Like a block map, a tree is followed lazily, a run at a time, down
//! one path of at most five levels; no file's whole map is ever held.

use crate::{Ext4Error, csum, inode::Inode, superblock::Superblock};
use filer_api::{Error, OpKind, ReadAt, checksum::Crc32c};
use filer_fs::{le, map::Run, read_exact};

/// A node's magic number, and the size of its header and of an entry.
const MAGIC: u16 = 0xF30A;
const HEADER: usize = 12;
const ENTRY: usize = 12;

/// The deepest tree.
const MAX_DEPTH: u16 = 5;

/// The longest extent written; longer ones are unwritten, by this much.
const MAX_WRITTEN: u16 = 32768;

/// The extent tree of a file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExtentTree {
    inode: u32,
    /// `i_block`, the root.
    root: [u8; 60],
    /// The seed of its nodes' checksums, under `metadata_csum`.
    seed: Option<Crc32c>,
}

/// A node's header, validated against its node's length.
struct Header {
    entries: usize,
    /// The most entries the node has room for, after which its checksum
    /// is.
    max: usize,
    depth: u16,
}

impl Header {
    fn parse(node: &[u8]) -> Option<Self> {
        let entries = usize::from(le::u16_at(node, 2)?);
        let max = usize::from(le::u16_at(node, 4)?);
        let depth = le::u16_at(node, 6)?;
        let room = max.checked_mul(ENTRY)?.checked_add(HEADER)?;
        if le::u16_at(node, 0)? != MAGIC || entries > max || room > node.len() || depth > MAX_DEPTH
        {
            return None;
        }
        Some(Self {
            entries,
            max,
            depth,
        })
    }
}

/// Returns entry `i` of a node: its first file block, and the rest.
fn entry(node: &[u8], i: usize) -> Option<(u32, &[u8])> {
    let at = i.checked_mul(ENTRY)?.checked_add(HEADER)?;
    let bytes = node.get(at..at.checked_add(ENTRY)?)?;
    Some((le::u32_at(bytes, 0)?, bytes))
}

/// Returns a 48-bit block number, of its high 16 bits and low 32.
fn block(high: u16, low: u32) -> u64 {
    let [h0, h1] = high.to_le_bytes();
    let [l0, l1, l2, l3] = low.to_le_bytes();
    u64::from_le_bytes([l0, l1, l2, l3, h0, h1, 0, 0])
}

impl ExtentTree {
    /// Returns the extent tree of `inode`, which must address every block
    /// of its size: at most 2^32.
    pub(crate) fn new(inode: &Inode, sb: &Superblock) -> Result<Self, Ext4Error> {
        let number = inode.number();
        let blocks = inode.size().div_ceil(u64::from(sb.block_size()));
        if blocks > 1 << 32 {
            return Err(Ext4Error::FileTooLarge { inode: number });
        }
        Ok(Self {
            inode: number,
            root: *inode.block(),
            seed: sb.checksums().inode_seed(number, inode.generation()),
        })
    }

    /// Returns the inode whose tree this is.
    pub(crate) const fn inode(&self) -> u32 {
        self.inode
    }

    /// Maps the run of at most `max` blocks (at least 1) from file block
    /// `logical`, reading the nodes on the way.
    pub(crate) async fn map<S: ReadAt + ?Sized>(
        &self,
        source: &S,
        sb: &Superblock,
        logical: u64,
        max: u64,
        op: OpKind,
    ) -> Result<Run, Error> {
        let bad = || Ext4Error::BadExtentTree { inode: self.inode }.into_error(op);
        let max = max.max(1);
        let mut node = self.root.to_vec();
        // Where the subtree followed ends: the next index's first block.
        let mut limit = u64::MAX;
        let mut expected = None;
        loop {
            let header = Header::parse(&node).ok_or_else(bad)?;
            if expected.is_some_and(|depth| depth != header.depth) {
                return Err(bad());
            }
            // The last entry from `logical` or before, and the next's start.
            let mut found = None;
            let mut next = limit;
            let mut previous = None;
            for i in 0..header.entries {
                let (first, _) = entry(&node, i).ok_or_else(bad)?;
                let first = u64::from(first);
                if previous.is_some_and(|previous| first <= previous) {
                    return Err(bad());
                }
                previous = Some(first);
                if first <= logical {
                    found = Some(i);
                } else {
                    next = next.min(first);
                    break;
                }
            }
            if header.depth == 0 {
                return self
                    .leaf(&node, found, logical, next, max, sb)
                    .map_err(|error| error.into_error(op));
            }
            // An index: down to the node holding `logical`, or a hole to
            // the first index's start.
            let Some(i) = found else {
                let len = next.saturating_sub(logical).min(max);
                return Ok(Run {
                    len: len.max(1),
                    physical: None,
                });
            };
            let (_, bytes) = entry(&node, i).ok_or_else(bad)?;
            let low = le::u32_at(bytes, 4).ok_or_else(bad)?;
            let high = le::u16_at(bytes, 8).ok_or_else(bad)?;
            let child = block(high, low);
            let at = self
                .check(child, 1, sb)
                .map_err(|error| error.into_error(op))?;
            let len = usize::try_from(sb.block_size()).map_err(|_| bad())?;
            node = read_exact(source, at, len, op).await?;
            if let Some(seed) = self.seed {
                let checksum = Header::parse(&node).and_then(|header| {
                    let at = header.max.checked_mul(ENTRY)?.checked_add(HEADER)?;
                    Some((node.get(..at)?, le::u32_at(&node, at)?))
                });
                if !checksum
                    .is_some_and(|(covered, stored)| csum::chained_ok(seed, covered, &[], stored))
                {
                    return Err(Ext4Error::BadExtentChecksum {
                        inode: self.inode,
                        block: child,
                    }
                    .into_error(op));
                }
            }
            limit = next;
            expected = Some(header.depth.saturating_sub(1));
        }
    }

    /// Returns the run from `logical` in leaf `node`, whose entry `found`
    /// is the last from `logical` or before, and whose blocks from `next`
    /// are another extent's or another subtree's.
    fn leaf(
        &self,
        node: &[u8],
        found: Option<usize>,
        logical: u64,
        next: u64,
        max: u64,
        sb: &Superblock,
    ) -> Result<Run, Ext4Error> {
        let bad = Ext4Error::BadExtentTree { inode: self.inode };
        let hole = |end: u64| Run {
            len: end.saturating_sub(logical).clamp(1, max),
            physical: None,
        };
        let Some(i) = found else {
            return Ok(hole(next));
        };
        let (first, bytes) = entry(node, i).ok_or(bad)?;
        let first = u64::from(first);
        let len = le::u16_at(bytes, 4).ok_or(bad)?;
        let (len, written) = match len.checked_sub(MAX_WRITTEN) {
            Some(unwritten) if unwritten > 0 => (unwritten, false),
            _ => (len, true),
        };
        let end = first.saturating_add(u64::from(len));
        if len == 0 || end > next {
            return Err(bad);
        }
        if logical >= end {
            return Ok(hole(next));
        }
        let high = le::u16_at(bytes, 6).ok_or(bad)?;
        let low = le::u32_at(bytes, 8).ok_or(bad)?;
        let start = block(high, low);
        self.check(start, u64::from(len), sb)?;
        let within = logical.saturating_sub(first);
        let run = end.saturating_sub(logical).min(max);
        Ok(Run {
            len: run,
            physical: written.then(|| start.saturating_add(within)),
        })
    }

    /// Returns the offset of the `len` blocks from `block`, which must
    /// all be in the volume, past the superblock.
    fn check(&self, block: u64, len: u64, sb: &Superblock) -> Result<u64, Ext4Error> {
        let bad = Ext4Error::BadBlockNumber {
            inode: self.inode,
            block,
        };
        let end = block.checked_add(len).ok_or(bad)?;
        if block <= u64::from(sb.first_data_block()) || end > sb.blocks() {
            return Err(bad);
        }
        sb.block_offset(block).ok_or(bad)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{inode::tests::Raw, superblock::tests::Fields};
    use alloc::{vec, vec::Vec};
    use filer_api::ErrorKind;
    use filer_spec::block_on;

    /// A node of `depth` holding `entries`, with room for `max`, in `len`
    /// bytes.
    fn node(depth: u16, entries: &[[u8; 12]], max: u16, len: usize) -> Vec<u8> {
        let mut b = vec![0; len];
        b[..2].copy_from_slice(&MAGIC.to_le_bytes());
        b[2..4].copy_from_slice(&u16::try_from(entries.len()).unwrap().to_le_bytes());
        b[4..6].copy_from_slice(&max.to_le_bytes());
        b[6..8].copy_from_slice(&depth.to_le_bytes());
        for (i, entry) in entries.iter().enumerate() {
            b[HEADER + i * ENTRY..][..ENTRY].copy_from_slice(entry);
        }
        b
    }

    fn extent(first: u32, len: u16, start: u32) -> [u8; 12] {
        let mut e = [0; 12];
        e[..4].copy_from_slice(&first.to_le_bytes());
        e[4..6].copy_from_slice(&len.to_le_bytes());
        e[8..].copy_from_slice(&start.to_le_bytes());
        e
    }

    fn index(first: u32, leaf: u32) -> [u8; 12] {
        let mut e = [0; 12];
        e[..4].copy_from_slice(&first.to_le_bytes());
        e[4..8].copy_from_slice(&leaf.to_le_bytes());
        e
    }

    /// A test volume of 4096 blocks of 1 KiB, holding nodes at blocks.
    struct Volume {
        sb: Superblock,
        image: Vec<u8>,
    }

    impl Volume {
        fn new(nodes: &[(usize, Vec<u8>)]) -> Self {
            let sb = Superblock::parse(&Fields::new().bytes()).unwrap();
            let mut image = vec![0; 4096 * 1024];
            for (block, node) in nodes {
                image[block * 1024..][..node.len()].copy_from_slice(node);
            }
            Self { sb, image }
        }

        fn tree(&self, root: &[u8]) -> ExtentTree {
            let mut raw = Raw::file(1 << 20);
            raw.flags = 0x8_0000;
            raw.block.copy_from_slice(root);
            let inode = Inode::parse(&raw.bytes(256), 12).unwrap();
            ExtentTree::new(&inode, &self.sb).unwrap()
        }

        fn map(&self, tree: &ExtentTree, logical: u64, max: u64) -> Result<Run, Error> {
            block_on(tree.map(&self.image[..], &self.sb, logical, max, OpKind::Read))
        }
    }

    fn run(len: u64, physical: Option<u64>) -> Run {
        Run { len, physical }
    }

    #[test]
    fn maps_extents_holes_and_unwritten_extents() {
        let v = Volume::new(&[]);
        let root = node(
            0,
            &[
                extent(0, 4, 100),
                extent(10, MAX_WRITTEN + 2, 200),
                extent(20, 1, 300),
            ],
            4,
            60,
        );
        let tree = v.tree(&root);
        assert_eq!(v.map(&tree, 0, 10).unwrap(), run(4, Some(100)));
        assert_eq!(v.map(&tree, 2, 1).unwrap(), run(1, Some(102)));
        assert_eq!(v.map(&tree, 4, 100).unwrap(), run(6, None));
        assert_eq!(v.map(&tree, 10, 5).unwrap(), run(2, None));
        assert_eq!(v.map(&tree, 12, 100).unwrap(), run(8, None));
        assert_eq!(v.map(&tree, 25, 7).unwrap(), run(7, None));
    }

    #[test]
    fn follows_index_nodes() {
        let v = Volume::new(&[
            (50, node(0, &[extent(0, 10, 500)], 84, 1024)),
            (51, node(0, &[extent(100, 1, 600)], 84, 1024)),
        ]);
        let tree = v.tree(&node(1, &[index(0, 50), index(100, 51)], 4, 60));
        assert_eq!(v.map(&tree, 5, 1000).unwrap(), run(5, Some(505)));
        // A hole up to the next index's first block, not past it.
        assert_eq!(v.map(&tree, 10, 1000).unwrap(), run(90, None));
        assert_eq!(v.map(&tree, 100, 1000).unwrap(), run(1, Some(600)));
        assert_eq!(v.map(&tree, 101, 5).unwrap(), run(5, None));
    }

    #[test]
    fn refuses_malformed_trees() {
        let bad = |v: &Volume, root: &[u8], logical| {
            let error = v.map(&v.tree(root), logical, 1).unwrap_err();
            assert_eq!(error.kind(), ErrorKind::Corrupt);
            *core::error::Error::source(&error)
                .unwrap()
                .downcast_ref::<Ext4Error>()
                .unwrap()
        };
        let tree = Ext4Error::BadExtentTree { inode: 12 };
        let v = Volume::new(&[
            (50, node(1, &[extent(0, 1, 500)], 84, 1024)),
            (51, node(0, &[extent(0, 1, 9000)], 84, 1024)),
        ]);
        let mut magic = node(0, &[extent(0, 1, 100)], 4, 60);
        magic[0] = 0;
        assert_eq!(bad(&v, &magic, 0), tree);
        // More entries than room, or room past the root.
        assert_eq!(bad(&v, &node(0, &[extent(0, 1, 100)], 0, 60), 0), tree);
        assert_eq!(bad(&v, &node(0, &[], 5, 60), 0), tree);
        // Out of order, overlapping, or empty extents.
        let unordered = node(0, &[extent(5, 1, 100), extent(2, 1, 200)], 4, 60);
        assert_eq!(bad(&v, &unordered, 6), tree);
        let overlapping = node(0, &[extent(0, 8, 100), extent(4, 1, 200)], 4, 60);
        assert_eq!(bad(&v, &overlapping, 1), tree);
        assert_eq!(bad(&v, &node(0, &[extent(0, 0, 100)], 4, 60), 0), tree);
        // A child at the wrong depth.
        assert_eq!(bad(&v, &node(1, &[index(0, 50)], 4, 60), 0), tree);
        // Blocks outside the volume.
        assert_eq!(
            bad(&v, &node(1, &[index(0, 51)], 4, 60), 0),
            Ext4Error::BadBlockNumber {
                inode: 12,
                block: 9000
            }
        );
        assert_eq!(
            bad(&v, &node(1, &[index(0, 5000)], 4, 60), 0),
            Ext4Error::BadBlockNumber {
                inode: 12,
                block: 5000
            }
        );
    }
}

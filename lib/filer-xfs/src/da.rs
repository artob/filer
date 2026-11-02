// This is free and unencumbered software released into the public domain.

//! Hash-ordered trees of fork blocks, shared by directories and attribute
//! forks: node blocks above leaves, each block starting with a header of
//! its siblings, `forw` and `back`, and its magic. A node's entries are a
//! hash and a fork block, the hash the greatest of the leaf entries below
//! that block; leaves at one level are chained by `forw`, so that entries
//! of one hash may run on into the next leaf. With version 5, the header
//! is self-describing (`crc::DA`), and longer.

use alloc::vec::Vec;
use filer_fs::be;

/// A node block's magic number, and with version 5.
pub(crate) const NODE_MAGIC: u16 = 0xFEBE;
pub(crate) const NODE_MAGIC_V3: u16 = 0x3EBE;

/// The sizes of a node block's header, with version 5, and of an entry.
const NODE_HEADER: usize = 16;
const NODE_HEADER_V3: usize = 64;
const NODE_ENTRY: usize = 8;

/// Returns the node magic number of a volume, version 5 if `v5`.
pub(crate) const fn node_magic(v5: bool) -> u16 {
    if v5 { NODE_MAGIC_V3 } else { NODE_MAGIC }
}

/// The levels of the deepest tree read: far more than the 2^32 blocks a
/// fork's tree can address need.
pub(crate) const MAX_LEVEL: u16 = 16;

/// Returns a block's magic number.
pub(crate) fn magic(block: &[u8]) -> Option<u16> {
    be::u16_at(block, 8)
}

/// Returns the fork block of a block's next sibling, 0 for none.
pub(crate) fn forw(block: &[u8]) -> Option<u32> {
    be::u32_at(block, 0)
}

/// A node block: its level above the leaves, and its entries' hashes and
/// the fork blocks below them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Node {
    pub(crate) level: u16,
    pub(crate) entries: Vec<(u32, u32)>,
}

impl Node {
    /// Parses a node block, of version 5 if `v5`: a header holding the
    /// count and the level, then entries, at least one, their hashes
    /// ascending.
    pub(crate) fn parse(block: &[u8], v5: bool) -> Option<Self> {
        if magic(block)? != node_magic(v5) {
            return None;
        }
        // Version 5's header pads the count and level to 8 bytes.
        let (header, count_at) = if v5 {
            (NODE_HEADER_V3, 56)
        } else {
            (NODE_HEADER, 12)
        };
        let count = usize::from(be::u16_at(block, count_at)?);
        let level = be::u16_at(block, count_at.checked_add(2)?)?;
        if count == 0 || level == 0 || level > MAX_LEVEL {
            return None;
        }
        let mut entries = Vec::with_capacity(count);
        for i in 0..count {
            let at = header.checked_add(i.checked_mul(NODE_ENTRY)?)?;
            let hash = be::u32_at(block, at)?;
            let below = be::u32_at(block, at.checked_add(4)?)?;
            if entries.last().is_some_and(|&(last, _)| hash < last) {
                return None;
            }
            entries.push((hash, below));
        }
        Some(Self { level, entries })
    }

    /// Returns the fork block below which entries of `hash` start: that of
    /// the first entry whose hash is at least `hash`, if any.
    pub(crate) fn child(&self, hash: u32) -> Option<u32> {
        let index = self.entries.partition_point(|&(found, _)| found < hash);
        self.entries.get(index).map(|&(_, below)| below)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use alloc::vec;

    /// A node block of `size` bytes, of `level`, over entries of a hash
    /// and a fork block.
    pub(crate) fn node(size: usize, level: u16, entries: &[(u32, u32)]) -> Vec<u8> {
        let mut b = vec![0; size];
        b[8..10].copy_from_slice(&NODE_MAGIC.to_be_bytes());
        b[12..14].copy_from_slice(&u16::try_from(entries.len()).unwrap().to_be_bytes());
        b[14..16].copy_from_slice(&level.to_be_bytes());
        for (i, (hash, below)) in entries.iter().enumerate() {
            let at = NODE_HEADER + NODE_ENTRY * i;
            b[at..at + 4].copy_from_slice(&hash.to_be_bytes());
            b[at + 4..at + 8].copy_from_slice(&below.to_be_bytes());
        }
        b
    }

    #[test]
    fn parses_nodes_and_chooses_children() {
        let node = Node::parse(&node(4096, 1, &[(10, 100), (20, 101), (20, 102)]), false).unwrap();
        assert_eq!(node.level, 1);
        assert_eq!(node.child(0), Some(100));
        assert_eq!(node.child(10), Some(100));
        assert_eq!(node.child(11), Some(101));
        // The first of entries of one hash.
        assert_eq!(node.child(20), Some(101));
        assert_eq!(node.child(21), None);
    }

    #[test]
    fn parses_version_5_nodes() {
        let mut b = vec![0; 4096];
        b[8..10].copy_from_slice(&NODE_MAGIC_V3.to_be_bytes());
        b[56..58].copy_from_slice(&1u16.to_be_bytes());
        b[58..60].copy_from_slice(&2u16.to_be_bytes());
        b[64..72].copy_from_slice(&[0, 0, 0, 9, 0, 0, 0, 7]);
        let node = Node::parse(&b, true).unwrap();
        assert_eq!((node.level, node.entries), (2, vec![(9, 7)]));
        assert_eq!(Node::parse(&b, false), None);
    }

    #[test]
    fn rejects_bad_nodes() {
        let good = node(4096, 1, &[(10, 100)]);
        assert!(Node::parse(&good, false).is_some());
        let mut bad = good.clone();
        bad[8] = 0;
        assert_eq!(Node::parse(&bad, false), None);
        assert_eq!(Node::parse(&node(4096, 0, &[(10, 100)]), false), None);
        assert_eq!(Node::parse(&node(4096, 17, &[(10, 100)]), false), None);
        assert_eq!(Node::parse(&node(4096, 1, &[]), false), None);
        assert_eq!(
            Node::parse(&node(4096, 1, &[(20, 1), (10, 2)]), false),
            None
        );
        let mut bad = good;
        bad[12..14].copy_from_slice(&600u16.to_be_bytes());
        assert_eq!(Node::parse(&bad, false), None);
    }
}

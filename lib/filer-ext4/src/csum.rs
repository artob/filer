// This is free and unencumbered software released into the public domain.

//! Metadata checksums: `metadata_csum`, CRC-32C over nearly every
//! structure, or before it, `gdt_csum`, CRC-16 over group descriptors.
//!
//! Under `metadata_csum`, checksums are CRC-32C registers, as they stand
//! before the final inversion, chained from a seed: the superblock's from
//! the usual start, over its first 1020 bytes; everything else's from the
//! volume's seed, which is the register over the UUID, or under
//! `metadata_csum_seed` one the superblock keeps. A group descriptor's is
//! over its group number and its bytes, its low 16 bits kept. Structures
//! of one inode chain from the inode's seed, the register over its number
//! and generation: the inode's own, over its record, kept in two halves
//! (the high one only where the extra fields reach it); an extent tree
//! node's, over the node to its checksum, after its last possible entry;
//! a directory block's, over the block to a last, unused entry holding
//! it; and an index node's, over its entries in use and a tail after the
//! last possible one, which holds it. An attribute block's chains from
//! the volume's seed over its block number and its bytes.
//!
//! Each checksum's own field reads as zeros while it is computed. Under
//! `gdt_csum`, a group descriptor's checksum is CRC-16/MODBUS over the
//! UUID, the group number, and its bytes but the checksum.

use filer_api::checksum::{Crc16Modbus, Crc32c};
use filer_fs::le;

/// Where a group descriptor keeps its checksum.
const DESC_CHECKSUM: usize = 0x1E;

/// Where an inode keeps its checksum's halves, and where its extra fields
/// must reach for the high half.
const INODE_CHECKSUM_LOW: usize = 0x7C;
const INODE_CHECKSUM_HIGH: usize = 0x82;
const INODE_EXTRA: usize = 0x80;

/// Where an attribute block keeps its checksum.
const XATTR_CHECKSUM: usize = 0x10;

/// The size of the unused entry ending a directory block, and its type.
const DIR_TAIL: usize = 12;
const DIR_TAIL_TYPE: u8 = 0xDE;

/// How a volume's metadata is checksummed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Checksums {
    None,
    /// `gdt_csum`: group descriptors only, over the UUID.
    Descriptors {
        uuid: [u8; 16],
    },
    /// `metadata_csum`, from the volume's seed.
    Metadata {
        seed: u32,
    },
}

/// Returns the register of `crc`, as ext4 keeps checksums.
fn register(crc: Crc32c) -> u32 {
    crc.state()
}

/// Returns `bytes` with `len` bytes at `at` as zeros, if they are there.
fn zeroed(bytes: &[u8], at: usize, len: usize) -> alloc::vec::Vec<u8> {
    let mut copy = bytes.to_vec();
    if let Some(field) = copy.get_mut(at..at.saturating_add(len)) {
        field.fill(0);
    }
    copy
}

/// Returns `true` if the superblock's checksum, its last four bytes,
/// matches its first 1020.
pub(crate) fn superblock_ok(bytes: &[u8]) -> bool {
    let (Some(body), Some(stored)) = (bytes.get(..0x3FC), le::u32_at(bytes, 0x3FC)) else {
        return false;
    };
    register(Crc32c::new().chain(body)) == stored
}

impl Checksums {
    /// Returns `true` if group `group`'s descriptor, `desc`, matches its
    /// checksum, or there is none.
    pub(crate) fn descriptor_ok(&self, group: u32, desc: &[u8]) -> bool {
        let Some(stored) = le::u16_at(desc, DESC_CHECKSUM) else {
            return false;
        };
        let head = desc.get(..DESC_CHECKSUM).unwrap_or_default();
        let tail = desc
            .get(DESC_CHECKSUM.saturating_add(2)..)
            .unwrap_or_default();
        match *self {
            Self::None => true,
            Self::Descriptors { uuid } => {
                let crc = Crc16Modbus::new()
                    .chain(&uuid)
                    .chain(&group.to_le_bytes())
                    .chain(head)
                    .chain(tail);
                crc.finish() == stored
            },
            Self::Metadata { seed } => {
                let crc = Crc32c::from_state(seed)
                    .chain(&group.to_le_bytes())
                    .chain(head)
                    .chain(&[0; 2])
                    .chain(tail);
                let [c0, c1, ..] = register(crc).to_le_bytes();
                u16::from_le_bytes([c0, c1]) == stored
            },
        }
    }

    /// Returns the seed of inode `number`'s structures, of generation
    /// `generation`, under `metadata_csum`.
    pub(crate) fn inode_seed(&self, number: u32, generation: u32) -> Option<Crc32c> {
        match *self {
            Self::Metadata { seed } => Some(
                Crc32c::from_state(seed)
                    .chain(&number.to_le_bytes())
                    .chain(&generation.to_le_bytes()),
            ),
            _ => None,
        }
    }

    /// Returns `true` if attribute block `block`, `bytes`, matches its
    /// checksum, or there is none.
    pub(crate) fn xattr_block_ok(&self, block: u64, bytes: &[u8]) -> bool {
        let Self::Metadata { seed } = *self else {
            return true;
        };
        let Some(stored) = le::u32_at(bytes, XATTR_CHECKSUM) else {
            return false;
        };
        let crc = Crc32c::from_state(seed)
            .chain(&block.to_le_bytes())
            .chain(&zeroed(bytes, XATTR_CHECKSUM, 4));
        register(crc) == stored
    }
}

/// Returns `true` if an inode's record, `bytes`, matches its checksum,
/// from its seed.
pub(crate) fn inode_ok(seed: Crc32c, bytes: &[u8]) -> bool {
    let Some(low) = le::u16_at(bytes, INODE_CHECKSUM_LOW) else {
        return false;
    };
    // The high half is there if the extra fields reach past it.
    let extra = le::u16_at(bytes, INODE_EXTRA).map_or(0, usize::from);
    let high_end = INODE_CHECKSUM_HIGH.saturating_add(2);
    let high = (bytes.len() > INODE_EXTRA && INODE_EXTRA.saturating_add(extra) >= high_end)
        .then(|| le::u16_at(bytes, INODE_CHECKSUM_HIGH))
        .flatten();
    let mut record = zeroed(bytes, INODE_CHECKSUM_LOW, 2);
    if high.is_some()
        && let Some(field) = record.get_mut(INODE_CHECKSUM_HIGH..high_end)
    {
        field.fill(0);
    }
    let [c0, c1, c2, c3] = register(seed.chain(&record)).to_le_bytes();
    match high {
        Some(high) => {
            let ([l0, l1], [h0, h1]) = (low.to_le_bytes(), high.to_le_bytes());
            [c0, c1, c2, c3] == [l0, l1, h0, h1]
        },
        None => u16::from_le_bytes([c0, c1]) == low,
    }
}

/// Returns `true` if `covered`, then the bytes of `then`, match `stored`
/// from an inode's seed.
pub(crate) fn chained_ok(seed: Crc32c, covered: &[u8], then: &[u8], stored: u32) -> bool {
    register(seed.chain(covered).chain(then)) == stored
}

/// Returns `true` if directory block `block` ends in the unused entry
/// that holds its checksum, and the checksum matches from the inode's
/// seed.
pub(crate) fn dir_block_ok(seed: Crc32c, block: &[u8]) -> bool {
    let at = block.len().saturating_sub(DIR_TAIL);
    let Some(tail) = block.get(at..) else {
        return false;
    };
    let is_tail = le::u32_at(tail, 0) == Some(0)
        && le::u16_at(tail, 4) == Some(12)
        && tail.get(6..8) == Some(&[0, DIR_TAIL_TYPE][..]);
    let (Some(body), Some(stored)) = (block.get(..at), le::u32_at(tail, 8)) else {
        return false;
    };
    is_tail && chained_ok(seed, body, &[], stored)
}

/// Returns `true` if an index node of a hashed directory, `block`,
/// matches the checksum in the tail after its last possible entry, over
/// its entries in use and the tail: the root, if `root`, its entries after
/// its information, or an interior node, its entries after an unused
/// entry.
pub(crate) fn dx_ok(seed: Crc32c, block: &[u8], root: bool) -> bool {
    // The root's information is 0x18 bytes in, and says how long it is.
    let entries = if root {
        block
            .get(0x1D)
            .map_or(0, |&len| 0x18usize.saturating_add(usize::from(len)))
    } else {
        8
    };
    let checked = || {
        let limit = usize::from(le::u16_at(block, entries)?);
        let count = usize::from(le::u16_at(block, entries.checked_add(2)?)?);
        let tail = limit.checked_mul(8)?.checked_add(entries)?;
        let used = count.checked_mul(8)?.checked_add(entries)?;
        if count > limit {
            return None;
        }
        let reserved = block.get(tail..tail.checked_add(4)?)?;
        let stored = le::u32_at(block, tail.checked_add(4)?)?;
        let crc = seed
            .chain(block.get(..used)?)
            .chain(reserved)
            .chain(&[0; 4]);
        Some(register(crc) == stored)
    };
    checked().unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    /// Records of 128 bytes, or whose extra fields stop short, keep only
    /// the checksum's low half.
    #[test]
    fn checks_inodes_by_one_half_or_both() {
        let seed = Crc32c::new().chain(b"seed");
        for (len, extra, halves) in [(128, 0, 1), (256, 2, 1), (256, 4, 2), (256, 32, 2)] {
            let mut record = vec![0x5A; len];
            if len > 128 {
                record[0x80..0x82].copy_from_slice(&u16::to_le_bytes(extra));
            }
            record[0x7C..0x7E].fill(0);
            if halves == 2 {
                record[0x82..0x84].fill(0);
            }
            let [c0, c1, c2, c3] = register(seed.chain(&record)).to_le_bytes();
            record[0x7C..0x7E].copy_from_slice(&[c0, c1]);
            if halves == 2 {
                record[0x82..0x84].copy_from_slice(&[c2, c3]);
            }
            assert!(inode_ok(seed, &record), "{len} {extra}");
            let mut flipped = record.clone();
            flipped[0x10] ^= 1;
            assert!(!inode_ok(seed, &flipped), "{len} {extra}");
        }
    }

    #[test]
    fn needs_no_checksums_without_either_feature() {
        assert!(Checksums::None.descriptor_ok(3, &[0; 32]));
        assert!(Checksums::None.xattr_block_ok(3, &[0; 1024]));
        assert_eq!(Checksums::None.inode_seed(2, 0), None);
    }
}

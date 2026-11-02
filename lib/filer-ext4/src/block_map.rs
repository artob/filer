// This is free and unencumbered software released into the public domain.

//! Block maps: how ext2 and ext3 map a file's blocks, through 12 direct
//! block numbers and one indirect, one double indirect, and one triple
//! indirect block, each of which holds a block's worth of block numbers.
//! A block number of 0 is a hole, as is every block below it.
//!
//! Blocks are mapped lazily, a run at a time, reading the indirect blocks
//! on the way: no file's whole map is ever held.

use crate::{Ext4Error, inode::Inode, io, le, superblock::Superblock};
use alloc::vec::Vec;
use filer_api::{Error, OpKind, ReadAt};

/// The number of direct block numbers, and where the indirect, double
/// indirect, and triple indirect ones are in `i_block`.
const DIRECT: usize = 12;
const INDIRECT: usize = 12;
const DOUBLE: usize = 13;
const TRIPLE: usize = 14;

/// A run of a file's blocks: `len` blocks, all stored contiguously from
/// `physical`, or all holes if it is `None`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Run {
    pub(crate) len: u64,
    pub(crate) physical: Option<u64>,
}

/// The block map of a file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BlockMap {
    inode: u32,
    /// `i_block`, as 15 block numbers.
    roots: [u32; 15],
}

/// Where a file block's number is: in `i_block[root]`, if `depth` is 0,
/// or else `offset` blocks into the tree of that depth below it.
struct Location {
    root: usize,
    depth: u32,
    offset: u64,
}

impl BlockMap {
    /// Returns the block map of `inode`, which must address every block
    /// of its size.
    pub(crate) fn new(inode: &Inode, sb: &Superblock) -> Result<Self, Ext4Error> {
        let number = inode.number();
        let too_large = Ext4Error::FileTooLarge { inode: number };
        let blocks = inode.size().div_ceil(u64::from(sb.block_size()));
        if blocks > addressable(sb).ok_or(too_large)? {
            return Err(too_large);
        }
        let mut roots = [0; 15];
        let (numbers, _) = inode.block().as_chunks::<4>();
        for (root, bytes) in roots.iter_mut().zip(numbers) {
            *root = u32::from_le_bytes(*bytes);
        }
        Ok(Self {
            inode: number,
            roots,
        })
    }

    /// Maps the run of at most `max` blocks (at least 1) from file block
    /// `logical`, reading the indirect blocks on the way.
    pub(crate) async fn map<S: ReadAt + ?Sized>(
        &self,
        source: &S,
        sb: &Superblock,
        logical: u64,
        max: u64,
        op: OpKind,
    ) -> Result<Run, Error> {
        let fail = |error: Ext4Error| error.into_error(op);
        let too_large = || fail(Ext4Error::FileTooLarge { inode: self.inode });
        let max = max.max(1);
        let per_block = per_block(sb);
        let Location {
            root,
            mut depth,
            mut offset,
        } = locate(logical, per_block).ok_or_else(too_large)?;
        if depth == 0 {
            let direct = self.roots.get(..DIRECT).unwrap_or_default();
            return self.run(direct, root, max, sb).map_err(fail);
        }
        let mut pointer = self.roots.get(root).copied().unwrap_or(0);
        loop {
            // The blocks below `pointer`, and below each of its entries.
            let span = per_block.checked_pow(depth).ok_or_else(too_large)?;
            let child = per_block
                .checked_pow(depth.saturating_sub(1))
                .ok_or_else(too_large)?;
            if pointer == 0 {
                let len = span.saturating_sub(offset).min(max);
                return Ok(Run {
                    len,
                    physical: None,
                });
            }
            let at = self.check(u64::from(pointer), 1, sb).map_err(fail)?;
            let block = io::read_exact(source, at, block_len(sb)).await?;
            let index = offset.checked_div(child).ok_or_else(too_large)?;
            let index = usize::try_from(index).map_err(|_| too_large())?;
            offset = offset.checked_rem(child).ok_or_else(too_large)?;
            if depth == 1 {
                let (numbers, _) = block.as_chunks::<4>();
                let entries: Vec<u32> = numbers.iter().map(|b| u32::from_le_bytes(*b)).collect();
                return self.run(&entries, index, max, sb).map_err(fail);
            }
            let entry = index.checked_mul(4).ok_or_else(too_large)?;
            pointer = le::u32_at(&block, entry).ok_or_else(too_large)?;
            depth = depth.saturating_sub(1);
        }
    }

    /// Returns the run of at most `max` blocks starting at entry `index` of
    /// the block numbers `entries`: holes while they are 0, or else blocks
    /// while they are contiguous.
    fn run(
        &self,
        entries: &[u32],
        index: usize,
        max: u64,
        sb: &Superblock,
    ) -> Result<Run, Ext4Error> {
        let rest = entries.get(index..).unwrap_or_default();
        let Some(&first) = rest.first() else {
            return Err(Ext4Error::FileTooLarge { inode: self.inode });
        };
        let mut len = 0u64;
        let mut next = first;
        for &entry in rest {
            if entry != next || len >= max {
                break;
            }
            len = len.saturating_add(1);
            if first != 0 {
                next = next.wrapping_add(1);
            }
        }
        if first == 0 {
            return Ok(Run {
                len,
                physical: None,
            });
        }
        self.check(u64::from(first), len, sb)?;
        Ok(Run {
            len,
            physical: Some(u64::from(first)),
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

    /// Reads up to `len` bytes of the file's `size` from `off`: fewer only
    /// at its end. Holes read as zeros.
    pub(crate) async fn read<S: ReadAt + ?Sized>(
        &self,
        source: &S,
        sb: &Superblock,
        size: u64,
        off: u64,
        len: usize,
        op: OpKind,
    ) -> Result<Vec<u8>, Error> {
        let fail = |error: Ext4Error| error.into_error(op);
        let too_large = || fail(Ext4Error::FileTooLarge { inode: self.inode });
        let block = u64::from(sb.block_size());
        let end = u64::try_from(len)
            .ok()
            .and_then(|len| off.checked_add(len))
            .map_or(size, |end| end.min(size));
        let mut out = Vec::new();
        let mut at = off;
        while at < end {
            let logical = at.checked_div(block).ok_or_else(too_large)?;
            let within = at.checked_rem(block).ok_or_else(too_large)?;
            let max = end
                .saturating_sub(at.saturating_sub(within))
                .div_ceil(block);
            let run = self.map(source, sb, logical, max, op).await?;
            let run_end = logical
                .checked_add(run.len)
                .and_then(|blocks| blocks.checked_mul(block))
                .ok_or_else(too_large)?
                .min(end);
            let n = run_end.saturating_sub(at);
            let n = usize::try_from(n).map_err(|_| too_large())?;
            match run.physical {
                None => out.resize(out.len().saturating_add(n), 0),
                Some(physical) => {
                    let from = physical
                        .checked_mul(block)
                        .and_then(|from| from.checked_add(within))
                        .ok_or_else(too_large)?;
                    out.extend(io::read_exact(source, from, n).await?);
                },
            }
            if n == 0 {
                // A run is at least one block, so this cannot happen.
                return Err(too_large());
            }
            at = run_end;
        }
        Ok(out)
    }
}

/// Returns the number of block numbers a block holds.
fn per_block(sb: &Superblock) -> u64 {
    u64::from(sb.block_size()) / 4
}

/// Returns the block size, for reading an indirect block.
fn block_len(sb: &Superblock) -> usize {
    usize::try_from(sb.block_size()).unwrap_or(usize::MAX)
}

/// Returns the number of file blocks a block map addresses.
fn addressable(sb: &Superblock) -> Option<u64> {
    let p = per_block(sb);
    let direct = u64::try_from(DIRECT).ok()?;
    direct
        .checked_add(p)?
        .checked_add(p.checked_pow(2)?)?
        .checked_add(p.checked_pow(3)?)
}

/// Locates file block `logical`, where each block holds `per_block`
/// block numbers.
fn locate(logical: u64, per_block: u64) -> Option<Location> {
    let direct = u64::try_from(DIRECT).ok()?;
    if logical < direct {
        let root = usize::try_from(logical).ok()?;
        return Some(Location {
            root,
            depth: 0,
            offset: 0,
        });
    }
    let mut offset = logical.checked_sub(direct)?;
    for (root, depth) in [(INDIRECT, 1), (DOUBLE, 2), (TRIPLE, 3)] {
        let span = per_block.checked_pow(depth)?;
        if offset < span {
            return Some(Location {
                root,
                depth,
                offset,
            });
        }
        offset = offset.checked_sub(span)?;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{inode::tests::Raw, superblock::tests::Fields};
    use alloc::vec;
    use filer_spec::block_on;

    /// A 4 MiB volume of 1 KiB blocks, so 256 numbers per indirect block.
    struct Volume {
        sb: Superblock,
        image: Vec<u8>,
    }

    impl Volume {
        fn new() -> Self {
            Self {
                sb: Superblock::parse(&Fields::new().bytes()).unwrap(),
                image: vec![0; 4096 * 1024],
            }
        }

        /// Writes block numbers into block `block` from entry `index`.
        fn entries(&mut self, block: usize, index: usize, numbers: &[u32]) {
            for (i, number) in numbers.iter().enumerate() {
                let at = block * 1024 + (index + i) * 4;
                self.image[at..at + 4].copy_from_slice(&number.to_le_bytes());
            }
        }

        /// Fills block `block` with the byte `fill`.
        fn fill(&mut self, block: usize, fill: u8) {
            self.image[block * 1024..(block + 1) * 1024].fill(fill);
        }

        fn map(&self, roots: &[u32; 15], size: u64) -> Result<BlockMap, Ext4Error> {
            let mut raw = Raw::file(size);
            for (i, root) in roots.iter().enumerate() {
                raw.block[i * 4..i * 4 + 4].copy_from_slice(&root.to_le_bytes());
            }
            let inode = Inode::parse(&raw.bytes(256), 12).unwrap();
            BlockMap::new(&inode, &self.sb)
        }

        fn run(&self, map: &BlockMap, logical: u64, max: u64) -> Result<Run, Error> {
            block_on(map.map(&self.image, &self.sb, logical, max, OpKind::Read))
        }
    }

    fn run(len: u64, physical: Option<u64>) -> Run {
        Run { len, physical }
    }

    #[test]
    fn maps_direct_blocks_in_runs() {
        let v = Volume::new();
        let mut roots = [0; 15];
        roots[..6].copy_from_slice(&[100, 101, 102, 0, 0, 50]);
        let map = v.map(&roots, 12 * 1024).unwrap();
        assert_eq!(v.run(&map, 0, 99).unwrap(), run(3, Some(100)));
        assert_eq!(v.run(&map, 1, 99).unwrap(), run(2, Some(101)));
        assert_eq!(v.run(&map, 0, 2).unwrap(), run(2, Some(100)));
        assert_eq!(v.run(&map, 3, 99).unwrap(), run(2, None));
        assert_eq!(v.run(&map, 5, 99).unwrap(), run(1, Some(50)));
        // The rest of the direct blocks are holes.
        assert_eq!(v.run(&map, 6, 99).unwrap(), run(6, None));
    }

    #[test]
    fn maps_indirect_blocks() {
        let mut v = Volume::new();
        v.entries(200, 0, &[300, 301, 0, 400]);
        let mut roots = [0; 15];
        roots[INDIRECT] = 200;
        let map = v.map(&roots, 300 * 1024).unwrap();
        assert_eq!(v.run(&map, 12, 99).unwrap(), run(2, Some(300)));
        assert_eq!(v.run(&map, 14, 99).unwrap(), run(1, None));
        assert_eq!(v.run(&map, 15, 99).unwrap(), run(1, Some(400)));
        // The rest of the indirect block is holes, to its end.
        assert_eq!(v.run(&map, 16, 999).unwrap(), run(252, None));
    }

    #[test]
    fn maps_double_and_triple_indirect_blocks() {
        let mut v = Volume::new();
        // Double: its second indirect block maps file blocks from
        // 12 + 256 + 256.
        v.entries(201, 1, &[202]);
        v.entries(202, 0, &[500, 501]);
        // Triple: the first double indirect block's first indirect block.
        v.entries(210, 0, &[211]);
        v.entries(211, 0, &[212]);
        v.entries(212, 3, &[600]);
        let mut roots = [0; 15];
        roots[DOUBLE] = 201;
        roots[TRIPLE] = 210;
        let triple = 12 + 256 + 256 * 256;
        let map = v.map(&roots, (triple + 10) * 1024).unwrap();
        assert_eq!(v.run(&map, 12 + 256, 999).unwrap(), run(256, None));
        assert_eq!(v.run(&map, 12 + 512, 999).unwrap(), run(2, Some(500)));
        assert_eq!(v.run(&map, triple + 3, 999).unwrap(), run(1, Some(600)));
        assert_eq!(v.run(&map, triple, 999).unwrap(), run(3, None));
        // A hole in the triple indirect block spans its whole subtree.
        assert_eq!(
            v.run(&map, triple + 256 * 256, u64::MAX).unwrap(),
            run(256 * 256, None)
        );
    }

    #[test]
    fn refuses_blocks_outside_the_volume() {
        let mut v = Volume::new();
        let bad = |block| Ext4Error::BadBlockNumber { inode: 12, block };
        let error = |result: Result<Run, Error>| {
            *core::error::Error::source(&result.unwrap_err())
                .unwrap()
                .downcast_ref::<Ext4Error>()
                .unwrap()
        };
        let mut roots = [0; 15];
        roots[..3].copy_from_slice(&[4095, 4096, 1]);
        roots[INDIRECT] = 5000;
        roots[DOUBLE] = 300;
        v.entries(300, 0, &[4094, 4095]);
        let map = v.map(&roots, 600 * 1024).unwrap();
        assert_eq!(v.run(&map, 0, 1).unwrap(), run(1, Some(4095)));
        assert_eq!(error(v.run(&map, 1, 1)), bad(4096));
        // The superblock's block is no file's.
        assert_eq!(error(v.run(&map, 2, 1)), bad(1));
        assert_eq!(error(v.run(&map, 12, 1)), bad(5000));
        // An indirect block's entry outside the volume.
        v.entries(4094, 0, &[u32::MAX]);
        assert_eq!(error(v.run(&map, 12 + 256, 1)), bad(u64::from(u32::MAX)));
    }

    #[test]
    fn refuses_sizes_past_the_map() {
        let v = Volume::new();
        let max = 12 + 256 + 256 * 256 + 256 * 256 * 256;
        assert!(v.map(&[0; 15], max * 1024).is_ok());
        assert_eq!(
            v.map(&[0; 15], max * 1024 + 1),
            Err(Ext4Error::FileTooLarge { inode: 12 })
        );
        let map = v.map(&[0; 15], 1024).unwrap();
        assert_eq!(
            v.run(&map, max, 1).unwrap_err().kind(),
            filer_api::ErrorKind::Corrupt
        );
    }

    #[test]
    fn reads_data_holes_and_partial_blocks() {
        let mut v = Volume::new();
        v.fill(100, 0xAA);
        v.fill(101, 0xBB);
        v.fill(300, 0xCC);
        v.entries(200, 0, &[300]);
        let mut roots = [0; 15];
        roots[..2].copy_from_slice(&[100, 101]);
        roots[INDIRECT] = 200;
        let size = 12 * 1024 + 10;
        let map = v.map(&roots, size).unwrap();
        let read =
            |off, len| block_on(map.read(&v.image, &v.sb, size, off, len, OpKind::Read)).unwrap();
        let all = read(0, usize::MAX);
        assert_eq!(all.len(), 12 * 1024 + 10);
        assert!(all[..1024].iter().all(|&b| b == 0xAA));
        assert!(all[1024..2048].iter().all(|&b| b == 0xBB));
        assert!(all[2048..12 * 1024].iter().all(|&b| b == 0));
        assert!(all[12 * 1024..].iter().all(|&b| b == 0xCC));
        assert_eq!(
            read(1020, 8),
            [0xAA, 0xAA, 0xAA, 0xAA, 0xBB, 0xBB, 0xBB, 0xBB]
        );
        assert_eq!(read(2047, 2), [0xBB, 0]);
        assert_eq!(read(size - 1, 100), [0xCC]);
        assert!(read(size, 100).is_empty());
        assert!(read(u64::MAX, 100).is_empty());
    }
}

// This is free and unencumbered software released into the public domain.

//! Hashed (htree) directory lookups: descending a directory's index to
//! the one block that may hold a name, rather than reading them all.
//!
//! An indexed directory's first block is the index's root, after the
//! `.` and `..` entries: its information (a reserved zero, the hash
//! version, its own length, 8, and the levels of interior nodes below
//! it, at most two), then the entries. Interior nodes are blocks that
//! read as one unused entry, then entries. Entries are a header (the
//! limit and count of entries, and the first entry's block), then pairs of
//! a hash and a block, in order of hash: each block holds the names whose
//! hashes are from its entry's to the next one's. Blocks are counted in
//! the directory; leaves are blocks of entries as any directory has.
//!
//! A hash's lowest bit, clear in a name's, set in an entry's, marks a
//! block continuing the names of the same hash as the block before it,
//! which a lookup reads on to.
//!
//! An index this driver cannot follow, as one of an unknown hash
//! version or one inconsistent with the directory, is not used: such a
//! lookup reads every block instead, as for a directory with no index.

use crate::{
    dir::{DirEntry, Directory},
    hash::{Hasher, Version},
    superblock::Superblock,
};
use alloc::{vec, vec::Vec};
use filer_api::{Error, OpKind, ReadAt};
use filer_fs::le;

/// Where the root keeps its information and entries, and an interior node
/// its entries.
const ROOT_INFO: usize = 0x18;
const ROOT_ENTRIES: usize = 0x20;
const NODE_ENTRIES: usize = 0x8;

/// The length of the root's information.
const INFO_LEN: u8 = 8;

/// The most levels of interior nodes, as `largedir` allows.
const MAX_LEVELS: u8 = 2;

/// The size of an entry.
const ENTRY: usize = 8;

/// What a lookup through the index found.
#[derive(Debug)]
pub(crate) enum Probe {
    Found(DirEntry),
    Absent,
    /// The index cannot be followed.
    Unusable,
}

/// An entry of a node: the least hash of its block, and the block.
#[derive(Clone, Copy, Debug)]
struct Entry {
    hash: u32,
    block: u32,
}

/// Parses the entries at `at` of a node of a directory of `blocks`
/// blocks: `None` if they are inconsistent with it, as when out of order,
/// past the block, or naming a block the directory lacks, or its root.
fn entries(bytes: &[u8], at: usize, blocks: u64) -> Option<Vec<Entry>> {
    let limit = usize::from(le::u16_at(bytes, at)?);
    let count = usize::from(le::u16_at(bytes, at.checked_add(2)?)?);
    let end = limit.checked_mul(ENTRY)?.checked_add(at)?;
    if count == 0 || count > limit || end > bytes.len() {
        return None;
    }
    let mut entries: Vec<Entry> = Vec::with_capacity(count);
    for i in 0..count {
        let at = i.checked_mul(ENTRY)?.checked_add(at)?;
        // The first entry's place holds the header, its hash the least.
        let hash = if i == 0 { 0 } else { le::u32_at(bytes, at)? };
        let block = le::u32_at(bytes, at.checked_add(4)?)?;
        let ordered = entries.last().is_none_or(|last| last.hash <= hash);
        if !ordered || block == 0 || u64::from(block) >= blocks {
            return None;
        }
        entries.push(Entry { hash, block });
    }
    Some(entries)
}

/// Returns the entry of `entries` whose block may hold names of `hash`:
/// the last whose hash is not above it.
fn position(entries: &[Entry], hash: u32) -> usize {
    entries
        .iter()
        .rposition(|entry| entry.hash <= hash)
        .unwrap_or(0)
}

/// Returns the block of the entry `path` ends at.
fn leaf(path: &[(Vec<Entry>, usize)]) -> Option<u32> {
    let (entries, at) = path.last()?;
    entries.get(*at).map(|entry| entry.block)
}

/// Finds the entry `name` in indexed directory `dir`, matching bytes
/// exactly, through its index.
pub(crate) async fn lookup<S: ReadAt + ?Sized>(
    dir: &Directory,
    source: &S,
    sb: &Superblock,
    name: &[u8],
    op: OpKind,
) -> Result<Probe, Error> {
    let blocks = dir.blocks(sb);
    let root = dir.raw_block(source, sb, 0, op).await?;
    let info = |at: usize| root.get(ROOT_INFO.saturating_add(at)).copied();
    let levels = info(6).unwrap_or(u8::MAX);
    let version = info(4).and_then(Version::new);
    let (Some(version), Some(0)) = (version, le::u32_at(&root, ROOT_INFO)) else {
        return Ok(Probe::Unusable);
    };
    if info(5) != Some(INFO_LEN) || levels > MAX_LEVELS {
        return Ok(Probe::Unusable);
    }
    let hasher = Hasher {
        version,
        unsigned: sb.unsigned_hash(),
        seed: sb.hash_seed(),
    };
    let (hash, _) = hasher.hash(name);
    let Some(top) = entries(&root, ROOT_ENTRIES, blocks) else {
        return Ok(Probe::Unusable);
    };
    // The path from the root: each level's entries, and the one taken.
    let depth = usize::from(levels).saturating_add(1);
    let at = position(&top, hash);
    let mut path = vec![(top, at)];
    while path.len() < depth {
        let Some(block) = leaf(&path) else {
            return Ok(Probe::Unusable);
        };
        let bytes = dir.raw_block(source, sb, u64::from(block), op).await?;
        let Some(below) = entries(&bytes, NODE_ENTRIES, blocks) else {
            return Ok(Probe::Unusable);
        };
        let at = position(&below, hash);
        path.push((below, at));
    }
    // Each leaf is read once at most, as hashes only grow.
    for _ in 0..blocks {
        let Some(block) = leaf(&path) else {
            return Ok(Probe::Unusable);
        };
        let found = dir.block(source, sb, u64::from(block), op).await?;
        if let Some(entry) = found.into_iter().find(|entry| entry.name == name) {
            return Ok(Probe::Found(entry));
        }
        // On to the next entry at the deepest level that has one, if its
        // block continues the hash, then down to its first leaf.
        loop {
            let Some((entries, at)) = path.last_mut() else {
                return Ok(Probe::Absent);
            };
            if at.saturating_add(1) < entries.len() {
                *at = at.saturating_add(1);
                break;
            }
            path.pop();
        }
        let next = path.last().and_then(|(entries, at)| entries.get(*at));
        if next.is_none_or(|entry| entry.hash & !1 != hash) {
            return Ok(Probe::Absent);
        }
        while path.len() < depth {
            let Some(block) = leaf(&path) else {
                return Ok(Probe::Unusable);
            };
            let bytes = dir.raw_block(source, sb, u64::from(block), op).await?;
            let Some(below) = entries(&bytes, NODE_ENTRIES, blocks) else {
                return Ok(Probe::Unusable);
            };
            path.push((below, 0));
        }
    }
    Ok(Probe::Unusable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Options, dir::DirEntry, volume::Volume};
    use filer_fs::read_exact;
    use filer_spec::{Sparse, block_on};

    const OP: OpKind = OpKind::Open;

    fn image(sparse: &[u8]) -> Vec<u8> {
        let sparse = Sparse::new(sparse);
        let len = usize::try_from(block_on(sparse.size()).unwrap()).unwrap();
        block_on(read_exact(&sparse, 0, len, OP)).unwrap()
    }

    fn open(image: Vec<u8>) -> Volume<Vec<u8>> {
        block_on(Volume::open(image, Options::new())).unwrap()
    }

    /// Returns the root's directory `name`, and where its first block is.
    fn dir(volume: &Volume<Vec<u8>>, name: &[u8]) -> (Directory, usize) {
        let root = block_on(volume.root(OP)).unwrap();
        let node = block_on(volume.lookup(&root, name, OP)).unwrap();
        let map = volume.map(&node, OP).unwrap();
        let run = block_on(map.map(&volume.source, &volume.sb, 0, 1, OP)).unwrap();
        let at = run.physical.unwrap() * u64::from(volume.sb.block_size());
        let dir = volume.directory(&node, OP).unwrap();
        (dir, usize::try_from(at).unwrap())
    }

    /// Every entry of `dir`, by a linear read.
    fn listed(volume: &Volume<Vec<u8>>, dir: &Directory) -> Vec<DirEntry> {
        (0..dir.blocks(&volume.sb))
            .flat_map(|i| block_on(dir.block(&volume.source, &volume.sb, i, OP)).unwrap())
            .filter(|entry| !entry.is_dot())
            .collect()
    }

    fn probe(volume: &Volume<Vec<u8>>, dir: &Directory, name: &[u8]) -> Probe {
        block_on(lookup(dir, &volume.source, &volume.sb, name, OP)).unwrap()
    }

    fn tea() -> Vec<u8> {
        image(include_bytes!("../tests/fixtures/ext2-htree-tea.sparse"))
    }

    fn legacy() -> Vec<u8> {
        image(include_bytes!("../tests/fixtures/ext2-htree-legacy.sparse"))
    }

    /// TEA over unsigned bytes, two levels deep; legacy over signed ones;
    /// and half-MD4: the index finds every entry a linear read lists, and
    /// nothing else.
    #[test]
    fn agrees_with_the_scan() {
        let ext2 = image(include_bytes!("../tests/fixtures/ext2-1k.sparse"));
        for (image, name, count) in [
            (tea(), &b"big"[..], 700),
            (legacy(), b"big", 700),
            (ext2, b"many", 100),
        ] {
            let volume = open(image);
            let (dir, _) = dir(&volume, name);
            let entries = listed(&volume, &dir);
            assert!(entries.len() >= count, "{}", entries.len());
            for entry in &entries {
                match probe(&volume, &dir, &entry.name) {
                    Probe::Found(found) => assert_eq!(&found, entry),
                    other => panic!("{other:?}"),
                }
            }
            for absent in [&b"absent"[..], b"0000-", b"0699-x", b"."] {
                assert!(matches!(probe(&volume, &dir, absent), Probe::Absent));
            }
        }
    }

    /// A block whose entry's hash has its lowest bit set continues the
    /// names of that hash from the block before.
    #[test]
    fn reads_on_into_blocks_continuing_a_hash() {
        let mut image = legacy();
        let volume = open(image.clone());
        let (dir, root) = dir(&volume, b"big");
        let raw = block_on(dir.raw_block(&volume.source, &volume.sb, 0, OP)).unwrap();
        let top = entries(&raw, ROOT_ENTRIES, dir.blocks(&volume.sb)).unwrap();
        // The first name of the third entry's block, which the second's
        // continues into once the third's hash is that name's.
        let block = u64::from(top[2].block);
        let name = block_on(dir.block(&volume.source, &volume.sb, block, OP)).unwrap()[0]
            .name
            .clone();
        let hasher = Hasher {
            version: Version::Legacy,
            unsigned: false,
            seed: volume.sb.hash_seed(),
        };
        let (hash, _) = hasher.hash(&name);
        assert!(hash > top[1].hash && hash < top[3].hash);
        let at = root + ROOT_ENTRIES + 2 * ENTRY;
        image[at..at + 4].copy_from_slice(&(hash | 1).to_le_bytes());
        let volume = open(image);
        let (dir, _) = self::dir(&volume, b"big");
        assert!(matches!(probe(&volume, &dir, &name), Probe::Found(_)));
    }

    /// Indexes of an unknown hash version, too deep, or naming blocks the
    /// directory lacks are not followed, and lookups read every block.
    #[test]
    fn falls_back_on_unusable_indexes() {
        for (at, value) in [(ROOT_INFO + 4, 7), (ROOT_INFO + 6, 3), (ROOT_INFO + 5, 9)] {
            let mut image = tea();
            let volume = open(image.clone());
            let (_, root) = dir(&volume, b"big");
            image[root + at] = value;
            let volume = open(image);
            let (dir, _) = self::dir(&volume, b"big");
            let entries = listed(&volume, &dir);
            let name = &entries[350].name;
            assert!(matches!(probe(&volume, &dir, name), Probe::Unusable));
            let found = block_on(dir.lookup(&volume.source, &volume.sb, name, OP)).unwrap();
            assert_eq!(found.as_ref(), Some(&entries[350]));
        }
        // A block past the directory's end.
        let mut image = tea();
        let volume = open(image.clone());
        let (_, root) = dir(&volume, b"big");
        let at = root + ROOT_ENTRIES + ENTRY + 4;
        image[at..at + 4].copy_from_slice(&0xFFFF_u32.to_le_bytes());
        let volume = open(image);
        let (dir, _) = self::dir(&volume, b"big");
        assert!(matches!(probe(&volume, &dir, b"x"), Probe::Unusable));
    }
}

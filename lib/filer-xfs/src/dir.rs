// This is free and unencumbered software released into the public domain.

//! Directories (version 2), read linearly.
//!
//! A small directory is short form, its entries packed in the inode. A
//! larger one keeps its entries in data blocks of the directory block size
//! at the start of its data fork, below 32 GiB: one block holding its hash
//! leaves too, or many indexed by leaf, node, and free-space blocks at 32
//! and 64 GiB. Reading the data blocks in order sees exactly the entries,
//! so the indexes are not needed to list or find them.
//!
//! An entry's directory offset is its position in the data blocks, and in
//! a short-form directory the tag it keeps for the same purpose; offsets
//! ascend through a directory, so they serve as cookies.

use crate::{
    XfsError, be,
    file_map::FileMap,
    inode::{Format, Inode},
    superblock::Superblock,
};
use alloc::vec::Vec;
use filer_api::{Error, FileKind, OpKind, ReadAt};

/// The magic numbers of a single-block directory's block, `XD2B`, and of
/// a data block, `XD2D`.
const BLOCK_MAGIC: u32 = 0x5844_3242;
const DATA_MAGIC: u32 = 0x5844_3244;

/// The size of a data block's header: the magic, and three best free
/// regions.
const DATA_HEADER: usize = 16;

/// The size of a single-block directory's tail (the leaf count and the
/// stale count), and of each leaf entry before it.
const BLOCK_TAIL: usize = 8;
const LEAF_ENTRY: usize = 8;

/// The tag that starts an unused region instead of an inode number.
const FREE_TAG: u16 = 0xFFFF;

/// Data blocks lie below this directory offset; leaves start there.
const LEAF_OFFSET: u64 = 32 << 30;

/// An entry of a directory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DirEntry {
    /// The entry's directory offset.
    pub(crate) offset: u64,
    pub(crate) inode: u64,
    pub(crate) name: Vec<u8>,
    /// The kind the entry records, if the volume records kinds.
    pub(crate) kind: Option<FileKind>,
}

impl DirEntry {
    /// Returns `true` for `.` and `..`, which listings skip.
    pub(crate) fn is_dot(&self) -> bool {
        matches!(self.name.as_slice(), b"." | b"..")
    }
}

/// Decodes an entry's file type, recorded with `FTYPE`.
fn kind(byte: u8) -> Result<Option<FileKind>, ()> {
    Ok(Some(match byte {
        0 => return Ok(None),
        1 => FileKind::File,
        2 => FileKind::Dir,
        3 => FileKind::CharDevice,
        4 => FileKind::BlockDevice,
        5 => FileKind::Fifo,
        6 => FileKind::Socket,
        7 => FileKind::Symlink,
        _ => return Err(()),
    }))
}

/// Checks that `name` is a name: not empty, without `/` or NUL.
fn is_name(name: &[u8]) -> bool {
    !name.is_empty() && !name.iter().any(|&b| b == b'/' || b == 0)
}

/// Parses short-form directory `dir`'s entries from its data fork: a count
/// of entries, a count of those needing 64-bit inode numbers (if any, all
/// numbers are 64-bit), and the parent's number; then each entry's name
/// length, offset tag, name, type (with `ftype`), and inode number.
pub(crate) fn parse_short(bytes: &[u8], ftype: bool, dir: u64) -> Result<Vec<DirEntry>, XfsError> {
    let bad = |offset| XfsError::BadDirEntry { dir, offset };
    let count = bytes.first().copied().ok_or(bad(0))?;
    let wide = bytes.get(1).copied().ok_or(bad(0))? != 0;
    let number = |at: usize| {
        if wide {
            be::u64_at(bytes, at)
        } else {
            be::u32_at(bytes, at).map(u64::from)
        }
    };
    let width: usize = if wide { 8 } else { 4 };
    number(2).filter(|&parent| parent != 0).ok_or(bad(0))?;
    let mut at = 2usize.saturating_add(width);
    let mut entries = Vec::with_capacity(usize::from(count));
    for _ in 0..count {
        let start = u64::try_from(at).unwrap_or(u64::MAX);
        let name_len = usize::from(bytes.get(at).copied().ok_or(bad(start))?);
        let offset = u64::from(be::u16_at(bytes, at.saturating_add(1)).ok_or(bad(start))?);
        let name_at = at.saturating_add(3);
        let name = bytes
            .get(name_at..name_at.saturating_add(name_len))
            .filter(|name| is_name(name))
            .ok_or(bad(offset))?;
        let mut next = name_at.saturating_add(name_len);
        let kind = if ftype {
            let byte = bytes.get(next).copied().ok_or(bad(offset))?;
            next = next.saturating_add(1);
            kind(byte).map_err(|()| bad(offset))?
        } else {
            None
        };
        let inode = number(next).ok_or(bad(offset))?;
        if entries
            .last()
            .is_some_and(|last: &DirEntry| offset <= last.offset)
            || inode == 0
        {
            return Err(bad(offset));
        }
        entries.push(DirEntry {
            offset,
            inode,
            name: name.to_vec(),
            kind,
        });
        at = next.saturating_add(width);
    }
    if at != bytes.len() {
        return Err(bad(u64::try_from(at).unwrap_or(u64::MAX)));
    }
    Ok(entries)
}

/// Parses the entries of data block `block` of directory `dir`, which is
/// at directory offset `base`. In a single-block directory, the hash
/// leaves and tail at the block's end are not entries.
pub(crate) fn parse_block(
    block: &[u8],
    base: u64,
    single: bool,
    ftype: bool,
    dir: u64,
) -> Result<Vec<DirEntry>, XfsError> {
    let bad_block = XfsError::BadDirBlock { dir, offset: base };
    let magic = be::u32_at(block, 0).ok_or(bad_block)?;
    let end = match magic {
        DATA_MAGIC if !single => block.len(),
        BLOCK_MAGIC if single => {
            let tail = block.len().checked_sub(BLOCK_TAIL).ok_or(bad_block)?;
            let count = be::u32_at(block, tail).ok_or(bad_block)?;
            let leaves = usize::try_from(count)
                .ok()
                .and_then(|count| count.checked_mul(LEAF_ENTRY))
                .ok_or(bad_block)?;
            tail.checked_sub(leaves)
                .filter(|&end| end >= DATA_HEADER)
                .ok_or(bad_block)?
        },
        _ => return Err(bad_block),
    };
    let mut entries = Vec::new();
    let mut at = DATA_HEADER;
    while at < end {
        let offset = u64::try_from(at)
            .ok()
            .and_then(|at| base.checked_add(at))
            .unwrap_or(u64::MAX);
        let bad = XfsError::BadDirEntry { dir, offset };
        let tag_of = |len: usize| {
            let tag_at = at.checked_add(len)?.checked_sub(2)?;
            be::u16_at(block, tag_at).filter(|&tag| usize::from(tag) == at)
        };
        if be::u16_at(block, at).ok_or(bad)? == FREE_TAG {
            // An unused region: its length, and its tag at its end.
            let len = usize::from(be::u16_at(block, at.saturating_add(2)).ok_or(bad)?);
            if len == 0 || len % 8 != 0 || at.saturating_add(len) > end || tag_of(len).is_none() {
                return Err(bad);
            }
            at = at.saturating_add(len);
            continue;
        }
        // An entry: inode number, name length, name, type, and tag,
        // padded to 8 bytes.
        let inode = be::u64_at(block, at).ok_or(bad)?;
        let name_len = usize::from(block.get(at.saturating_add(8)).copied().ok_or(bad)?);
        let name_at = at.saturating_add(9);
        let name = block
            .get(name_at..name_at.saturating_add(name_len))
            .filter(|name| is_name(name))
            .ok_or(bad)?;
        let kind = if ftype {
            let byte = block
                .get(name_at.saturating_add(name_len))
                .copied()
                .ok_or(bad)?;
            kind(byte).map_err(|()| bad)?
        } else {
            None
        };
        let len = 11usize
            .saturating_add(name_len)
            .saturating_add(usize::from(ftype))
            .next_multiple_of(8);
        if at.saturating_add(len) > end || tag_of(len).is_none() || inode == 0 {
            return Err(bad);
        }
        entries.push(DirEntry {
            offset,
            inode,
            name: name.to_vec(),
            kind,
        });
        at = at.saturating_add(len);
    }
    if at != end {
        return Err(bad_block);
    }
    Ok(entries)
}

/// A directory's contents.
#[derive(Clone, Debug)]
pub(crate) enum Directory {
    /// Short form: the entries, parsed from the inode.
    Short { dir: u64, entries: Vec<DirEntry> },
    /// In data blocks: `size` bytes of them, mapped by `map`.
    Blocks { dir: u64, map: FileMap, size: u64 },
}

impl Directory {
    /// Returns directory `inode`'s contents. Data blocks fill whole
    /// directory blocks below the leaves, and no more than the volume
    /// holds, which bounds a scan.
    pub(crate) fn new(inode: &Inode, sb: &Superblock) -> Result<Self, XfsError> {
        let dir = inode.number();
        let size = inode.size();
        if inode.data().format == Format::Local {
            let entries = parse_short(&inode.data().bytes, sb.has_ftype(), dir)?;
            return Ok(Self::Short { dir, entries });
        }
        let block = u64::from(sb.dir_block_size());
        if size == 0 || !size.is_multiple_of(block) || size > LEAF_OFFSET || size > sb.volume_len()
        {
            return Err(XfsError::BadDirectorySize { dir, size });
        }
        let map = FileMap::new(dir, inode.data(), sb)?;
        Ok(Self::Blocks { dir, map, size })
    }

    /// Returns the directory's inode number.
    pub(crate) const fn inode(&self) -> u64 {
        match self {
            Self::Short { dir, .. } | Self::Blocks { dir, .. } => *dir,
        }
    }

    /// Returns the number of directory blocks, one for a short-form
    /// directory.
    pub(crate) fn blocks(&self, sb: &Superblock) -> u64 {
        match self {
            Self::Short { .. } => 1,
            Self::Blocks { size, .. } => size
                .checked_div(u64::from(sb.dir_block_size()))
                .unwrap_or(0),
        }
    }

    /// Returns the index of the directory block holding directory offset
    /// `offset`.
    pub(crate) fn block_of(&self, sb: &Superblock, offset: u64) -> u64 {
        match self {
            Self::Short { .. } => 0,
            Self::Blocks { .. } => offset
                .checked_div(u64::from(sb.dir_block_size()))
                .unwrap_or(0),
        }
    }

    /// Reads and parses directory block `index`: none for a hole, which
    /// only freed data blocks leave.
    pub(crate) async fn block<S: ReadAt + ?Sized>(
        &self,
        source: &S,
        sb: &Superblock,
        index: u64,
        op: OpKind,
    ) -> Result<Vec<DirEntry>, Error> {
        let (dir, map, size) = match self {
            Self::Short { entries, .. } => return Ok(entries.clone()),
            Self::Blocks { dir, map, size } => (*dir, map, *size),
        };
        let fail = |error: XfsError| error.into_error(op);
        let bad = || fail(XfsError::BadDirectorySize { dir, size });
        let block = sb.dir_block_size();
        let base = index.checked_mul(u64::from(block)).ok_or_else(bad)?;
        let first = base
            .checked_div(u64::from(sb.block_size()))
            .ok_or_else(bad)?;
        if map.map(source, sb, first, 1, op).await?.physical.is_none() {
            return Ok(Vec::new());
        }
        let len = usize::try_from(block).map_err(|_| bad())?;
        let bytes = map.read(source, sb, size, base, len, op).await?;
        if bytes.len() != len {
            return Err(bad());
        }
        let single = size == u64::from(block);
        parse_block(&bytes, base, single, sb.has_ftype(), dir).map_err(fail)
    }

    /// Finds the entry `name`, matching bytes exactly.
    pub(crate) async fn lookup<S: ReadAt + ?Sized>(
        &self,
        source: &S,
        sb: &Superblock,
        name: &[u8],
        op: OpKind,
    ) -> Result<Option<DirEntry>, Error> {
        for index in 0..self.blocks(sb) {
            let entries = self.block(source, sb, index, op).await?;
            if let Some(entry) = entries.into_iter().find(|entry| entry.name == name) {
                return Ok(Some(entry));
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::{
        file_map::tests::record,
        inode::tests::Raw,
        superblock::{Version, tests::Fields},
    };
    use alloc::vec;
    use filer_api::ErrorKind;
    use filer_spec::block_on;

    /// Lays out a short-form directory of `entries` (offset tag, name,
    /// type, inode), with 64-bit numbers if `wide`.
    pub(crate) fn short(entries: &[(u16, &[u8], u8, u64)], wide: bool, ftype: bool) -> Vec<u8> {
        let mut b = vec![u8::try_from(entries.len()).unwrap(), u8::from(wide)];
        let number = |b: &mut Vec<u8>, n: u64| {
            if wide {
                b.extend_from_slice(&n.to_be_bytes());
            } else {
                b.extend_from_slice(&u32::try_from(n).unwrap().to_be_bytes());
            }
        };
        number(&mut b, 128);
        for (offset, name, kind, inode) in entries {
            b.push(u8::try_from(name.len()).unwrap());
            b.extend_from_slice(&offset.to_be_bytes());
            b.extend_from_slice(name);
            if ftype {
                b.push(*kind);
            }
            number(&mut b, *inode);
        }
        b
    }

    /// Lays out a data block of `size` bytes holding `entries` (inode,
    /// name, type) from the header on, the rest unused; as a single-block
    /// directory's, its tail counts `leaves` hash leaves.
    pub(crate) fn data_block(
        size: usize,
        entries: &[(u64, &[u8], u8)],
        ftype: bool,
        leaves: Option<u32>,
    ) -> Vec<u8> {
        let mut b = vec![0; size];
        let magic = if leaves.is_some() {
            BLOCK_MAGIC
        } else {
            DATA_MAGIC
        };
        b[..4].copy_from_slice(&magic.to_be_bytes());
        let end = match leaves {
            Some(count) => {
                b[size - 8..size - 4].copy_from_slice(&count.to_be_bytes());
                size - 8 - 8 * usize::try_from(count).unwrap()
            },
            None => size,
        };
        let mut at = DATA_HEADER;
        for (inode, name, kind) in entries {
            let len = (11 + name.len() + usize::from(ftype)).next_multiple_of(8);
            b[at..at + 8].copy_from_slice(&inode.to_be_bytes());
            b[at + 8] = u8::try_from(name.len()).unwrap();
            b[at + 9..at + 9 + name.len()].copy_from_slice(name);
            if ftype {
                b[at + 9 + name.len()] = *kind;
            }
            b[at + len - 2..at + len].copy_from_slice(&u16::try_from(at).unwrap().to_be_bytes());
            at += len;
        }
        if at < end {
            let len = end - at;
            b[at..at + 2].copy_from_slice(&FREE_TAG.to_be_bytes());
            b[at + 2..at + 4].copy_from_slice(&u16::try_from(len).unwrap().to_be_bytes());
            b[end - 2..end].copy_from_slice(&u16::try_from(at).unwrap().to_be_bytes());
        }
        b
    }

    fn entry(offset: u64, inode: u64, name: &[u8], kind: Option<FileKind>) -> DirEntry {
        DirEntry {
            offset,
            inode,
            name: name.to_vec(),
            kind,
        }
    }

    #[test]
    fn parses_short_form_directories() {
        let bytes = short(&[(0x30, b"a", 1, 131), (0x40, b"sub", 2, 132)], false, true);
        assert_eq!(
            parse_short(&bytes, true, 128).unwrap(),
            [
                entry(0x30, 131, b"a", Some(FileKind::File)),
                entry(0x40, 132, b"sub", Some(FileKind::Dir)),
            ]
        );
        let bytes = short(&[(0x30, b"far", 0, 1 << 40)], true, false);
        assert_eq!(
            parse_short(&bytes, false, 128).unwrap(),
            [entry(0x30, 1 << 40, b"far", None)]
        );
        assert_eq!(parse_short(&short(&[], false, true), true, 128), Ok(vec![]));
    }

    #[test]
    fn rejects_bad_short_form_directories() {
        let check = |bytes: &[u8]| {
            assert!(matches!(
                parse_short(bytes, true, 128),
                Err(XfsError::BadDirEntry { dir: 128, .. })
            ));
        };
        let good = short(&[(0x30, b"a", 1, 131), (0x40, b"b", 1, 132)], false, true);
        check(&good[..good.len() - 1]);
        check(&[good.as_slice(), &[0]].concat());
        check(&short(
            &[(0x30, b"a", 1, 131), (0x30, b"b", 1, 132)],
            false,
            true,
        ));
        check(&short(&[(0x30, b"a/b", 1, 131)], false, true));
        check(&short(&[(0x30, b"", 1, 131)], false, true));
        check(&short(&[(0x30, b"a", 9, 131)], false, true));
        check(&short(&[(0x30, b"a", 1, 0)], false, true));
        check(&[0, 0, 0, 0, 0, 0]);
        check(&[]);
    }

    #[test]
    fn parses_data_blocks() {
        let entries: [(u64, &[u8], u8); 3] = [(128, b".", 2), (64, b"..", 2), (131, b"file", 1)];
        let block = data_block(4096, &entries, true, None);
        let parsed = parse_block(&block, 8192, false, true, 128).unwrap();
        assert_eq!(
            parsed,
            [
                entry(8192 + 16, 128, b".", Some(FileKind::Dir)),
                entry(8192 + 32, 64, b"..", Some(FileKind::Dir)),
                entry(8192 + 48, 131, b"file", Some(FileKind::File)),
            ]
        );
        assert!(parsed[0].is_dot() && parsed[1].is_dot() && !parsed[2].is_dot());

        // A single-block directory stops at its leaves.
        let block = data_block(4096, &entries, false, Some(3));
        let parsed = parse_block(&block, 0, true, false, 128).unwrap();
        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[2], entry(48, 131, b"file", None));
    }

    #[test]
    fn rejects_bad_data_blocks() {
        let entries: [(u64, &[u8], u8); 1] = [(131, b"file", 1)];
        let good = data_block(4096, &entries, true, None);
        let check = |block: &[u8], single: bool| {
            assert!(parse_block(block, 0, single, true, 128).is_err());
        };
        // The wrong magic for the directory's form.
        check(&good, true);
        check(&data_block(4096, &entries, true, Some(0)), false);
        // A tag that is not the entry's offset.
        let mut bad = good.clone();
        bad[16 + 14] = 0x99;
        check(&bad, false);
        // A name with NUL.
        let mut bad = good.clone();
        bad[16 + 9] = 0;
        check(&bad, false);
        // An unused region of a bad length.
        let mut bad = good.clone();
        bad[32 + 2..32 + 4].copy_from_slice(&7u16.to_be_bytes());
        check(&bad, false);
        // Too many leaves for the block.
        let mut bad = data_block(4096, &entries, true, Some(0));
        bad[4096 - 8..4096 - 4].copy_from_slice(&600u32.to_be_bytes());
        check(&bad, true);
    }

    /// A directory inode of `size` bytes whose data blocks are mapped by
    /// `records`.
    fn blocks_dir(size: u64, records: &[[u8; 16]]) -> Inode {
        let mut raw = Raw::file(size);
        raw.mode = 0o40_755;
        raw.extents = u32::try_from(records.len()).unwrap();
        raw.literal = records.concat();
        Inode::parse(&raw.bytes(256), 128, Version::V4).unwrap()
    }

    #[test]
    fn reads_and_looks_up_directories() {
        let sb = Superblock::parse(&Fields::new().bytes()).unwrap();
        let mut image = vec![0; 4096 * 1024];
        let first: [(u64, &[u8], u8); 3] = [(128, b".", 2), (128, b"..", 2), (131, b"a", 1)];
        image[10 * 4096..11 * 4096].copy_from_slice(&data_block(4096, &first, true, None));
        let third: [(u64, &[u8], u8); 1] = [(132, b"c", 7)];
        image[20 * 4096..21 * 4096].copy_from_slice(&data_block(4096, &third, true, None));
        // Data blocks 0 and 2, with a hole between, and a leaf at 32 GiB.
        let leaf = LEAF_OFFSET / 4096;
        let inode = blocks_dir(
            3 * 4096,
            &[
                record(0, 10, 1, false),
                record(2, 20, 1, false),
                record(leaf, 30, 1, false),
            ],
        );
        let dir = Directory::new(&inode, &sb).unwrap();
        assert_eq!(dir.blocks(&sb), 3);
        assert_eq!(dir.block_of(&sb, 8200), 2);
        let block = |index| block_on(dir.block(&image, &sb, index, OpKind::ReadDir)).unwrap();
        assert_eq!(block(0).len(), 3);
        assert!(block(1).is_empty());
        assert_eq!(
            block(2),
            [entry(8192 + 16, 132, b"c", Some(FileKind::Symlink))]
        );
        let lookup = |name| block_on(dir.lookup(&image, &sb, name, OpKind::Open)).unwrap();
        assert_eq!(lookup(b"c").unwrap().inode, 132);
        assert_eq!(lookup(b"a").unwrap().inode, 131);
        assert_eq!(lookup(b"b"), None);

        // A short-form directory.
        let mut raw = Raw::local(0o40_755, &short(&[(0x30, b"x", 1, 133)], false, true));
        raw.links = 2;
        let inode = Inode::parse(&raw.bytes(256), 128, Version::V4).unwrap();
        let dir = Directory::new(&inode, &sb).unwrap();
        assert_eq!(dir.blocks(&sb), 1);
        let found = block_on(dir.lookup(&image, &sb, b"x", OpKind::Open)).unwrap();
        assert_eq!(found.unwrap().inode, 133);
    }

    #[test]
    fn rejects_bad_directory_sizes() {
        let sb = Superblock::parse(&Fields::new().bytes()).unwrap();
        for size in [0, 4095, LEAF_OFFSET + 4096] {
            let inode = blocks_dir(size, &[record(0, 10, 1, false)]);
            let error = Directory::new(&inode, &sb).unwrap_err();
            assert_eq!(error, XfsError::BadDirectorySize { dir: 128, size });
            assert_eq!(error.kind(), ErrorKind::Corrupt);
        }
    }
}

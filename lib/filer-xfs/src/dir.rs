// This is free and unencumbered software released into the public domain.

//! Directories (version 2).
//!
//! A small directory is short form, its entries packed in the inode. A
//! larger one keeps its entries in data blocks of the directory block size
//! at the start of its data fork, below 32 GiB: one block holding its hash
//! leaves too, or many indexed by leaf, node, and free-space blocks at 32
//! and 64 GiB. Reading the data blocks in order sees exactly the entries,
//! so listings read them so. Lookups in a directory of many blocks follow
//! the name's hash down the nodes to the leaves, whose entries' addresses,
//! in 8-byte units, are the offsets of the data entries; a directory of
//! one block, or whose index is inconsistent, is scanned instead.
//!
//! An entry's directory offset is its position in the data blocks, and in
//! a short-form directory the tag it keeps for the same purpose; offsets
//! ascend through a directory, so they serve as cookies.
//!
//! With version 5, blocks have other magic numbers, and longer headers,
//! self-describing (`crc::DIR_DATA` and `crc::DA`), which are checked as
//! each is read.

use crate::{
    XfsError, be, crc, da,
    file_map::FileMap,
    hash,
    inode::{Format, Inode},
    superblock::Superblock,
};
use alloc::{collections::BTreeSet, vec::Vec};
use filer_api::{Error, ErrorKind, FileKind, OpKind, ReadAt};

/// The size of a single-block directory's tail (the leaf count and the
/// stale count), and of each leaf entry before it.
const BLOCK_TAIL: usize = 8;
const LEAF_ENTRY: usize = 8;

/// The tag that starts an unused region instead of an inode number.
const FREE_TAG: u16 = 0xFFFF;

/// Data blocks lie below this directory offset; leaves start there, and
/// free-space blocks at twice it.
const LEAF_OFFSET: u64 = 32 << 30;

/// What a volume's directory blocks look like, by its version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Shape {
    /// The magic numbers of a single-block directory's block, and of a
    /// data block.
    block_magic: u32,
    data_magic: u32,
    /// The size of a data block's header, which ends with its three best
    /// free regions.
    data_header: usize,
    /// The magic numbers of a leaf block alone, `leaf1`, and of one of
    /// many below nodes, `leafn`.
    leaf1_magic: u16,
    leafn_magic: u16,
    /// The size of a leaf block's header, and where its count is: after
    /// its siblings and magic, and its self-description.
    leaf_header: usize,
    leaf_count: usize,
}

/// Version 4's: `XD2B`, `XD2D`, and leaves of 0xD2F1 and 0xD2FF.
pub(crate) const V4: Shape = Shape {
    block_magic: 0x5844_3242,
    data_magic: 0x5844_3244,
    data_header: 16,
    leaf1_magic: 0xD2F1,
    leafn_magic: 0xD2FF,
    leaf_header: 16,
    leaf_count: 12,
};

/// Version 5's: `XDB3`, `XDD3`, and leaves of 0x3DF1 and 0x3DFF.
pub(crate) const V5: Shape = Shape {
    block_magic: 0x5844_4233,
    data_magic: 0x5844_4433,
    data_header: 64,
    leaf1_magic: 0x3DF1,
    leafn_magic: 0x3DFF,
    leaf_header: 64,
    leaf_count: 56,
};

/// Returns the shape of directory blocks on volume `sb`.
fn shape(sb: &Superblock) -> Shape {
    if sb.is_v5() { V5 } else { V4 }
}

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

/// Parses the hash leaves of a leaf block of `shape`: hashes, ascending,
/// and data entries' addresses, 0 for a stale leaf. A `leaf1` block ends
/// with the best free space of each data block, and their count.
fn parse_leaves(block: &[u8], shape: Shape) -> Option<Vec<(u32, u32)>> {
    let magic = da::magic(block)?;
    if magic != shape.leaf1_magic && magic != shape.leafn_magic {
        return None;
    }
    let count = usize::from(be::u16_at(block, shape.leaf_count)?);
    let mut end = block.len();
    if magic == shape.leaf1_magic {
        let tail = end.checked_sub(4)?;
        let bests = usize::try_from(be::u32_at(block, tail)?).ok()?;
        end = tail.checked_sub(bests.checked_mul(2)?)?;
    }
    if count
        .checked_mul(LEAF_ENTRY)?
        .checked_add(shape.leaf_header)?
        > end
    {
        return None;
    }
    let mut leaves = Vec::with_capacity(count);
    for i in 0..count {
        let at = shape.leaf_header.checked_add(i.checked_mul(LEAF_ENTRY)?)?;
        let hash = be::u32_at(block, at)?;
        if leaves.last().is_some_and(|&(last, _)| hash < last) {
            return None;
        }
        leaves.push((hash, be::u32_at(block, at.checked_add(4)?)?));
    }
    Some(leaves)
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

/// Parses the entries of data block `block`, of `shape`, of directory
/// `dir`, which is at directory offset `base`. In a single-block
/// directory, the hash leaves and tail at the block's end are not entries.
pub(crate) fn parse_block(
    block: &[u8],
    shape: Shape,
    base: u64,
    single: bool,
    ftype: bool,
    dir: u64,
) -> Result<Vec<DirEntry>, XfsError> {
    let bad_block = XfsError::BadDirBlock { dir, offset: base };
    let magic = be::u32_at(block, 0).ok_or(bad_block)?;
    let end = match magic {
        magic if magic == shape.data_magic && !single => block.len(),
        magic if magic == shape.block_magic && single => {
            let tail = block.len().checked_sub(BLOCK_TAIL).ok_or(bad_block)?;
            let count = be::u32_at(block, tail).ok_or(bad_block)?;
            let leaves = usize::try_from(count)
                .ok()
                .and_then(|count| count.checked_mul(LEAF_ENTRY))
                .ok_or(bad_block)?;
            tail.checked_sub(leaves)
                .filter(|&end| end >= shape.data_header)
                .ok_or(bad_block)?
        },
        _ => return Err(bad_block),
    };
    let mut entries = Vec::new();
    let mut at = shape.data_header;
    while at < end {
        let (entry, len) = parse_entry(block, at, end, base, ftype, dir)?;
        entries.extend(entry);
        at = at.saturating_add(len);
    }
    if at != end {
        return Err(bad_block);
    }
    Ok(entries)
}

/// Parses what is at `at` in data block `block` of directory `dir`, which
/// is at directory offset `base` and holds entries up to `end`: an entry,
/// or an unused region, which is `None`; and its length. An entry is an
/// inode number, name length, name, type (with `ftype`), and tag, padded
/// to 8 bytes; an unused region `FREE_TAG`, its length, and a tag at its
/// end. Each tag is the offset of its start in the block.
fn parse_entry(
    block: &[u8],
    at: usize,
    end: usize,
    base: u64,
    ftype: bool,
    dir: u64,
) -> Result<(Option<DirEntry>, usize), XfsError> {
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
        let len = usize::from(be::u16_at(block, at.saturating_add(2)).ok_or(bad)?);
        if len == 0 || len % 8 != 0 || at.saturating_add(len) > end || tag_of(len).is_none() {
            return Err(bad);
        }
        return Ok((None, len));
    }
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
    let entry = DirEntry {
        offset,
        inode,
        name: name.to_vec(),
        kind,
    };
    Ok((Some(entry), len))
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

    /// Reads the directory block at fork block `first`, `limit` bytes into
    /// the fork at most: none for a hole. With version 5, its header must
    /// be of `layout`, and name it.
    async fn read<S: ReadAt + ?Sized>(
        &self,
        source: &S,
        sb: &Superblock,
        first: u64,
        limit: u64,
        layout: crc::Layout,
        op: OpKind,
    ) -> Result<Option<Vec<u8>>, Error> {
        let Self::Blocks { dir, map, .. } = self else {
            return Ok(None);
        };
        let fail = |error: XfsError| error.into_error(op);
        let bad = || {
            fail(XfsError::BadDirBlock {
                dir: *dir,
                offset: first.saturating_mul(u64::from(sb.block_size())),
            })
        };
        let Some(physical) = map.map(source, sb, first, 1, op).await?.physical else {
            return Ok(None);
        };
        let offset = sb.block_offset(physical).ok_or_else(bad)?;
        let base = first
            .checked_mul(u64::from(sb.block_size()))
            .ok_or_else(bad)?;
        let len = usize::try_from(sb.dir_block_size()).map_err(|_| bad())?;
        let bytes = map.read(source, sb, limit, base, len, op).await?;
        if bytes.len() != len {
            return Err(bad());
        }
        if sb.is_v5() {
            crc::check(&bytes, layout, sb, offset, *dir).map_err(fail)?;
        }
        Ok(Some(bytes))
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
        let (dir, size) = match self {
            Self::Short { entries, .. } => return Ok(entries.clone()),
            Self::Blocks { dir, size, .. } => (*dir, *size),
        };
        let fail = |error: XfsError| error.into_error(op);
        let bad = || fail(XfsError::BadDirectorySize { dir, size });
        let block = sb.dir_block_size();
        let base = index.checked_mul(u64::from(block)).ok_or_else(bad)?;
        let first = base
            .checked_div(u64::from(sb.block_size()))
            .ok_or_else(bad)?;
        let read = self.read(source, sb, first, size, crc::DIR_DATA, op);
        let Some(bytes) = read.await? else {
            return Ok(Vec::new());
        };
        let single = size == u64::from(block);
        parse_block(&bytes, shape(sb), base, single, sb.has_ftype(), dir).map_err(fail)
    }

    /// Finds the entry `name`: matching bytes exactly, or, on an
    /// `ASCII_CI` volume, folded to lower case.
    pub(crate) async fn lookup<S: ReadAt + ?Sized>(
        &self,
        source: &S,
        sb: &Superblock,
        name: &[u8],
        op: OpKind,
    ) -> Result<Option<DirEntry>, Error> {
        if self.blocks(sb) > 1 {
            match self.find(source, sb, name, op).await {
                Err(error) if error.kind() == ErrorKind::Corrupt => {},
                found => return found,
            }
        }
        for index in 0..self.blocks(sb) {
            let entries = self.block(source, sb, index, op).await?;
            if let Some(entry) = entries
                .into_iter()
                .find(|entry| matches(sb, &entry.name, name))
            {
                return Ok(Some(entry));
            }
        }
        Ok(None)
    }

    /// Finds the entry `name` by its hash, in a directory of many blocks,
    /// failing if the index is inconsistent.
    async fn find<S: ReadAt + ?Sized>(
        &self,
        source: &S,
        sb: &Superblock,
        name: &[u8],
        op: OpKind,
    ) -> Result<Option<DirEntry>, Error> {
        let Self::Blocks { dir, .. } = self else {
            return Ok(None);
        };
        let fail = || {
            XfsError::BadDirBlock {
                dir: *dir,
                offset: LEAF_OFFSET,
            }
            .into_error(op)
        };
        let hash = if sb.ascii_ci() {
            hash::hash_folded(name)
        } else {
            hash::hash(name)
        };
        // Down the nodes, if any, to the leaf where entries of the hash
        // start; each block's level one below its parent's.
        let first = LEAF_OFFSET
            .checked_div(u64::from(sb.block_size()))
            .ok_or_else(fail)?;
        let mut at = first;
        let mut level = None;
        let shape = shape(sb);
        let mut block = loop {
            let block = self.index_block(source, sb, at, op).await?;
            match da::magic(&block) {
                Some(magic) if magic == da::node_magic(sb.is_v5()) => {
                    let node = da::Node::parse(&block, sb.is_v5()).ok_or_else(fail)?;
                    if level.is_some_and(|level| level != node.level) {
                        return Err(fail());
                    }
                    let Some(child) = node.child(hash) else {
                        return Ok(None);
                    };
                    level = Some(node.level.saturating_sub(1));
                    at = u64::from(child);
                },
                Some(magic) if magic == shape.leaf1_magic && level.is_none() => break block,
                Some(magic) if magic == shape.leafn_magic && level == Some(0) => break block,
                _ => return Err(fail()),
            }
        };
        // Along the leaves, while entries of the hash may run on.
        let mut seen = BTreeSet::new();
        loop {
            if !seen.insert(at) {
                return Err(fail());
            }
            let leaves = parse_leaves(&block, shape).ok_or_else(fail)?;
            let start = leaves.partition_point(|&(found, _)| found < hash);
            for &(found, address) in leaves.get(start..).unwrap_or_default() {
                if found != hash {
                    return Ok(None);
                }
                if address == 0 {
                    continue;
                }
                let entry = self.entry_at(source, sb, address, op).await?;
                if matches(sb, &entry.name, name) {
                    return Ok(Some(entry));
                }
            }
            match da::forw(&block).ok_or_else(fail)? {
                0 => return Ok(None),
                next => at = u64::from(next),
            }
            block = self.index_block(source, sb, at, op).await?;
            if da::magic(&block) != Some(shape.leafn_magic) {
                return Err(fail());
            }
        }
    }

    /// Reads the directory block of leaves or nodes at fork block `at`,
    /// which must be between the leaves' offset and the free-space
    /// blocks'.
    async fn index_block<S: ReadAt + ?Sized>(
        &self,
        source: &S,
        sb: &Superblock,
        at: u64,
        op: OpKind,
    ) -> Result<Vec<u8>, Error> {
        let fail = || {
            XfsError::BadDirBlock {
                dir: self.inode(),
                offset: LEAF_OFFSET,
            }
            .into_error(op)
        };
        let end = at
            .checked_mul(u64::from(sb.block_size()))
            .filter(|&off| (LEAF_OFFSET..LEAF_OFFSET.saturating_mul(2)).contains(&off))
            .and_then(|off| off.checked_add(u64::from(sb.dir_block_size())))
            .ok_or_else(fail)?;
        self.read(source, sb, at, end, crc::DA, op)
            .await?
            .ok_or_else(fail)
    }

    /// Returns the data entry at `address`, in 8-byte units, which must be
    /// one, in a directory of many blocks.
    async fn entry_at<S: ReadAt + ?Sized>(
        &self,
        source: &S,
        sb: &Superblock,
        address: u32,
        op: OpKind,
    ) -> Result<DirEntry, Error> {
        let offset = u64::from(address).saturating_mul(8);
        let dir = self.inode();
        let fail = || XfsError::BadDirEntry { dir, offset }.into_error(op);
        let Self::Blocks { size, .. } = self else {
            return Err(fail());
        };
        let shape = shape(sb);
        let block = u64::from(sb.dir_block_size());
        let base = offset
            .checked_div(block)
            .and_then(|index| index.checked_mul(block))
            .filter(|&base| base < *size)
            .ok_or_else(fail)?;
        let first = base
            .checked_div(u64::from(sb.block_size()))
            .ok_or_else(fail)?;
        let bytes = self
            .read(source, sb, first, *size, crc::DIR_DATA, op)
            .await?
            .ok_or_else(fail)?;
        if be::u32_at(&bytes, 0) != Some(shape.data_magic) {
            return Err(fail());
        }
        let at = usize::try_from(offset.saturating_sub(base))
            .ok()
            .filter(|&at| at >= shape.data_header)
            .ok_or_else(fail)?;
        let len = bytes.len();
        match parse_entry(&bytes, at, len, base, sb.has_ftype(), dir) {
            Ok((Some(entry), _)) => Ok(entry),
            _ => Err(fail()),
        }
    }
}

/// Returns whether entry name `found` matches `name`: exactly, or, on an
/// `ASCII_CI` volume, folded to lower case.
fn matches(sb: &Superblock, found: &[u8], name: &[u8]) -> bool {
    if sb.ascii_ci() {
        hash::eq_folded(found, name)
    } else {
        found == name
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
            V4.block_magic
        } else {
            V4.data_magic
        };
        b[..4].copy_from_slice(&magic.to_be_bytes());
        let end = match leaves {
            Some(count) => {
                b[size - 8..size - 4].copy_from_slice(&count.to_be_bytes());
                size - 8 - 8 * usize::try_from(count).unwrap()
            },
            None => size,
        };
        let mut at = V4.data_header;
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
        let parsed = parse_block(&block, V4, 8192, false, true, 128).unwrap();
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
        let parsed = parse_block(&block, V4, 0, true, false, 128).unwrap();
        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[2], entry(48, 131, b"file", None));
    }

    #[test]
    fn rejects_bad_data_blocks() {
        let entries: [(u64, &[u8], u8); 1] = [(131, b"file", 1)];
        let good = data_block(4096, &entries, true, None);
        let check = |block: &[u8], single: bool| {
            assert!(parse_block(block, V4, 0, single, true, 128).is_err());
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

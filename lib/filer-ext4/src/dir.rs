// This is free and unencumbered software released into the public domain.

//! Directories: blocks of entries, read linearly. Hashed (htree)
//! directories hide their index in blocks that read as one unused entry,
//! so a linear read sees exactly their entries too.

use crate::{Ext4Error, block_map::BlockMap, le, superblock::Superblock};
use alloc::vec::Vec;
use filer_api::{Error, FileKind, OpKind, ReadAt};

/// The longest name an entry holds.
const MAX_NAME: usize = 255;

/// The size of an entry's fixed fields, before its name.
const HEADER: usize = 8;

/// An entry of a directory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DirEntry {
    /// The entry's offset in the directory, and of the next entry.
    pub(crate) offset: u64,
    pub(crate) next: u64,
    pub(crate) inode: u32,
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

/// Parses the entries of block `block` of directory `dir`, at offset
/// `base` in it, skipping unused ones. With `filetype`, an entry's name
/// length is one byte and its kind the next; without, the length is two.
pub(crate) fn parse_block(
    block: &[u8],
    base: u64,
    filetype: bool,
    dir: u32,
) -> Result<Vec<DirEntry>, Ext4Error> {
    let mut entries = Vec::new();
    let mut at = 0usize;
    while at < block.len() {
        let offset = u64::try_from(at)
            .ok()
            .and_then(|at| base.checked_add(at))
            .unwrap_or(u64::MAX);
        let bad = Ext4Error::BadDirEntry { dir, offset };
        let inode = le::u32_at(block, at).ok_or(bad)?;
        let rec_len = match le::u16_at(block, at.saturating_add(4)).ok_or(bad)? {
            // A 64 KiB block's whole-block entry: 65536 does not fit.
            0xFFFF if block.len() == 0x1_0000 => 0x1_0000,
            len => usize::from(len),
        };
        let name_at = at.saturating_add(6);
        let (name_len, kind) = if filetype {
            let len = block.get(name_at).copied().ok_or(bad)?;
            let kind = block.get(name_at.saturating_add(1)).copied().ok_or(bad)?;
            (usize::from(len), kind)
        } else {
            (usize::from(le::u16_at(block, name_at).ok_or(bad)?), 0)
        };
        let needed = HEADER.saturating_add(name_len).next_multiple_of(4);
        let end = at.checked_add(rec_len).ok_or(bad)?;
        if name_len > MAX_NAME || rec_len % 4 != 0 || rec_len < needed || end > block.len() {
            return Err(bad);
        }
        if inode != 0 {
            let name_start = at.saturating_add(HEADER);
            let name = block
                .get(name_start..name_start.saturating_add(name_len))
                .ok_or(bad)?;
            if name.is_empty() || name.iter().any(|&b| b == b'/' || b == 0) {
                return Err(bad);
            }
            let kind = match kind {
                0 => None,
                1 => Some(FileKind::File),
                2 => Some(FileKind::Dir),
                3 => Some(FileKind::CharDevice),
                4 => Some(FileKind::BlockDevice),
                5 => Some(FileKind::Fifo),
                6 => Some(FileKind::Socket),
                7 => Some(FileKind::Symlink),
                _ => return Err(bad),
            };
            let next = u64::try_from(end)
                .ok()
                .and_then(|end| base.checked_add(end))
                .ok_or(bad)?;
            entries.push(DirEntry {
                offset,
                next,
                inode,
                name: name.to_vec(),
                kind,
            });
        }
        at = end;
    }
    Ok(entries)
}

/// A directory's contents: its map and size.
#[derive(Clone, Debug)]
pub(crate) struct Directory {
    inode: u32,
    map: BlockMap,
    size: u64,
}

impl Directory {
    /// Returns directory `inode`'s contents, `size` bytes mapped by `map`:
    /// whole blocks, and no more than the volume holds, as a directory has
    /// no holes. That bounds a scan, even if a corrupt map names one block
    /// many times.
    pub(crate) fn new(
        inode: u32,
        map: BlockMap,
        size: u64,
        sb: &Superblock,
    ) -> Result<Self, Ext4Error> {
        if !size.is_multiple_of(u64::from(sb.block_size())) || size > sb.volume_len() {
            return Err(Ext4Error::BadDirectorySize { dir: inode, size });
        }
        Ok(Self { inode, map, size })
    }

    /// Returns the directory's inode number.
    pub(crate) const fn inode(&self) -> u32 {
        self.inode
    }

    /// Returns the number of blocks in the directory.
    pub(crate) fn blocks(&self, sb: &Superblock) -> u64 {
        self.size
            .checked_div(u64::from(sb.block_size()))
            .unwrap_or(0)
    }

    /// Reads and parses block `index` of the directory.
    pub(crate) async fn block<S: ReadAt + ?Sized>(
        &self,
        source: &S,
        sb: &Superblock,
        index: u64,
        op: OpKind,
    ) -> Result<Vec<DirEntry>, Error> {
        let fail = |error: Ext4Error| error.into_error(op);
        let size = u64::from(sb.block_size());
        let base = index.checked_mul(size).ok_or_else(|| {
            fail(Ext4Error::BadDirectorySize {
                dir: self.inode,
                size: self.size,
            })
        })?;
        let len = usize::try_from(size).unwrap_or(usize::MAX);
        let bytes = self.map.read(source, sb, self.size, base, len, op).await?;
        parse_block(&bytes, base, sb.has_filetype(), self.inode).map_err(fail)
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
mod tests {
    use super::*;
    use alloc::vec;

    /// Lays out entries of (inode, name, kind) in a block of `size`
    /// bytes, the last one taking the rest.
    fn block(size: usize, entries: &[(u32, &[u8], u8)], filetype: bool) -> Vec<u8> {
        let mut b = vec![0; size];
        let mut at = 0;
        for (i, (inode, name, kind)) in entries.iter().enumerate() {
            let len = (8 + name.len()).next_multiple_of(4);
            let rec_len = if i + 1 == entries.len() {
                size - at
            } else {
                len
            };
            b[at..at + 4].copy_from_slice(&inode.to_le_bytes());
            let encoded = if rec_len == 0x1_0000 { 0xFFFF } else { rec_len };
            b[at + 4..at + 6].copy_from_slice(&u16::try_from(encoded).unwrap().to_le_bytes());
            if filetype {
                b[at + 6] = u8::try_from(name.len()).unwrap();
                b[at + 7] = *kind;
            } else {
                b[at + 6..at + 8]
                    .copy_from_slice(&u16::try_from(name.len()).unwrap().to_le_bytes());
            }
            b[at + 8..at + 8 + name.len()].copy_from_slice(name);
            at += rec_len;
        }
        b
    }

    #[test]
    fn bounds_directory_sizes() {
        use crate::{inode::Inode, inode::tests::Raw, superblock::tests::Fields};
        let sb = Superblock::parse(&Fields::new().bytes()).unwrap();
        let directory = |size: u64| {
            let mut raw = Raw::file(size);
            raw.mode = 0o040_755;
            let inode = Inode::parse(&raw.bytes(256), 12).unwrap();
            let map = BlockMap::new(&inode, &sb).unwrap();
            Directory::new(12, map, size, &sb).map(|dir| dir.blocks(&sb))
        };
        assert_eq!(directory(4096 * 1024), Ok(4096));
        for size in [1000, 4097 * 1024] {
            assert_eq!(
                directory(size).unwrap_err(),
                Ext4Error::BadDirectorySize { dir: 12, size }
            );
        }
    }

    #[test]
    fn parses_entries_with_kinds() {
        let b = block(
            1024,
            &[
                (2, b".", 2),
                (2, b"..", 2),
                (0, b"gone", 1),
                (12, b"file.txt", 1),
                (13, b"link", 7),
            ],
            true,
        );
        let entries = parse_block(&b, 2048, true, 2).unwrap();
        let names: Vec<_> = entries.iter().map(|e| e.name.as_slice()).collect();
        assert_eq!(names, [&b"."[..], b"..", b"file.txt", b"link"]);
        assert!(entries[0].is_dot() && entries[1].is_dot() && !entries[2].is_dot());
        assert_eq!(
            entries[2],
            DirEntry {
                offset: 2048 + 12 + 12 + 12,
                next: 2048 + 12 + 12 + 12 + 16,
                inode: 12,
                name: b"file.txt".to_vec(),
                kind: Some(FileKind::File),
            }
        );
        assert_eq!(entries[3].kind, Some(FileKind::Symlink));
        assert_eq!(entries[3].next, 2048 + 1024);
    }

    #[test]
    fn parses_entries_without_kinds() {
        let b = block(1024, &[(2, b".", 0), (11, b"lost+found", 0)], false);
        let entries = parse_block(&b, 0, false, 2).unwrap();
        assert_eq!(entries[1].name, b"lost+found");
        assert_eq!(entries[1].kind, None);
    }

    #[test]
    fn parses_whole_64k_blocks() {
        let b = block(0x1_0000, &[(0, b"", 0)], true);
        assert_eq!(&b[4..6], &[0xFF, 0xFF]);
        assert!(parse_block(&b, 0, true, 2).unwrap().is_empty());
        // Not in a smaller block.
        let mut b = block(1024, &[(0, b"", 0)], true);
        b[4..6].copy_from_slice(&[0xFF, 0xFF]);
        assert!(parse_block(&b, 0, true, 2).is_err());
    }

    #[test]
    fn parses_htree_blocks_as_unused() {
        // An interior node: one unused entry spanning the block, then the
        // index, which is not read.
        let mut b = block(1024, &[(0, b"", 0)], true);
        b[8..16].copy_from_slice(&[0x7C, 0, 1, 0, 1, 0, 0, 0]);
        assert!(parse_block(&b, 0, true, 2).unwrap().is_empty());
    }

    #[test]
    fn refuses_malformed_entries() {
        let bad = |b: &[u8]| parse_block(b, 4096, true, 5).unwrap_err();
        let good = block(1024, &[(2, b".", 2), (12, b"name", 1)], true);
        let entry = Ext4Error::BadDirEntry {
            dir: 5,
            offset: 4096 + 12,
        };
        let mut b = good.clone();
        // A record length of zero, not a multiple of four, too short for
        // the name, or past the block.
        for rec_len in [0u16, 13, 8, 1020] {
            b[16..18].copy_from_slice(&rec_len.to_le_bytes());
            assert_eq!(bad(&b), entry, "{rec_len}");
        }
        // Names with `/` or NUL, or empty.
        let mut b = good.clone();
        b[21] = b'/';
        assert_eq!(bad(&b), entry);
        b[21] = 0;
        assert_eq!(bad(&b), entry);
        let mut b = good.clone();
        b[18] = 0;
        assert_eq!(bad(&b), entry);
        // An unknown kind.
        let mut b = good.clone();
        b[19] = 8;
        assert_eq!(bad(&b), entry);
        // A name longer than 255 bytes, without kinds.
        let mut b = block(1024, &[(2, b".", 0), (12, b"name", 0)], false);
        b[18..20].copy_from_slice(&256u16.to_le_bytes());
        assert_eq!(parse_block(&b, 4096, false, 5).unwrap_err(), entry);
        // A truncated header.
        assert!(parse_block(&good[..14], 0, true, 5).is_err());
    }
}

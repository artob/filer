// This is free and unencumbered software released into the public domain.

//! Building the index of an archive: one pass over its headers, then one
//! over what they said, since a hardlinked file's data may come with its
//! last link.

use crate::{
    codec::Sum,
    header::{
        BLOCK, CHAR, DIR, FIFO, FILE, Format, Header, MAX_HEADER, SOCKET, SYMLINK, TRAILER, TYPE,
        align, is_compressed, parse,
    },
};
use alloc::{boxed::Box, collections::BTreeMap, vec::Vec};
use core::time::Duration;
use filer_api::{
    BufResult, Error, ErrorKind, FileKind, ReadAt, Timestamp,
    tree::{Ino, Node, Tree},
};
use filer_pack::{ArchiveError, Index, Limits, Member};

fn fail(kind: ErrorKind, offset: u64, reason: &'static str) -> Error {
    ArchiveError::new("cpio", offset, reason).into_error(kind)
}

/// Reads exactly `len` bytes at `offset`.
async fn read<S: ReadAt>(source: &S, offset: u64, len: usize) -> Result<Vec<u8>, Error> {
    let BufResult(result, buf) = source.read_exact_at(Vec::with_capacity(len), offset).await;
    match result {
        Ok(()) => Ok(buf),
        Err(error) if error.kind() == ErrorKind::UnexpectedEof => {
            Err(fail(ErrorKind::Corrupt, offset, "truncated"))
        },
        Err(error) => Err(error),
    }
}

/// Identifies a hardlinked file: its archive's number among those
/// concatenated, its device, and its inode.
type LinkKey = (usize, u64, u64);

/// A member, as its header described it.
struct Entry {
    /// The header's offset.
    offset: u64,
    name: Vec<u8>,
    item: Item,
}

enum Item {
    Dir,
    File {
        member: Member<Sum>,
        link: Option<LinkKey>,
    },
    Symlink(Box<[u8]>),
    Special(FileKind),
}

/// Builds the index of the archive in `source`: of each of the archives
/// concatenated there, as in a Linux initramfs image.
pub(crate) async fn index<S: ReadAt>(
    source: &S,
    limits: &Limits,
) -> Result<Tree<Member<Sum>>, Error> {
    let entries = entries(source, limits).await?;
    // The member holding each hardlinked file's data: the last with any,
    // as `newc` stores it only with the last link.
    let mut data: BTreeMap<LinkKey, Member<Sum>> = BTreeMap::new();
    for entry in &entries {
        if let Item::File {
            member,
            link: Some(key),
        } = &entry.item
            && (member.size > 0 || !data.contains_key(key))
        {
            data.insert(*key, *member);
        }
    }
    let mut index = Index::new("cpio");
    let mut nodes: BTreeMap<LinkKey, Ino> = BTreeMap::new();
    for Entry { offset, name, item } in entries {
        match item {
            Item::Dir => index.dir(&name, offset)?,
            Item::Symlink(target) => index.insert(&name, Node::Symlink(target), offset)?,
            Item::Special(kind) => index.insert(&name, Node::Special(kind), offset)?,
            Item::File { member, link: None } => index.insert(&name, Node::File(member), offset)?,
            Item::File {
                member,
                link: Some(key),
            } => match nodes.get(&key) {
                Some(&ino) => index.alias(&name, ino, offset)?,
                None => {
                    let member = data.get(&key).copied().unwrap_or(member);
                    let ino = index.add(&name, Node::File(member), offset)?;
                    nodes.insert(key, ino);
                },
            },
        }
    }
    Ok(index.into_tree())
}

/// Reads every member's header, in order.
async fn entries<S: ReadAt>(source: &S, limits: &Limits) -> Result<Vec<Entry>, Error> {
    let end = source.size().await?;
    let mut entries = Vec::new();
    let mut members = 0_usize;
    let mut offset = 0_u64;
    // Where the current archive began, which alignment counts from, and
    // its number.
    let mut base = 0_u64;
    let mut archive = 0_usize;
    if end == 0 {
        return Err(fail(ErrorKind::Corrupt, 0, "not a cpio archive"));
    }
    // An archive may end without its trailer.
    while offset < end {
        let available = usize::try_from(end.saturating_sub(offset)).unwrap_or(MAX_HEADER);
        let bytes = read(source, offset, available.min(MAX_HEADER)).await?;
        let corrupt = |reason| fail(ErrorKind::Corrupt, offset, reason);
        let Some(format) = Format::detect(&bytes) else {
            return Err(if offset != base {
                corrupt("bad magic number")
            } else if is_compressed(&bytes) {
                fail(ErrorKind::Unsupported, offset, "compressed archive")
            } else {
                corrupt("not a cpio archive")
            });
        };
        let header = bytes
            .get(..format.len())
            .ok_or_else(|| corrupt("truncated header"))?;
        let header = parse(format, header).ok_or_else(|| corrupt("bad header"))?;
        members = members.saturating_add(1);
        if members > limits.max_entries {
            return Err(fail(ErrorKind::LimitExceeded, offset, "too many members"));
        }

        // The name, with its NUL, within the archive.
        let name_at = offset.saturating_add(u64::try_from(format.len()).unwrap_or(u64::MAX));
        let too_long = || fail(ErrorKind::LimitExceeded, offset, "name too long");
        let name_len = usize::try_from(header.name_len).map_err(|_| too_long())?;
        if name_len > limits.max_path_len.saturating_add(1) {
            return Err(too_long());
        }
        if header.name_len > end.saturating_sub(name_at) {
            return Err(corrupt("truncated name"));
        }
        let name = read(source, name_at, name_len).await?;
        let Some((0, name)) = name.split_last() else {
            return Err(corrupt("bad name"));
        };
        let alignment = format.alignment();
        let data = align(name_at.saturating_add(header.name_len), base, alignment)
            .ok_or_else(|| corrupt("bad header"))?;
        if header.size > end.saturating_sub(data) {
            return Err(corrupt("member extends past the end"));
        }
        // Within the archive but for its padding, as the data is.
        let next = align(data.saturating_add(header.size), base, alignment)
            .ok_or_else(|| corrupt("member extends past the end"))?;

        if name == TRAILER {
            // Padding, perhaps another archive, or the end.
            offset = skip_zeros(source, next.min(end), end).await?;
            base = offset;
            archive = archive.saturating_add(1);
            continue;
        }
        let crc = matches!(format, Format::Newc { crc: true });
        let item = match header.mode & TYPE {
            DIR => Item::Dir,
            FILE => Item::File {
                member: member(&header, data, crc),
                link: (header.nlink > 1).then_some((archive, header.dev, header.ino)),
            },
            SYMLINK => {
                let len = usize::try_from(header.size)
                    .ok()
                    .filter(|&len| len <= limits.max_path_len)
                    .ok_or_else(too_long)?;
                let target = read(source, data, len).await?;
                Item::Symlink(target.into_boxed_slice())
            },
            CHAR => Item::Special(FileKind::CharDevice),
            BLOCK => Item::Special(FileKind::BlockDevice),
            FIFO => Item::Special(FileKind::Fifo),
            SOCKET => Item::Special(FileKind::Socket),
            _ => return Err(corrupt("unknown file type")),
        };
        entries.push(Entry {
            offset,
            name: name.to_vec(),
            item,
        });
        offset = next;
    }
    Ok(entries)
}

/// Returns a regular file's member, at `data` in the archive.
fn member(header: &Header, data: u64, crc: bool) -> Member<Sum> {
    let modified = i64::try_from(header.mtime)
        .ok()
        .map(|secs| Timestamp::new(secs, 0).with_granularity(Duration::from_secs(1)));
    let codec = Sum(crc.then_some(header.check));
    Member::encoded(data, header.size, header.size, codec).with_modified(modified)
}

/// Returns the offset of the first byte from `from` that is not zero, or
/// `end`.
async fn skip_zeros<S: ReadAt>(source: &S, from: u64, end: u64) -> Result<u64, Error> {
    const CHUNK: u64 = 512;
    let mut offset = from;
    while offset < end {
        let len = end.saturating_sub(offset).min(CHUNK);
        let bytes = read(source, offset, usize::try_from(len).unwrap_or(0)).await?;
        if let Some(at) = bytes.iter().position(|&b| b != 0) {
            return Ok(offset.saturating_add(u64::try_from(at).unwrap_or(0)));
        }
        offset = offset.saturating_add(len);
    }
    Ok(end)
}

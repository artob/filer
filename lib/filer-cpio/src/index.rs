// This is free and unencumbered software released into the public domain.

//! Building the index of an archive: one pass over its headers. A
//! hardlinked file's node is added with its first link, and its data,
//! which `newc` stores with the last link only, replaces the first's.

use crate::{
    codec::Sum,
    header::{Format, Header, MAX_HEADER, TRAILER, align, is_compressed, parse},
};
use alloc::collections::BTreeMap;
use core::time::Duration;
use filer_api::{
    Error, ErrorKind, FileKind, ReadAt, Timestamp,
    tree::{Ino, Node, Tree},
};
use filer_pack::{Index, Limits, Member, Parser};

const CPIO: Parser = Parser::new("cpio");

/// Identifies a hardlinked file: its archive's number among those
/// concatenated, its device, and its inode.
type LinkKey = (usize, u64, u64);

/// Builds the index of the archive in `source`: of each of the archives
/// concatenated there, as in a Linux initramfs image.
pub(crate) async fn index<S: ReadAt>(
    source: &S,
    limits: &Limits,
) -> Result<Tree<Member<Sum>>, Error> {
    let end = source.size().await?;
    let mut index = Index::new(CPIO, *limits);
    // The node of each hardlinked file seen so far.
    let mut nodes: BTreeMap<LinkKey, Ino> = BTreeMap::new();
    let mut offset = 0_u64;
    // Where the current archive began, which alignment counts from, and
    // its number.
    let mut base = 0_u64;
    let mut archive = 0_usize;
    if end == 0 {
        return Err(CPIO.fail(ErrorKind::Corrupt, 0, "not a cpio archive"));
    }
    // An archive may end without its trailer.
    while offset < end {
        let available = usize::try_from(end.saturating_sub(offset)).unwrap_or(MAX_HEADER);
        let bytes = CPIO.read(source, offset, available.min(MAX_HEADER)).await?;
        let corrupt = |reason| CPIO.fail(ErrorKind::Corrupt, offset, reason);
        let Some(format) = Format::detect(&bytes) else {
            return Err(if offset != base {
                corrupt("bad magic number")
            } else if is_compressed(&bytes) {
                CPIO.fail(ErrorKind::Unsupported, offset, "compressed archive")
            } else {
                corrupt("not a cpio archive")
            });
        };
        let header = bytes
            .get(..format.len())
            .ok_or_else(|| corrupt("truncated header"))?;
        let header = parse(format, header).ok_or_else(|| corrupt("bad header"))?;

        // The name, with its NUL, within the archive.
        let name_at = offset.saturating_add(u64::try_from(format.len()).unwrap_or(u64::MAX));
        if header.name_len == 0 {
            return Err(corrupt("bad name"));
        }
        let name_len = index
            .name_len(header.name_len.saturating_sub(1), offset)?
            .saturating_add(1);
        if header.name_len > end.saturating_sub(name_at) {
            return Err(corrupt("truncated name"));
        }
        let name = CPIO.read(source, name_at, name_len).await?;
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
        let kind = u32::try_from(header.mode)
            .ok()
            .and_then(FileKind::from_unix_mode)
            .ok_or_else(|| corrupt("unknown file type"))?;
        match kind {
            FileKind::Dir => index.dir(name, offset)?,
            FileKind::File if header.nlink > 1 => {
                let member = member(&header, data, crc);
                let key = (archive, header.dev, header.ino);
                match nodes.get(&key) {
                    Some(&ino) => {
                        index.alias(name, ino, offset)?;
                        if member.size > 0 {
                            index.set_member(ino, member, offset)?;
                        }
                    },
                    None => {
                        let ino = index.add(name, Node::File(member), offset)?;
                        nodes.insert(key, ino);
                    },
                }
            },
            FileKind::File => index.insert(name, Node::File(member(&header, data, crc)), offset)?,
            FileKind::Symlink => {
                let len = index.target_len(header.size, offset)?;
                let target = CPIO.read(source, data, len).await?;
                index.insert(name, Node::Symlink(target.into_boxed_slice()), offset)?;
            },
            // Devices, FIFOs, and sockets.
            kind => index.insert(name, Node::Special(kind), offset)?,
        }
        offset = next;
    }
    Ok(index.into_tree())
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
        let bytes = CPIO
            .read(source, offset, usize::try_from(len).unwrap_or(0))
            .await?;
        if let Some(at) = bytes.iter().position(|&b| b != 0) {
            return Ok(offset.saturating_add(u64::try_from(at).unwrap_or(0)));
        }
        offset = offset.saturating_add(len);
    }
    Ok(end)
}

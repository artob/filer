// This is free and unencumbered software released into the public domain.

//! Building the index of an archive: one pass over its headers.

use crate::header::{BLOCK, Header, decimal, pax_records, pax_time, text};
use alloc::{boxed::Box, vec::Vec};
use core::time::Duration;
use filer_api::{
    BufResult, Error, ErrorKind, FileKind, ReadAt, Timestamp,
    archive::{ArchiveError, Index, Limits, Member},
    tree::{Node, Tree},
};

fn fail(kind: ErrorKind, offset: u64, reason: &'static str) -> Error {
    ArchiveError::new("tar", offset, reason).into_error(kind)
}

/// Overrides from a PAX extended header or GNU long-name records, for the
/// next member only.
#[derive(Default)]
struct Pending {
    path: Option<Vec<u8>>,
    linkpath: Option<Vec<u8>>,
    size: Option<u64>,
    mtime: Option<(i64, u32)>,
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

/// Builds the index of the archive in `source`.
pub(crate) async fn index<S: ReadAt>(source: &S, limits: &Limits) -> Result<Tree<Member>, Error> {
    let end = source.size().await?;
    let mut index = Index::new("tar");
    let mut pending = Pending::default();
    let mut entries = 0_usize;
    let mut offset = 0_u64;
    loop {
        // The archive should end with zero blocks, but ending at a block
        // boundary is accepted too.
        if offset == end {
            break;
        }
        if end - offset < BLOCK as u64 {
            return Err(fail(ErrorKind::Corrupt, offset, "truncated header"));
        }
        let block = read(source, offset, BLOCK).await?;
        let block: &[u8; BLOCK] = block.as_slice().try_into().expect("read one block");
        let header = Header(block);
        if header.is_zero() {
            break;
        }
        if !header.checksum_ok() {
            return Err(fail(ErrorKind::Corrupt, offset, "bad checksum"));
        }
        let typeflag = header.typeflag();
        let data = offset + BLOCK as u64;
        let header_size = header
            .size()
            .ok_or_else(|| fail(ErrorKind::Corrupt, offset, "bad size"))?;
        // A PAX size overrides the header's, for regular members.
        let size = match typeflag {
            b'x' | b'g' | b'L' | b'K' => header_size,
            _ => pending.size.take().unwrap_or(header_size),
        };
        if size > end - data {
            return Err(fail(
                ErrorKind::Corrupt,
                offset,
                "member extends past the end",
            ));
        }
        let next = data + size.div_ceil(BLOCK as u64) * BLOCK as u64;
        match typeflag {
            b'x' | b'L' | b'K' => {
                if size > limits.max_extension_len {
                    let reason = "extended header too large";
                    return Err(fail(ErrorKind::LimitExceeded, offset, reason));
                }
                let record = read(source, data, size as usize).await?;
                match typeflag {
                    b'x' => apply_pax(&mut pending, &record, offset)?,
                    b'L' => pending.path = Some(text(&record).to_vec()),
                    _ => pending.linkpath = Some(text(&record).to_vec()),
                }
                offset = next;
                continue;
            },
            // Global PAX headers are not applied.
            b'g' => {
                offset = next;
                continue;
            },
            b'S' => {
                return Err(fail(ErrorKind::Unsupported, offset, "GNU sparse members"));
            },
            _ => {},
        }

        entries += 1;
        if entries > limits.max_entries {
            return Err(fail(ErrorKind::LimitExceeded, offset, "too many members"));
        }
        let name = pending.path.take().unwrap_or_else(|| header.name());
        let link = pending
            .linkpath
            .take()
            .unwrap_or_else(|| header.linkname().to_vec());
        if name.len() > limits.max_path_len || link.len() > limits.max_path_len {
            return Err(fail(ErrorKind::LimitExceeded, offset, "name too long"));
        }
        let modified = match pending.mtime.take() {
            Some((secs, nanos)) => Some(Timestamp::new(secs, nanos)),
            None => header
                .mtime()
                .and_then(|secs| i64::try_from(secs).ok())
                .map(|secs| Timestamp::new(secs, 0).with_granularity(Duration::from_secs(1))),
        };
        match typeflag {
            b'5' => index.dir(&name, offset)?,
            b'1' => index.hardlink(&name, &link, offset)?,
            b'2' => index.insert(&name, Node::Symlink(Box::from(link.as_slice())), offset)?,
            b'3' => index.insert(&name, Node::Special(FileKind::CharDevice), offset)?,
            b'4' => index.insert(&name, Node::Special(FileKind::BlockDevice), offset)?,
            b'6' => index.insert(&name, Node::Special(FileKind::Fifo), offset)?,
            // Regular files, contiguous files, and, as POSIX requires,
            // unknown types.
            _ => {
                let member = Member::new(data, size).with_modified(modified);
                index.insert(&name, Node::File(member), offset)?;
            },
        }
        offset = next;
    }
    Ok(index.into_tree())
}

fn apply_pax(pending: &mut Pending, record: &[u8], offset: u64) -> Result<(), Error> {
    let corrupt = || fail(ErrorKind::Corrupt, offset, "bad PAX header");
    for (key, value) in pax_records(record).ok_or_else(corrupt)? {
        match key {
            b"path" => pending.path = Some(value.to_vec()),
            b"linkpath" => pending.linkpath = Some(value.to_vec()),
            b"size" => pending.size = Some(decimal(value).ok_or_else(corrupt)?),
            b"mtime" => pending.mtime = Some(pax_time(value).ok_or_else(corrupt)?),
            _ => {},
        }
    }
    Ok(())
}

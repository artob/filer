// This is free and unencumbered software released into the public domain.

//! Building the index of an archive: one pass over its headers.

use crate::header::{
    HEADER, HEADER_LEN, Header, MAGIC, Name, THIN_MAGIC, bsd_name, classify, is_bsd_symbol_table,
    long_name,
};
use alloc::vec::Vec;
use core::time::Duration;
use filer_api::{
    BufResult, Error, ErrorKind, ReadAt, Timestamp,
    tree::{Node, Tree},
};
use filer_pack::{ArchiveError, Index, Limits, Member};

fn fail(kind: ErrorKind, offset: u64, reason: &'static str) -> Error {
    ArchiveError::new("ar", offset, reason).into_error(kind)
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
    let not_ar = || fail(ErrorKind::Corrupt, 0, "not an ar archive");
    let magic = read(source, 0, MAGIC.len()).await.map_err(|_| not_ar())?;
    if magic.as_slice() == THIN_MAGIC {
        return Err(fail(ErrorKind::Unsupported, 0, "thin archives"));
    }
    if magic.as_slice() != MAGIC {
        return Err(not_ar());
    }
    let mut index = Index::new("ar");
    let mut table: Option<Vec<u8>> = None;
    let mut members = 0_usize;
    // Past the magic number, which `read` found.
    let mut offset = 8_u64;
    // The last member's padding may be missing.
    while offset < end {
        if end.saturating_sub(offset) < HEADER_LEN {
            return Err(fail(ErrorKind::Corrupt, offset, "truncated header"));
        }
        let block = read(source, offset, HEADER).await?;
        let block: &[u8; HEADER] = block
            .as_slice()
            .try_into()
            .map_err(|_| fail(ErrorKind::Corrupt, offset, "truncated header"))?;
        let header = Header(block);
        if !header.is_valid() {
            return Err(fail(ErrorKind::Corrupt, offset, "bad header"));
        }
        let corrupt = |reason| fail(ErrorKind::Corrupt, offset, reason);
        let size = header.size().ok_or_else(|| corrupt("bad size"))?;
        // Within the archive, as `offset + HEADER_LEN <= end`.
        let data = offset.saturating_add(HEADER_LEN);
        if size > end.saturating_sub(data) {
            return Err(corrupt("member extends past the end"));
        }
        // An odd length is padded to an even one. Both fit, since `size`
        // lies within the archive.
        let next = data.saturating_add(size).saturating_add(size & 1);
        members = members.saturating_add(1);
        if members > limits.max_entries {
            return Err(fail(ErrorKind::LimitExceeded, offset, "too many members"));
        }
        let modified = header
            .mtime()
            .ok_or_else(|| corrupt("bad time"))?
            .and_then(|secs| i64::try_from(secs).ok())
            .map(|secs| Timestamp::new(secs, 0).with_granularity(Duration::from_secs(1)));
        let member = |offset, size| Node::File(Member::new(offset, size).with_modified(modified));
        match classify(header.raw_name()) {
            Name::Short(name) => {
                insert(&mut index, name, member(data, size), offset, limits)?;
            },
            Name::Long(at) => {
                let table = table
                    .as_deref()
                    .ok_or_else(|| corrupt("long name before the name table"))?;
                let name = long_name(table, at).ok_or_else(|| corrupt("bad long name"))?;
                insert(&mut index, name, member(data, size), offset, limits)?;
            },
            Name::Bsd(len) => {
                if len > size {
                    return Err(corrupt("bad long name"));
                }
                let too_long = || fail(ErrorKind::LimitExceeded, offset, "name too long");
                let len = usize::try_from(len)
                    .ok()
                    .filter(|&len| len <= limits.max_path_len)
                    .ok_or_else(too_long)?;
                let name = read(source, data, len).await?;
                let name = bsd_name(&name);
                if !is_bsd_symbol_table(name) {
                    // Both within the member, as `len <= size`.
                    let len = u64::try_from(len).map_err(|_| too_long())?;
                    let file = member(data.saturating_add(len), size.saturating_sub(len));
                    insert(&mut index, name, file, offset, limits)?;
                }
            },
            Name::Table => {
                if table.is_some() {
                    return Err(corrupt("second name table"));
                }
                let too_large = || fail(ErrorKind::LimitExceeded, offset, "name table too large");
                let len = usize::try_from(size)
                    .ok()
                    .filter(|_| size <= limits.max_extension_len)
                    .ok_or_else(too_large)?;
                table = Some(read(source, data, len).await?);
            },
            Name::Special => {},
            Name::Bad => return Err(corrupt("bad name")),
        }
        offset = next;
    }
    Ok(index.into_tree())
}

/// Records the member `name`, from the header at `offset`.
fn insert(
    index: &mut Index,
    name: &[u8],
    node: Node<Member>,
    offset: u64,
    limits: &Limits,
) -> Result<(), Error> {
    if name.len() > limits.max_path_len {
        return Err(fail(ErrorKind::LimitExceeded, offset, "name too long"));
    }
    index.insert(name, node, offset)
}

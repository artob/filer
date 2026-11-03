// This is free and unencumbered software released into the public domain.

//! Building the index of an archive: one pass over its headers.

use crate::header::{
    HEADER, HEADER_LEN, Header, MAGIC, Name, THIN_MAGIC, bsd_name, classify, is_bsd_symbol_table,
    long_name,
};
use alloc::vec::Vec;
use core::time::Duration;
use filer_api::{
    Error, ErrorKind, ReadAt, Timestamp,
    tree::{Node, Tree},
};
use filer_pack::{Index, Limits, Member, Parser, Reader};

const AR: Parser = Parser::new("ar");

/// Builds the index of the archive in `source`.
pub(crate) async fn index<S: ReadAt>(source: &S, limits: &Limits) -> Result<Tree<Member>, Error> {
    let end = source.size().await?;
    let mut reader = Reader::new(source, AR, end);
    let not_ar = || AR.fail(ErrorKind::Corrupt, 0, "not an ar archive");
    let magic = reader.read(0, MAGIC.len()).await.map_err(|_| not_ar())?;
    if magic == THIN_MAGIC {
        return Err(AR.fail(ErrorKind::Unsupported, 0, "thin archives"));
    }
    if magic != MAGIC {
        return Err(not_ar());
    }
    let mut index = Index::new(AR, *limits);
    let mut table: Option<Vec<u8>> = None;
    // Past the magic number, which `read` found.
    let mut offset = 8_u64;
    // The last member's padding may be missing.
    while offset < end {
        if end.saturating_sub(offset) < HEADER_LEN {
            return Err(AR.fail(ErrorKind::Corrupt, offset, "truncated header"));
        }
        let block: [u8; HEADER] = reader
            .read(offset, HEADER)
            .await?
            .try_into()
            .map_err(|_| AR.fail(ErrorKind::Corrupt, offset, "truncated header"))?;
        let header = Header(&block);
        if !header.is_valid() {
            return Err(AR.fail(ErrorKind::Corrupt, offset, "bad header"));
        }
        let corrupt = |reason| AR.fail(ErrorKind::Corrupt, offset, reason);
        let size = header.size().ok_or_else(|| corrupt("bad size"))?;
        // Within the archive, as `offset + HEADER_LEN <= end`.
        let data = offset.saturating_add(HEADER_LEN);
        if size > end.saturating_sub(data) {
            return Err(corrupt("member extends past the end"));
        }
        // An odd length is padded to an even one. Both fit, since `size`
        // lies within the archive.
        let next = data.saturating_add(size).saturating_add(size & 1);
        let modified = header
            .mtime()
            .ok_or_else(|| corrupt("bad time"))?
            .and_then(|secs| i64::try_from(secs).ok())
            .map(|secs| Timestamp::new(secs, 0).with_granularity(Duration::from_secs(1)));
        let member = |offset, size| Node::File(Member::new(offset, size).with_modified(modified));
        match classify(header.raw_name()) {
            Name::Short(name) => index.insert(name, member(data, size), offset)?,
            Name::Long(at) => {
                let table = table
                    .as_deref()
                    .ok_or_else(|| corrupt("long name before the name table"))?;
                let name = long_name(table, at).ok_or_else(|| corrupt("bad long name"))?;
                index.insert(name, member(data, size), offset)?;
            },
            Name::Bsd(len) => {
                if len > size {
                    return Err(corrupt("bad long name"));
                }
                let name = reader.read(data, index.name_len(len, offset)?).await?;
                let name = bsd_name(name);
                if !is_bsd_symbol_table(name) {
                    // Both within the member, as `len <= size`.
                    let file = member(data.saturating_add(len), size.saturating_sub(len));
                    index.insert(name, file, offset)?;
                }
            },
            Name::Table => {
                if table.is_some() {
                    return Err(corrupt("second name table"));
                }
                let too_large =
                    || AR.fail(ErrorKind::LimitExceeded, offset, "name table too large");
                let len = usize::try_from(size)
                    .ok()
                    .filter(|_| size <= limits.max_extension_len)
                    .ok_or_else(too_large)?;
                table = Some(reader.read(data, len).await?.to_vec());
            },
            Name::Special => {},
            Name::Bad => return Err(corrupt("bad name")),
        }
        offset = next;
    }
    Ok(index.into_tree())
}

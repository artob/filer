// This is free and unencumbered software released into the public domain.

//! Building the index of an archive: its central directory, each header
//! checked against its member's local header.

use crate::header::{
    CENTRAL_LEN, Central, CentralFixed, EOCD_LEN, EOCD64_LEN, Eocd, Kind, LOCAL_LEN, LOCATOR_LEN,
    LocalFixed, MAX_COMMENT, STORED, find_eocd, parse_locator,
};
use alloc::{boxed::Box, vec::Vec};
use filer_api::{
    BufResult, Error, ErrorKind, ReadAt,
    archive::{ArchiveError, Index, Limits, Member},
    tree::{Node, Tree},
};

/// How much of the central directory to read at once.
const CHUNK: usize = 64 * 1024;

fn fail(kind: ErrorKind, offset: u64, reason: &'static str) -> Error {
    ArchiveError::new("zip", offset, reason).into_error(kind)
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

/// Converts a length that fits in memory to a `u64`.
fn wide(len: usize) -> u64 {
    u64::try_from(len).unwrap_or(u64::MAX)
}

/// A forward reader over a range of the source, reading ahead in chunks so
/// that small headers do not each cost a read.
struct Reader<'s, S> {
    source: &'s S,
    /// The offset of the first byte of `buf` not yet taken.
    pos: u64,
    /// The end of the range.
    end: u64,
    buf: Vec<u8>,
    taken: usize,
}

impl<'s, S: ReadAt> Reader<'s, S> {
    fn new(source: &'s S, start: u64, end: u64) -> Self {
        Self {
            source,
            pos: start,
            end,
            buf: Vec::new(),
            taken: 0,
        }
    }

    /// Takes the next `len` bytes, or fails if the range ends first.
    async fn take(&mut self, len: usize) -> Result<&[u8], Error> {
        let available = self.buf.len().saturating_sub(self.taken);
        if available < len {
            let buffered = self.pos.saturating_add(wide(available));
            let remaining = self.end.saturating_sub(buffered);
            let wanted = len.saturating_sub(available).max(CHUNK);
            let wanted = usize::try_from(remaining).map_or(wanted, |r| r.min(wanted));
            if available.saturating_add(wanted) < len {
                return Err(fail(ErrorKind::Corrupt, self.pos, "truncated header"));
            }
            let more = read(self.source, buffered, wanted).await?;
            self.buf.drain(..self.taken);
            self.buf.extend_from_slice(&more);
            self.taken = 0;
        }
        let start = self.taken;
        let stop = start.saturating_add(len);
        let bytes = self
            .buf
            .get(start..stop)
            .ok_or_else(|| fail(ErrorKind::Corrupt, self.pos, "truncated header"))?;
        self.taken = stop;
        self.pos = self.pos.saturating_add(wide(len));
        Ok(bytes)
    }
}

/// Locates the central directory from the records at the end of the
/// archive, and returns it with the offset where it must end.
async fn locate<S: ReadAt>(source: &S, end: u64) -> Result<(Eocd, u64), Error> {
    let not_zip = || {
        fail(
            ErrorKind::Corrupt,
            end,
            "no end of central directory record",
        )
    };
    let tail_len = end.min(wide(EOCD_LEN.saturating_add(MAX_COMMENT)));
    let tail_start = end.saturating_sub(tail_len);
    let tail = read(source, tail_start, usize::try_from(tail_len).unwrap_or(0)).await?;
    let pos = find_eocd(&tail).ok_or_else(not_zip)?;
    let eocd_offset = tail_start.saturating_add(wide(pos));
    let (eocd, _) = tail.get(pos..).and_then(Eocd::parse).ok_or_else(not_zip)?;

    let locator = pos
        .checked_sub(LOCATOR_LEN)
        .and_then(|at| tail.get(at..pos))
        .and_then(parse_locator);
    let (eocd, cd_end) = match locator {
        None => (eocd, eocd_offset),
        Some((offset, disks)) => {
            let locator_offset = eocd_offset.saturating_sub(wide(LOCATOR_LEN));
            if disks > 1 {
                return Err(fail(
                    ErrorKind::Unsupported,
                    locator_offset,
                    "spanned archives",
                ));
            }
            let bad = || fail(ErrorKind::Corrupt, offset, "bad ZIP64 end record");
            if offset.saturating_add(wide(EOCD64_LEN)) > locator_offset {
                return Err(bad());
            }
            let record = read(source, offset, EOCD64_LEN).await?;
            let zip64 = Eocd::parse64(&record).ok_or_else(bad)?;
            if !eocd.agrees_with(&zip64) {
                return Err(bad());
            }
            (zip64, offset)
        },
    };
    if eocd.disk != 0 || eocd.cd_disk != 0 || eocd.disk_entries != eocd.entries {
        return Err(fail(
            ErrorKind::Unsupported,
            eocd_offset,
            "spanned archives",
        ));
    }
    if eocd.cd_offset.saturating_add(eocd.cd_size) > cd_end {
        let reason = "central directory past its end record";
        return Err(fail(ErrorKind::Corrupt, eocd_offset, reason));
    }
    Ok((eocd, eocd.cd_offset.saturating_add(eocd.cd_size)))
}

/// Builds the index of the archive in `source`.
pub(crate) async fn index<S: ReadAt>(source: &S, limits: &Limits) -> Result<Tree<Member>, Error> {
    let end = source.size().await?;
    let (eocd, cd_end) = locate(source, end).await?;
    if eocd.entries > wide(limits.max_entries) {
        return Err(fail(ErrorKind::LimitExceeded, cd_end, "too many members"));
    }
    let mut index = Index::new("zip");
    let mut reader = Reader::new(source, eocd.cd_offset, cd_end);
    for _ in 0..eocd.entries {
        let offset = reader.pos;
        let corrupt = |reason| fail(ErrorKind::Corrupt, offset, reason);
        let fixed = CentralFixed::parse(reader.take(CENTRAL_LEN).await?)
            .ok_or_else(|| corrupt("bad central directory header"))?;
        let central = fixed
            .complete(reader.take(fixed.variable_len()).await?)
            .ok_or_else(|| corrupt("missing ZIP64 field"))?;
        add(source, &mut index, &central, eocd.cd_offset, offset, limits).await?;
    }
    if reader.pos != cd_end {
        let reason = "central directory size mismatch";
        return Err(fail(ErrorKind::Corrupt, reader.pos, reason));
    }
    Ok(index.into_tree())
}

/// Adds the member `central`, from the central directory header at
/// `offset`, after checking its local header.
async fn add<S: ReadAt>(
    source: &S,
    index: &mut Index,
    central: &Central,
    cd_offset: u64,
    offset: u64,
    limits: &Limits,
) -> Result<(), Error> {
    let name = central.name.as_slice();
    if name.len() > limits.max_path_len {
        return Err(fail(ErrorKind::LimitExceeded, offset, "name too long"));
    }
    // ZIP names use `/` alone; a backslash or drive letter would mean
    // something else to a Windows extractor.
    if name.contains(&b'\\') || matches!(name, [letter, b':', ..] if letter.is_ascii_alphabetic()) {
        return Err(fail(ErrorKind::InvalidName, offset, "unsafe name"));
    }
    if central.disk != 0 {
        return Err(fail(ErrorKind::Unsupported, offset, "spanned archives"));
    }
    let data = check_local(source, central, cd_offset).await?;

    let kind = central.kind();
    if kind != Kind::Dir {
        if central.encrypted() {
            return Err(fail(ErrorKind::Unsupported, offset, "encrypted members"));
        }
        if central.fixed.method != STORED {
            let reason = "compression method";
            return Err(fail(ErrorKind::Unsupported, offset, reason));
        }
        if central.size != central.compressed {
            let reason = "stored member sizes differ";
            return Err(fail(ErrorKind::Corrupt, offset, reason));
        }
    }
    match kind {
        Kind::Dir => index.dir(name, offset),
        Kind::File => {
            let member = Member::new(data, central.size).with_modified(central.modified);
            index.insert(name, Node::File(member), offset)
        },
        Kind::Symlink => {
            let len = usize::try_from(central.size)
                .ok()
                .filter(|&len| len <= limits.max_path_len)
                .ok_or_else(|| fail(ErrorKind::LimitExceeded, offset, "link target too long"))?;
            let target = read(source, data, len).await?;
            index.insert(name, Node::Symlink(Box::from(target)), offset)
        },
        Kind::Special(kind) => index.insert(name, Node::Special(kind), offset),
    }
}

/// Reads the local header of `central`'s member, checks that it agrees and
/// that the member lies before the central directory, and returns the
/// offset of the member's data.
async fn check_local<S: ReadAt>(
    source: &S,
    central: &Central,
    cd_offset: u64,
) -> Result<u64, Error> {
    let offset = central.local_offset;
    let corrupt = |reason| fail(ErrorKind::Corrupt, offset, reason);
    if offset.saturating_add(wide(LOCAL_LEN)) > cd_offset {
        return Err(corrupt("local header past the central directory"));
    }
    let fixed = LocalFixed::parse(&read(source, offset, LOCAL_LEN).await?)
        .ok_or_else(|| corrupt("bad local header"))?;
    let variable_offset = offset.saturating_add(wide(LOCAL_LEN));
    let variable_len = fixed.variable_len();
    if variable_offset.saturating_add(wide(variable_len)) > cd_offset {
        return Err(corrupt("local header past the central directory"));
    }
    let variable = read(source, variable_offset, variable_len).await?;
    let (local, name) = fixed
        .complete(&variable)
        .ok_or_else(|| corrupt("bad local header"))?;
    if !local.agrees_with(name, central) {
        return Err(corrupt("local and central headers disagree"));
    }
    let data = variable_offset.saturating_add(wide(variable_len));
    match data.checked_add(central.compressed) {
        Some(data_end) if data_end <= cd_offset => Ok(data),
        _ => Err(corrupt("member extends past the central directory")),
    }
}

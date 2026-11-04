// This is free and unencumbered software released into the public domain.

//! Building the index of an archive: its central directory, each header
//! checked against its member's local header.

use crate::{
    codec::Method,
    header::{
        CENTRAL_LEN, Central, CentralFixed, EOCD_LEN, EOCD64_LEN, Eocd, Kind, LOCAL_LEN,
        LOCATOR_LEN, LocalFixed, MAX_COMMENT, find_eocd, parse_locator,
    },
};
use alloc::{boxed::Box, vec, vec::Vec};
use filer_api::{
    Error, ErrorKind, ReadAt,
    tree::{Node, Tree},
};
use filer_pack::{Codec, Index, Limits, Member, Parser, Reader};
use known_codepages::CodePage;

const ZIP: Parser = Parser::new("zip");

/// Converts a length that fits in memory to a `u64`.
fn wide(len: usize) -> u64 {
    u64::try_from(len).unwrap_or(u64::MAX)
}

/// Locates the central directory from the records at the end of the
/// archive, and returns it with the offset where it must end.
async fn locate<S: ReadAt>(source: &S, end: u64) -> Result<(Eocd, u64), Error> {
    let not_zip = || {
        ZIP.fail(
            ErrorKind::Corrupt,
            end,
            "no end of central directory record",
        )
    };
    let tail_len = end.min(wide(EOCD_LEN.saturating_add(MAX_COMMENT)));
    let tail_start = end.saturating_sub(tail_len);
    let tail = ZIP
        .read(source, tail_start, usize::try_from(tail_len).unwrap_or(0))
        .await?;
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
                return Err(ZIP.fail(ErrorKind::Unsupported, locator_offset, "spanned archives"));
            }
            let bad = || ZIP.fail(ErrorKind::Corrupt, offset, "bad ZIP64 end record");
            if offset.saturating_add(wide(EOCD64_LEN)) > locator_offset {
                return Err(bad());
            }
            let record = ZIP.read(source, offset, EOCD64_LEN).await?;
            let zip64 = Eocd::parse64(&record).ok_or_else(bad)?;
            if !eocd.agrees_with(&zip64) {
                return Err(bad());
            }
            (zip64, offset)
        },
    };
    if eocd.disk != 0 || eocd.cd_disk != 0 || eocd.disk_entries != eocd.entries {
        return Err(ZIP.fail(ErrorKind::Unsupported, eocd_offset, "spanned archives"));
    }
    if eocd.cd_offset.saturating_add(eocd.cd_size) > cd_end {
        let reason = "central directory past its end record";
        return Err(ZIP.fail(ErrorKind::Corrupt, eocd_offset, reason));
    }
    Ok((eocd, eocd.cd_offset.saturating_add(eocd.cd_size)))
}

/// Builds the index of the archive in `source`.
pub(crate) async fn index<S: ReadAt>(
    source: &S,
    limits: &Limits,
    page: CodePage,
) -> Result<Tree<Member<Method>>, Error> {
    let end = source.size().await?;
    let (eocd, cd_end) = locate(source, end).await?;
    let mut index = Index::new(ZIP, *limits);
    // The central directory's headers lie end to end, and the members'
    // local headers before it, mostly in the same order.
    let mut reader = Reader::new(source, ZIP, cd_end);
    let mut locals = Reader::new(source, ZIP, eocd.cd_offset);
    let mut pos = eocd.cd_offset;
    let mut total = 0_u64;
    // Each member's local header and data, with its central header.
    let mut spans = Vec::new();
    for _ in 0..eocd.entries {
        let offset = pos;
        let corrupt = |reason| ZIP.fail(ErrorKind::Corrupt, offset, reason);
        let fixed = CentralFixed::parse(reader.read(pos, CENTRAL_LEN).await?)
            .ok_or_else(|| corrupt("bad central directory header"))?;
        pos = pos.saturating_add(wide(CENTRAL_LEN));
        let variable_len = fixed.variable_len();
        let central = fixed
            .complete(reader.read(pos, variable_len).await?)
            .ok_or_else(|| corrupt("missing ZIP64 field"))?;
        pos = pos.saturating_add(wide(variable_len));
        // Decoding never passes the declared sizes, so bounding them
        // bounds the output.
        total = total.saturating_add(central.size);
        if total > limits.max_total_size {
            let reason = "members too large in total";
            return Err(ZIP.fail(ErrorKind::LimitExceeded, offset, reason));
        }
        if limits.exceeds_ratio(central.size, central.compressed) {
            let reason = "compression ratio too high";
            return Err(ZIP.fail(ErrorKind::LimitExceeded, offset, reason));
        }
        let span = add(source, &mut locals, &mut index, &central, page, offset).await?;
        spans.push((span, offset));
    }
    if pos != cd_end {
        let reason = "central directory size mismatch";
        return Err(ZIP.fail(ErrorKind::Corrupt, pos, reason));
    }
    // Members sharing bytes are how bombs that need no recursion get
    // their ratio, and no archiver writes them.
    spans.sort_unstable();
    for pair in spans.windows(2) {
        if let [((_, end), _), ((start, _), offset)] = pair
            && start < end
        {
            return Err(ZIP.fail(ErrorKind::Corrupt, *offset, "overlapping members"));
        }
    }
    Ok(index.into_tree())
}

/// Adds the member `central`, from the central directory header at
/// `offset`, after checking its local header with `locals`, and returns
/// where its local header starts and its data ends.
async fn add<S: ReadAt>(
    source: &S,
    locals: &mut Reader<'_, S>,
    index: &mut Index<Method>,
    central: &Central,
    page: CodePage,
    offset: u64,
) -> Result<(u64, u64), Error> {
    let name = central.decoded_name(page);
    let name = name.as_slice();
    // ZIP names use `/` alone; a backslash or drive letter would mean
    // something else to a Windows extractor.
    if name.contains(&b'\\') || matches!(name, [letter, b':', ..] if letter.is_ascii_alphabetic()) {
        return Err(ZIP.fail(ErrorKind::InvalidName, offset, "unsafe name"));
    }
    if central.disk != 0 {
        return Err(ZIP.fail(ErrorKind::Unsupported, offset, "spanned archives"));
    }
    let data = check_local(locals, central).await?;

    // Members this build cannot decode fail when read, not here.
    let codec = Method {
        method: central.fixed.method,
        crc: central.fixed.crc,
        size: central.size,
        encrypted: central.encrypted(),
    };
    if codec.is_stored() && central.size != central.compressed {
        let reason = "stored member sizes differ";
        return Err(ZIP.fail(ErrorKind::Corrupt, offset, reason));
    }
    match central.kind() {
        Kind::Dir => index.dir(name, offset),
        Kind::File => {
            let member = Member::encoded(data, central.compressed, central.size, codec)
                .with_modified(central.modified);
            index.insert(name, Node::File(member), offset)
        },
        Kind::Symlink => {
            let len = index.target_len(central.size, offset)?;
            let target = link_target(source, data, central, codec, offset, len).await?;
            index.insert(name, Node::Symlink(Box::from(target)), offset)
        },
        Kind::Special(kind) => index.insert(name, Node::Special(kind), offset),
    }?;
    Ok((
        central.local_offset,
        data.saturating_add(central.compressed),
    ))
}

/// Reads and decodes the target of the symlink `central`, whose data lies
/// at `data`, and which decodes to `len` bytes.
async fn link_target<S: ReadAt>(
    source: &S,
    data: u64,
    central: &Central,
    codec: Method,
    offset: u64,
    len: usize,
) -> Result<Vec<u8>, Error> {
    let too_long = || ZIP.fail(ErrorKind::LimitExceeded, offset, "link target too long");
    if codec.is_stored() {
        return ZIP.read(source, data, len).await;
    }
    // Any method that would pack a short target in much more than its own
    // length is not worth reading.
    let packed = usize::try_from(central.compressed)
        .ok()
        .filter(|&packed| packed <= len.saturating_add(64))
        .ok_or_else(too_long)?;
    let input = ZIP.read(source, data, packed).await?;
    let bad = |error: Error| ZIP.fail(error.kind(), offset, "bad link target");
    let mut decoder = codec.decoder().map_err(bad)?;
    let mut target = vec![0; len];
    let (mut used, mut filled) = (0, 0);
    while filled < len {
        let input = input.get(used..).unwrap_or_default();
        let output = target.get_mut(filled..).unwrap_or_default();
        let (consumed, produced) = decoder.decode(input, output).map_err(bad)?;
        if consumed == 0 && produced == 0 {
            return Err(ZIP.fail(ErrorKind::Corrupt, offset, "bad link target"));
        }
        used = used.saturating_add(consumed);
        filled = filled.saturating_add(produced);
    }
    decoder.finish().map_err(bad)?;
    Ok(target)
}

/// Reads the local header of `central`'s member with `locals`, which ends
/// at the central directory, checks that it agrees and that the member
/// lies before the central directory, and returns the offset of the
/// member's data.
async fn check_local<S: ReadAt>(
    locals: &mut Reader<'_, S>,
    central: &Central,
) -> Result<u64, Error> {
    let cd_offset = locals.end();
    let offset = central.local_offset;
    let corrupt = |reason| ZIP.fail(ErrorKind::Corrupt, offset, reason);
    if offset.saturating_add(wide(LOCAL_LEN)) > cd_offset {
        return Err(corrupt("local header past the central directory"));
    }
    let fixed = LocalFixed::parse(locals.read(offset, LOCAL_LEN).await?)
        .ok_or_else(|| corrupt("bad local header"))?;
    let variable_offset = offset.saturating_add(wide(LOCAL_LEN));
    let variable_len = fixed.variable_len();
    if variable_offset.saturating_add(wide(variable_len)) > cd_offset {
        return Err(corrupt("local header past the central directory"));
    }
    let variable = locals.read(variable_offset, variable_len).await?;
    let (local, name) = fixed
        .complete(variable)
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

// This is free and unencumbered software released into the public domain.

//! A compressed block's literals section (RFC 8878 §3.1.1.3.1): its
//! literals stored as they are, as one byte repeated, or Huffman-coded in
//! one stream or four.

use super::{corrupt, huffman::Huffman};
use crate::Error;
use alloc::vec::Vec;

/// Reads the literals section that `block` begins with, of at most `max`
/// literals, and returns them and the section's length: as they lie in
/// `block` if stored as they are, or else decoded into `buffer`. A Huffman
/// table it describes replaces `huffman`, which a section without one
/// reuses.
pub(super) fn read<'a>(
    block: &'a [u8],
    huffman: &mut Option<Huffman>,
    buffer: &'a mut Vec<u8>,
    max: usize,
) -> Result<(&'a [u8], usize), Error> {
    let &first = block.first().ok_or_else(corrupt)?;
    let (kind, format) = (first & 0x03, (first >> 2) & 0x03);
    // A little-endian header of up to five bytes, its sizes past the
    // type and format, in the lower four bits of the first.
    let field = |len: usize| -> Result<u64, Error> {
        let mut le = [0; 8];
        let header = block.get(..len).ok_or_else(corrupt)?;
        le.get_mut(..len)
            .ok_or_else(corrupt)?
            .copy_from_slice(header);
        Ok(u64::from_le_bytes(le) >> 4)
    };
    let size = |value: u64| usize::try_from(value).ok().filter(|&size| size <= max);

    // Stored or repeated: one size, of 5, 12, or 20 bits.
    if kind < 2 {
        let (header, regenerated) = match format {
            1 => (2, field(2)?),
            3 => (3, field(3)?),
            _ => (1, u64::from(first >> 3)),
        };
        let regenerated = size(regenerated).ok_or_else(corrupt)?;
        let rest = block.get(header..).unwrap_or_default();
        if kind == 0 {
            let literals = rest.get(..regenerated).ok_or_else(corrupt)?;
            return Ok((literals, header.saturating_add(regenerated)));
        }
        let &byte = rest.first().ok_or_else(corrupt)?;
        buffer.clear();
        buffer.resize(regenerated, byte);
        return Ok((buffer, header.saturating_add(1)));
    }

    // Huffman-coded: what they decode to and take, of 10, 14, or 18 bits
    // each, then the tree, unless the last one's serves, and the streams.
    let (header, width) = match format {
        0 | 1 => (3, 10),
        2 => (4, 14),
        _ => (5, 18),
    };
    let sizes = field(header)?;
    let mask = 1_u64.wrapping_shl(width).wrapping_sub(1);
    let regenerated = size(sizes & mask).ok_or_else(corrupt)?;
    let compressed = usize::try_from((sizes >> width) & mask).map_err(|_| corrupt())?;
    let end = header.saturating_add(compressed);
    let mut streams = block.get(header..end).ok_or_else(corrupt)?;
    if kind == 2 {
        let (table, used) = Huffman::read(streams)?;
        streams = streams.get(used..).ok_or_else(corrupt)?;
        *huffman = Some(table);
    }
    let huffman = huffman.as_ref().ok_or_else(corrupt)?;
    buffer.clear();
    buffer.resize(regenerated, 0);
    if format == 0 {
        huffman.decode(streams, buffer)?;
        return Ok((buffer, end));
    }

    // Four streams, the first three's lengths before them, each but the
    // last decoding to a quarter, rounded up.
    let (jump, mut rest) = streams.split_first_chunk::<6>().ok_or_else(corrupt)?;
    let (lens, _) = jump.as_chunks::<2>();
    let lens = lens.iter().map(|&len| Some(u16::from_le_bytes(len)));
    let quarter = regenerated.div_ceil(4);
    let mut outputs = buffer.chunks_mut(quarter.max(1));
    for len in lens.chain([None]) {
        let stream = match len {
            Some(len) => {
                let (stream, after) = rest
                    .split_at_checked(usize::from(len))
                    .ok_or_else(corrupt)?;
                rest = after;
                stream
            },
            None => rest,
        };
        let output = outputs.next().unwrap_or_default();
        // Only the last may decode to less, by at most three.
        if len.is_some() && output.len() < quarter {
            return Err(corrupt());
        }
        huffman.decode(stream, output)?;
    }
    Ok((buffer, end))
}

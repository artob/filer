// This is free and unencumbered software released into the public domain.

//! A compressed block's literals section (RFC 8878 §3.1.1.3.1): its
//! literals stored as they are, as one byte repeated, or Huffman-coded in
//! one stream or four.

use super::{
    corrupt,
    huffman::{Huffman, Runs},
};
use crate::Error;
use alloc::vec::Vec;

/// The fewest literals four streams may decode to, as zstd refuses fewer:
/// below it, a quarter each would leave the last streams none.
const MIN_FOUR_STREAMS: usize = 6;

/// The bytes kept past the literals, so that the last of them are copied
/// as the others are, more at once than they are, where a buffer holds
/// them.
const PAD: usize = 32;

/// Reads the literals section that `block` begins with, of at most `max`
/// literals, and returns them, with what bytes follow them, how many
/// they are, and the section's length: as they lie in `block` if stored
/// as they are, or else decoded into `buffer`. A Huffman table it
/// describes replaces `huffman`, which a section without one reuses.
pub(super) fn read<'a>(
    block: &'a [u8],
    huffman: &mut Option<Huffman>,
    runs: &mut Runs,
    buffer: &'a mut Vec<u8>,
    max: usize,
) -> Result<(&'a [u8], usize, usize), Error> {
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
            // With what follows them in the block, as far as it goes.
            if rest.len() < regenerated {
                return Err(corrupt());
            }
            let padded = rest.get(..regenerated.saturating_add(PAD)).unwrap_or(rest);
            return Ok((padded, regenerated, header.saturating_add(regenerated)));
        }
        let &byte = rest.first().ok_or_else(corrupt)?;
        ready(buffer, regenerated, max, byte);
        return Ok((buffer, regenerated, header.saturating_add(1)));
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
    if format != 0 && regenerated < MIN_FOUR_STREAMS {
        return Err(corrupt());
    }
    let compressed = usize::try_from((sizes >> width) & mask).map_err(|_| corrupt())?;
    let end = header.saturating_add(compressed);
    let mut streams = block.get(header..end).ok_or_else(corrupt)?;
    if kind == 2 {
        let (table, used) = Huffman::read(streams)?;
        streams = streams.get(used..).ok_or_else(corrupt)?;
        *huffman = Some(table);
        runs.clear();
    }
    let huffman = huffman.as_ref().ok_or_else(corrupt)?;
    let runs = runs.of(huffman, regenerated, streams.len());
    ready(buffer, regenerated, max, 0);
    let literals = buffer.get_mut(..regenerated).ok_or_else(corrupt)?;
    if format == 0 {
        huffman.decode(runs, streams, literals)?;
        return Ok((buffer, regenerated, end));
    }

    // Four streams, the first three's lengths before them, each but the
    // last decoding to a quarter, rounded up, and the last to the rest.
    let (jump, rest) = streams.split_first_chunk::<6>().ok_or_else(corrupt)?;
    let ([first, second, third], _) = jump.as_chunks::<2>() else {
        return Err(corrupt());
    };
    let len = |len: &[u8; 2]| usize::from(u16::from_le_bytes(*len));
    let lens = [len(first), len(second), len(third)];
    huffman.decode4(runs, rest, lens, literals)?;
    Ok((buffer, regenerated, end))
}

/// Fills `buffer` with `len` of `byte`, of at most `max`, and the bytes
/// kept past them. Short, it is freed, as its literals are not kept,
/// before one twice as large, up to `max` and those, is allocated:
/// growing it in place would hold both at once, and pass `max`.
fn ready(buffer: &mut Vec<u8>, len: usize, max: usize, byte: u8) {
    let (len, max) = (len.saturating_add(PAD), max.saturating_add(PAD));
    if buffer.capacity() < len {
        let grown = buffer.capacity().saturating_mul(2).min(max).max(len);
        *buffer = Vec::new();
        buffer.reserve_exact(grown);
    }
    buffer.clear();
    buffer.resize(len, byte);
}

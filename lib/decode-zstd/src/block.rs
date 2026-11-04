// This is free and unencumbered software released into the public domain.

//! Compressed blocks (RFC 8878 §3.1.1.3): literals, then the sequences
//! that interleave them with copies of what came before.

use super::{
    huffman::Huffman,
    literals,
    sequences::{self, Codes},
};
use crate::Error;
use alloc::vec::Vec;

/// What a frame's compressed blocks pass on, each to the next: the last
/// Huffman table, and their sequences' tables and offsets.
#[derive(Clone, Debug, Default)]
pub(super) struct Tables {
    huffman: Option<Huffman>,
    codes: Codes,
}

/// Decodes a compressed block's `content` onto `window`, of whose bytes it
/// may copy from `reach` back, decoding to at most `max` bytes, with
/// `scratch` to decode its literals into.
pub(super) fn decode(
    content: &[u8],
    tables: &mut Tables,
    scratch: &mut Vec<u8>,
    window: &mut Vec<u8>,
    reach: usize,
    max: usize,
) -> Result<(), Error> {
    let (literals, used) = literals::read(content, &mut tables.huffman, scratch, max)?;
    let section = content.get(used..).unwrap_or_default();
    window.reserve(max);
    sequences::decode(section, &mut tables.codes, literals, window, reach, max)
}

// This is free and unencumbered software released into the public domain.

//! Compressed blocks (RFC 8878 §3.1.1.3): literals, then the sequences
//! that interleave them with copies of what came before.

use super::{corrupt, huffman::Huffman, literals, unsupported};
use alloc::vec::Vec;
use filer_api::Error;

/// What a frame's compressed blocks pass on, each to the next: the last
/// Huffman table.
#[derive(Clone, Debug, Default)]
pub(super) struct Tables {
    huffman: Option<Huffman>,
}

/// Decodes a compressed block's `content` onto `window`, of at most `max`
/// bytes, with `scratch` to decode its literals into.
pub(super) fn decode(
    content: &[u8],
    tables: &mut Tables,
    scratch: &mut Vec<u8>,
    window: &mut Vec<u8>,
    max: usize,
) -> Result<(), Error> {
    let (literals, used) = literals::read(content, &mut tables.huffman, scratch, max)?;
    match content.get(used..).unwrap_or_default() {
        // No sequences: the literals are the block.
        [0] => {
            window.extend_from_slice(literals);
            Ok(())
        },
        [] | [0, ..] => Err(corrupt()),
        _ => Err(unsupported()),
    }
}

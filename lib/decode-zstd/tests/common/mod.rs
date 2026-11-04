// This is free and unencumbered software released into the public domain.

//! What the tests and fuzzers share: zstd frames built by hand, and a check
//! that the decoder decodes the same in pieces as whole.

// Each test binary compiles this module and uses only part of it.
#![allow(dead_code)]

use decode_zstd::{Decoder, Error};
// Named, for the crate's own tests, which are `no_std`.
use std::{format, vec, vec::Vec};

/// The most a check decodes of any input.
pub const CAP: usize = 1024 * 1024;

/// A zstd block built by hand.
#[derive(Clone, Copy)]
pub enum Block<'a> {
    /// Bytes as they are.
    Raw(&'a [u8]),
    /// A byte, repeated as many times.
    Rle(u8, usize),
}

/// Returns a zstd frame of `blocks`, with a 128 KiB window and neither
/// content size nor checksum.
pub fn zstd(blocks: &[Block<'_>]) -> Vec<u8> {
    let mut frame = vec![0x28, 0xB5, 0x2F, 0xFD, 0x00, 7 << 3];
    for (i, block) in blocks.iter().enumerate() {
        let last = u32::from(i + 1 == blocks.len());
        let (kind, size, content) = match *block {
            Block::Raw(bytes) => (0, bytes.len(), bytes.to_vec()),
            Block::Rle(byte, count) => (1, count, vec![byte]),
        };
        let header = (size as u32) << 3 | kind << 1 | last;
        frame.extend_from_slice(&header.to_le_bytes()[..3]);
        frame.extend_from_slice(&content);
    }
    frame
}

/// Returns a skippable zstd frame holding `bytes`.
pub fn skippable(bytes: &[u8]) -> Vec<u8> {
    let mut frame = vec![0x5A, 0x2A, 0x4D, 0x18];
    frame.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    frame.extend_from_slice(bytes);
    frame
}

/// What `tests/data/numbers.zst` holds: `seq 1 3000` from a file, then
/// `end\n` from a pipe, in two frames.
pub fn numbers() -> Vec<u8> {
    let mut numbers: Vec<u8> = (1..=3000)
        .flat_map(|n| format!("{n}\n").into_bytes())
        .collect();
    numbers.extend_from_slice(b"end\n");
    numbers
}

/// Decodes `input`, giving the decoder at most `piece` bytes of input and
/// room for as many of output at a time, up to `CAP` in all, and then
/// [`finish`](Decoder::finish)es it.
pub fn decode(input: &[u8], piece: usize) -> Result<Vec<u8>, Error> {
    let mut decoder = Decoder::new();
    let mut output = Vec::new();
    let mut buf = vec![0; piece];
    let mut at = 0;
    loop {
        let end = input.len().min(at + piece);
        let room = piece.min(CAP - output.len());
        let (consumed, produced) = decoder.decode(&input[at..end], &mut buf[..room])?;
        assert!(
            consumed <= end - at && produced <= room,
            "broke its contract"
        );
        at += consumed;
        output.extend_from_slice(&buf[..produced]);
        if consumed == 0 && produced == 0 {
            break;
        }
    }
    decoder.finish()?;
    Ok(output)
}

/// Decodes `input` whole and in pieces of `piece` bytes, checks that both
/// agree, and returns what it decoded.
///
/// # Panics
///
/// If they do not agree.
pub fn agree(input: &[u8], piece: usize) -> Result<Vec<u8>, Error> {
    let whole = decode(input, input.len().max(64 * 1024));
    let pieces = decode(input, piece);
    match (&whole, &pieces) {
        (Ok(whole), Ok(pieces)) => {
            assert!(whole == pieces, "decoded otherwise in pieces of {piece}")
        },
        (Err(whole), Err(pieces)) => assert_eq!(whole, pieces, "in pieces of {piece}"),
        _ => panic!("decoded whole: {whole:?}; in pieces of {piece}: {pieces:?}"),
    }
    whole
}

// This is free and unencumbered software released into the public domain.

//! What the tests and fuzzers share: zstd frames built by hand; a check
//! that the decoder decodes the same in pieces as whole; and another
//! decoder to check it against.

// Each test binary compiles this module and uses only part of it.
#![allow(dead_code)]

use core::ops::Range;
use decode_zstd::{Decoder, Error};
use ruzstd::decoding::{BlockDecodingStrategy, FrameDecoder};
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

/// The most a block decodes to, and the largest window this crate
/// decodes, as zstd does by default.
const BLOCK: u64 = 128 << 10;
const WINDOW: u64 = 128 << 20;

/// Returns whether `ours`, what this crate decodes `stream` to, or `None`
/// if it refuses it, is what `ruzstd` decodes it to, as zstd would
/// ([`ruzstd`]).
///
/// `ruzstd` also decodes, to the wrong bytes, a frame whose bitstreams
/// it does not read to their ends, such as Huffman-coded literals in four
/// streams whose jump table is off, which zstd and this crate refuse; only
/// a checksum tells. So this crate may refuse what `ruzstd` decodes if a
/// frame of it has none.
pub fn agrees_with_ruzstd(stream: &[u8], ours: Option<&[u8]>) -> bool {
    match (ours, ruzstd(stream)) {
        (ours, Some((theirs, _))) if ours == Some(&theirs[..]) => true,
        (None, None) | (None, Some((_, false))) => true,
        _ => false,
    }
}

/// Decodes `input`, frames one after another, through `ruzstd`, as zstd
/// would: what it decodes to, and whether each of its frames has a
/// checksum, or `None` if it refuses it. `ruzstd` lets pass a frame whose
/// content size disagrees with what it decodes to, and a block that
/// decodes to more than its frame allows, which zstd and this crate
/// refuse, so this does too; and what this crate finds unsupported, a
/// window over 128 MiB, and what passes `CAP`, where [`decode`] stops.
pub fn ruzstd(input: &[u8]) -> Option<(Vec<u8>, bool)> {
    // zstd refuses a stream of no frame at all.
    if input.is_empty() {
        return None;
    }
    let (mut output, mut checked, mut rest) = (Vec::new(), true, input);
    while !rest.is_empty() {
        let magic = u32::from_le_bytes(rest.get(..4)?.try_into().ok()?);
        if magic & !0x0F == 0x184D_2A50 {
            let len = u32::from_le_bytes(rest.get(4..8)?.try_into().ok()?);
            rest = rest.get(8 + len as usize..)?;
            continue;
        }
        let frame = Frame::parse(rest)?;
        output.extend(frame.decode(CAP - output.len())?);
        checked &= frame.descriptor & 0x04 != 0;
        rest = &rest[frame.bytes.len()..];
    }
    Some((output, checked))
}

/// A zstd frame's layout, as far as checking what `ruzstd` decodes it to
/// needs, read from RFC 8878.
struct Frame<'a> {
    /// The whole frame, from its magic number to its checksum, if any.
    bytes: &'a [u8],
    /// Its frame header's descriptor, and where its dictionary ID is.
    descriptor: u8,
    dictionary: Range<usize>,
    /// Its window size, its content size, if it names one, and the most
    /// any of its blocks may decode to.
    window: u64,
    size: Option<u64>,
    max: u64,
    /// Each block, from its header, and what it decodes to, unless it is
    /// compressed.
    blocks: Vec<(Range<usize>, Option<u64>)>,
}

impl<'a> Frame<'a> {
    /// Reads the frame that `stream` begins with, or `None` if it is
    /// malformed, cut short, or unsupported.
    fn parse(stream: &'a [u8]) -> Option<Self> {
        if stream.get(..4)? != [0x28, 0xB5, 0x2F, 0xFD] {
            return None;
        }
        let descriptor = *stream.get(4)?;
        let single = descriptor & 0x20 != 0;
        // The reserved bit.
        if descriptor & 0x08 != 0 {
            return None;
        }
        let at = 5 + usize::from(!single);
        let dictionary = at..at + [0, 1, 2, 4][usize::from(descriptor & 3)];
        let header = dictionary.end + [usize::from(single), 2, 4, 8][usize::from(descriptor >> 6)];
        let field = stream.get(dictionary.end..header)?;
        let size = field
            .iter()
            .rev()
            .fold(0, |n, &byte| n << 8 | u64::from(byte));
        let size = match field.len() {
            0 => None,
            2 => Some(size + 256),
            _ => Some(size),
        };
        let window = match size {
            Some(size) if single => size,
            _ => {
                let byte = *stream.get(5)?;
                let base = 1 << (10 + (byte >> 3));
                base + base / 8 * u64::from(byte & 7)
            },
        };
        if window > WINDOW {
            return None;
        }
        let max = window.min(BLOCK);
        let (mut at, mut blocks) = (header, Vec::new());
        loop {
            let head = stream.get(at..at + 3)?;
            let head = u32::from_le_bytes([head[0], head[1], head[2], 0]);
            let (kind, size) = ((head >> 1) & 3, u64::from(head >> 3));
            if kind == 3 || size > max {
                return None;
            }
            let end = at + 3 + if kind == 1 { 1 } else { size as usize };
            stream.get(at..end)?;
            blocks.push((at..end, (kind != 2).then_some(size)));
            at = end;
            if head & 1 != 0 {
                break;
            }
        }
        let end = at + if descriptor & 0x04 != 0 { 4 } else { 0 };
        Some(Self {
            bytes: stream.get(..end)?,
            descriptor,
            dictionary,
            window,
            size,
            max,
            blocks,
        })
    }

    /// Decodes the frame through `ruzstd`, into at most `cap` bytes, and
    /// checks what `ruzstd` does not.
    fn decode(&self, cap: usize) -> Option<Vec<u8>> {
        let output = ruzstd_frame(self.bytes, self.blocks.len(), cap)?;
        if self.size.is_some_and(|size| size != output.len() as u64) {
            return None;
        }
        // No compressed block decodes to more than the frame allows, if
        // all of them together do not; else each is measured by decoding
        // the frame up to it.
        if output.len() as u64 > self.max {
            let mut before = 0;
            for (i, (_, size)) in self.blocks.iter().enumerate() {
                let after = match size {
                    Some(size) => before + size,
                    None => ruzstd_frame(&self.through(i), i + 1, usize::MAX)?.len() as u64,
                };
                if after - before > self.max {
                    return None;
                }
                before = after;
            }
        }
        Some(output)
    }

    /// Returns the frame cut short after its `last` block, as the last:
    /// without its content size or checksum, and so with a window of its
    /// own, at least as large.
    fn through(&self, last: usize) -> Vec<u8> {
        let exponent = (0..=17)
            .find(|e| 1 << (10 + e) >= self.window)
            .unwrap_or(17);
        let mut frame = vec![0x28, 0xB5, 0x2F, 0xFD, self.descriptor & 3, exponent << 3];
        frame.extend_from_slice(&self.bytes[self.dictionary.clone()]);
        let start = self.blocks[0].0.start;
        let block = frame.len() + self.blocks[last].0.start - start;
        frame.extend_from_slice(&self.bytes[start..self.blocks[last].0.end]);
        frame[block] |= 1;
        frame
    }
}

/// Decodes `frame`, of `blocks` blocks, through `ruzstd`, into at most
/// `cap` bytes, checking its checksum, if any.
fn ruzstd_frame(mut frame: &[u8], blocks: usize, cap: usize) -> Option<Vec<u8>> {
    let mut decoder = FrameDecoder::new();
    decoder.set_max_window_size(WINDOW);
    decoder.reset(&mut frame).ok()?;
    let mut output = Vec::new();
    for _ in 0..blocks {
        let strategy = BlockDecodingStrategy::UptoBlocks(1);
        decoder.decode_blocks(&mut frame, strategy).ok()?;
        decoder.collect_to_writer(&mut output).ok()?;
        if output.len() > cap {
            return None;
        }
    }
    let checksum = decoder.get_checksum_from_data();
    let sound = checksum.is_none() || checksum == decoder.get_calculated_checksum();
    (decoder.is_finished() && sound && frame.is_empty()).then_some(output)
}

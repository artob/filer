// This is free and unencumbered software released into the public domain.

//! What the decoders' tests and fuzzers share: gzip members built by hand,
//! and a check that a decoder decodes the same in pieces as whole.

// Each test binary compiles this module and uses only part of it.
#![allow(dead_code)]

use core::sync::atomic::{AtomicU64, Ordering};
use filer_api::{BufResult, Error, ReadAt, checksum::crc32};
use filer_pack::{Codec, Decoder, Unpacked};
// Named, for `filer-pack`'s own tests, which are `no_std`.
use std::{boxed::Box, format, vec, vec::Vec};

/// The most a check decodes of any input.
pub const CAP: usize = 1024 * 1024;

/// A gzip member's optional header fields.
#[derive(Clone, Copy, Default)]
pub struct Member<'a> {
    pub extra: Option<&'a [u8]>,
    pub name: Option<&'a [u8]>,
    pub comment: Option<&'a [u8]>,
    pub header_crc: bool,
}

impl Member<'_> {
    /// Returns the member's header.
    pub fn header(&self) -> Vec<u8> {
        let flags = u8::from(self.header_crc) << 1
            | u8::from(self.extra.is_some()) << 2
            | u8::from(self.name.is_some()) << 3
            | u8::from(self.comment.is_some()) << 4;
        // No time, no hints, and an unknown OS.
        let mut header = vec![0x1F, 0x8B, 8, flags, 0, 0, 0, 0, 0, 0xFF];
        if let Some(extra) = self.extra {
            header.extend_from_slice(&(extra.len() as u16).to_le_bytes());
            header.extend_from_slice(extra);
        }
        for text in [self.name, self.comment].into_iter().flatten() {
            header.extend_from_slice(text);
            header.push(0);
        }
        if self.header_crc {
            let crc = crc32(&header) as u16;
            header.extend_from_slice(&crc.to_le_bytes());
        }
        header
    }

    /// Returns the member holding `data`, deflated.
    pub fn gzip(&self, data: &[u8]) -> Vec<u8> {
        let mut member = self.header();
        member.extend_from_slice(&miniz_oxide::deflate::compress_to_vec(data, 6));
        member.extend_from_slice(&crc32(data).to_le_bytes());
        member.extend_from_slice(&(data.len() as u32).to_le_bytes());
        member
    }
}

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

/// What `tests/data/numbers.*` hold: `seq 1 3000` from a file, then
/// `end\n` from a pipe, in two members or frames.
pub fn numbers() -> Vec<u8> {
    let mut numbers: Vec<u8> = (1..=3000)
        .flat_map(|n| format!("{n}\n").into_bytes())
        .collect();
    numbers.extend_from_slice(b"end\n");
    numbers
}

/// The bytes that `Counting` decoders produced, across a test binary.
pub static PRODUCED: AtomicU64 = AtomicU64::new(0);

/// A decoder counting in `PRODUCED` what `D` produces, and passing its
/// checkpoints on.
pub struct Counting<D>(pub D);

impl<D: Decoder + 'static> Decoder for Counting<D> {
    fn decode(&mut self, input: &[u8], output: &mut [u8]) -> Result<(usize, usize), Error> {
        let (consumed, produced) = self.0.decode(input, output)?;
        PRODUCED.fetch_add(produced as u64, Ordering::Relaxed);
        Ok((consumed, produced))
    }

    fn finish(&mut self) -> Result<(), Error> {
        self.0.finish()
    }

    fn checkpoint(&self) -> Option<Box<dyn Decoder>> {
        Some(Box::new(Counting(self.0.checkpoint()?)))
    }

    fn buffered(&self) -> usize {
        self.0.buffered()
    }
}

/// A codec decoding with what its function returns.
#[derive(Clone, Copy)]
pub struct With(pub fn() -> Box<dyn Decoder>);

impl Codec for With {
    fn is_stored(&self) -> bool {
        false
    }

    fn decoder(&self) -> Result<Box<dyn Decoder>, Error> {
        Ok((self.0)())
    }
}

/// Reads the `size` bytes that `stream` decodes to with `codec`, through
/// `Unpacked`, which checks what follows them.
pub fn unpack(stream: Vec<u8>, size: usize, codec: With) -> Result<Vec<u8>, Error> {
    let len = stream.len() as u64;
    let unpacked = Unpacked::new(stream, 0..len, size as u64, codec);
    let read = unpacked.read_exact_at(Vec::with_capacity(size), 0);
    let BufResult(result, data) = filer_spec::block_on(read);
    result.map(|()| data)
}

/// Decodes `input` with `decoder`, giving it at most `piece` bytes of
/// input and room for as many of output at a time, up to `CAP` in all, and
/// then [`finish`](Decoder::finish)es it.
pub fn decode(decoder: &mut dyn Decoder, input: &[u8], piece: usize) -> Result<Vec<u8>, Error> {
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

/// Decodes `input` with a decoder from `new`, whole and in pieces of
/// `piece` bytes, checks that both agree, and returns what it decoded.
///
/// # Panics
///
/// If they do not agree.
pub fn agree<D: Decoder>(
    new: impl Fn() -> D,
    input: &[u8],
    piece: usize,
) -> Result<Vec<u8>, Error> {
    let whole = decode(&mut new(), input, input.len().max(64 * 1024));
    let pieces = decode(&mut new(), input, piece);
    match (&whole, &pieces) {
        (Ok(whole), Ok(pieces)) => {
            assert!(whole == pieces, "decoded otherwise in pieces of {piece}")
        },
        (Err(whole), Err(pieces)) => {
            assert_eq!(whole.kind(), pieces.kind(), "in pieces of {piece}")
        },
        _ => panic!("decoded whole: {whole:?}; in pieces of {piece}: {pieces:?}"),
    }
    whole
}

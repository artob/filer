// This is free and unencumbered software released into the public domain.

//! What the decoders' tests and fuzzers share: gzip members built by hand,
//! and a check that a decoder decodes the same in pieces as whole.

// Each test binary compiles this module and uses only part of it.
#![allow(dead_code)]

use filer_api::{Error, checksum::crc32};
use filer_pack::Decoder;

/// The most a check decodes of any input.
pub const CAP: usize = 256 * 1024;

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

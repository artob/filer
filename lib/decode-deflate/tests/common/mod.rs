// This is free and unencumbered software released into the public domain.

//! What the tests and fuzzers share: a check that the decoder decodes the
//! same in pieces as whole, consuming the stream to its end and no more,
//! and another decoder to check it against.

// Each test binary compiles this module and uses only part of it.
#![allow(dead_code)]

use decode_deflate::{Decoder, Error};

/// The most a check decodes of any input.
pub const CAP: usize = 1024 * 1024;

/// What a stream decodes to, and how much of it the decoder consumed.
pub type Decoded = Result<(Vec<u8>, usize), Error>;

/// Decodes `input`, giving the decoder at most `piece` bytes of input and
/// room for as many of output at a time, up to `CAP` in all, and then
/// [`finish`](Decoder::finish)es it.
pub fn decode(input: &[u8], piece: usize) -> Decoded {
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
    Ok((output, at))
}

/// Decodes `input` whole and in pieces of `piece` bytes, checks that both
/// agree, and returns what it decoded and consumed.
///
/// # Panics
///
/// If they do not agree.
pub fn agree(input: &[u8], piece: usize) -> Decoded {
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

/// Decodes `input` with zlib's semantics, through `zlib-rs`, a port of
/// it: what it decodes to and consumes, or `None` if it refuses it.
pub fn zlib(input: &[u8]) -> Option<(Vec<u8>, usize)> {
    use zlib_rs::{Inflate, InflateFlush, Status};
    let mut inflate = Inflate::new(false, 15);
    let mut output = vec![0; CAP];
    match inflate.decompress(input, &mut output, InflateFlush::Finish) {
        Ok(Status::StreamEnd) => {
            output.truncate(inflate.total_out() as usize);
            Some((output, inflate.total_in() as usize))
        },
        _ => None,
    }
}

/// The CRC-32 that zlib computes.
pub fn crc32(bytes: &[u8]) -> u32 {
    !bytes.iter().fold(!0_u32, |crc, &byte| {
        (0..8).fold(crc ^ u32::from(byte), |crc, _| {
            (crc >> 1) ^ (0xEDB8_8320 & (crc & 1).wrapping_neg())
        })
    })
}

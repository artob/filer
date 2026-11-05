// This is free and unencumbered software released into the public domain.

//! What the tests and fuzzers share: a check that the decoder decodes the
//! same in pieces as whole, consuming the stream to its end and no more;
//! and another decoder to check it against.

// Each test binary compiles this module and uses only part of it.
#![allow(dead_code)]

use decode_zlib::{Decoder, Error};

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

/// Decodes `input` whole into room for exactly `len` bytes, what it
/// decodes to, and then [`finish`](Decoder::finish)es it: a stream must end
/// though its last byte fills the room, and what follows, such as a check
/// or a count of none more, is still to be read.
pub fn exactly(input: &[u8], len: usize) -> Decoded {
    let (mut decoder, mut output) = (Decoder::new(), vec![0; len]);
    let (mut at, mut written) = (0, 0);
    loop {
        let (consumed, produced) = decoder.decode(&input[at..], &mut output[written..])?;
        (at, written) = (at + consumed, written + produced);
        if (consumed, produced) == (0, 0) {
            break;
        }
    }
    decoder.finish()?;
    output.truncate(written);
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
    if let Ok((data, _)) = &whole {
        let exact = exactly(input, data.len());
        assert!(
            exact.as_ref() == whole.as_ref(),
            "decoded otherwise into room for exactly {} bytes: {:?}",
            data.len(),
            exact.as_ref().map(|_| ()),
        );
    }
    whole
}

/// Decodes `input` as zlib does, through `zlib-rs`, a port of it: what it
/// decodes to and consumes, or `None` if it refuses it.
pub fn zlib(input: &[u8]) -> Option<(Vec<u8>, usize)> {
    use zlib_rs::{Inflate, InflateFlush, Status};
    let mut inflate = Inflate::new(true, 15);
    let mut output = vec![0; CAP];
    match inflate.decompress(input, &mut output, InflateFlush::Finish) {
        Ok(Status::StreamEnd) => {
            output.truncate(inflate.total_out() as usize);
            Some((output, inflate.total_in() as usize))
        },
        _ => None,
    }
}

/// The Adler-32 that zlib computes, as RFC 1950 defines it.
pub fn adler32(bytes: &[u8]) -> u32 {
    let (a, b) = bytes.iter().fold((1_u32, 0_u32), |(a, b), &byte| {
        let a = (a + u32::from(byte)) % 65_521;
        (a, (b + a) % 65_521)
    });
    b << 16 | a
}

/// Returns the zlib stream of `data`, deflated at `level`.
pub fn zlib_at(data: &[u8], level: u8) -> Vec<u8> {
    miniz_oxide::deflate::compress_to_vec_zlib(data, level)
}

/// Returns `stream` with its header's first byte `cmf`, and its check
/// made to hold again.
pub fn with_cmf(stream: &[u8], cmf: u8) -> Vec<u8> {
    let mut stream = stream.to_vec();
    stream[0] = cmf;
    let flg = stream[1] & 0xE0;
    let check = (31 - (u16::from(cmf) << 8 | u16::from(flg)) % 31) % 31;
    stream[1] = flg | check as u8;
    stream
}

/// What `numbers.zz` and `window.zz` hold: `seq 1 3000`.
pub fn numbers() -> Vec<u8> {
    (1..=3000)
        .flat_map(|n| format!("{n}\n").into_bytes())
        .collect()
}

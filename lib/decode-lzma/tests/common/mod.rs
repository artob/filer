// This is free and unencumbered software released into the public domain.

//! What the tests and fuzzers share: a check that the decoder decodes the
//! same in pieces as whole, consuming the stream to its end and no more;
//! another decoder to check it against, `lzma-rust2`'s; and streams that
//! its encoder writes.

// Each test binary compiles this module and uses only part of it.
#![allow(dead_code)]

use decode_lzma::{Decoder, Error};
use lzma_rust2::{
    Action, Lzma2Options, Lzma2Stream, Lzma2Writer, LzmaOptions, LzmaStream, LzmaWriter, Status,
};
use std::io::Write;

/// The most a check decodes of any input.
pub const CAP: usize = 4 * 1024 * 1024;

/// What a stream decodes to, and how much of it the decoder consumed.
pub type Decoded = Result<(Vec<u8>, usize), Error>;

/// Decodes `input` with a decoder from `new`, giving it at most `piece`
/// bytes of input and room for as many of output at a time, up to `CAP`
/// in all, and then [`finish`](Decoder::finish)es it.
pub fn decode(new: impl Fn() -> Result<Decoder, Error>, input: &[u8], piece: usize) -> Decoded {
    let mut decoder = new()?;
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
pub fn exactly(new: impl Fn() -> Result<Decoder, Error>, input: &[u8], len: usize) -> Decoded {
    let (mut decoder, mut output) = (new()?, vec![0; len]);
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
pub fn agree(new: impl Fn() -> Result<Decoder, Error>, input: &[u8], piece: usize) -> Decoded {
    let whole = decode(&new, input, input.len().max(64 * 1024));
    let pieces = decode(&new, input, piece);
    match (&whole, &pieces) {
        (Ok(whole), Ok(pieces)) => {
            assert!(whole == pieces, "decoded otherwise in pieces of {piece}")
        },
        (Err(whole), Err(pieces)) => assert_eq!(whole, pieces, "in pieces of {piece}"),
        _ => panic!("decoded whole: {whole:?}; in pieces of {piece}: {pieces:?}"),
    }
    if let Ok((data, _)) = &whole {
        let exact = exactly(&new, input, data.len());
        assert!(
            exact.as_ref() == whole.as_ref(),
            "decoded otherwise into room for exactly {} bytes: {:?}",
            data.len(),
            exact.as_ref().map(|_| ()),
        );
    }
    whole
}

/// An LZMA stream, without a header, and its properties as 7z stores
/// them.
pub struct Lzma {
    pub stream: Vec<u8>,
    pub properties: [u8; 5],
}

/// Returns `data` as LZMA by `lzma-rust2`'s encoder, with `options`, and
/// an end marker or not.
pub fn lzma(data: &[u8], options: &LzmaOptions, marker: bool) -> Lzma {
    let mut writer = LzmaWriter::new_no_header(Vec::new(), options, marker).unwrap();
    let props = writer.props();
    writer.write_all(data).unwrap();
    let stream = writer.finish().unwrap();
    let mut properties = [props, 0, 0, 0, 0];
    properties[1..].copy_from_slice(&options.dict_size.to_le_bytes());
    Lzma { stream, properties }
}

/// Returns `data` as LZMA2 by `lzma-rust2`'s encoder, with `options`, and
/// the property of its dictionary's size.
pub fn lzma2(data: &[u8], options: Lzma2Options) -> (Vec<u8>, u8) {
    let dictionary = options.lzma_options.dict_size;
    let mut writer = Lzma2Writer::new(Vec::new(), options);
    writer.write_all(data).unwrap();
    let property = (0..40)
        .find(|&property| (2 | (property & 1) as u32) << (property / 2 + 11) >= dictionary)
        .unwrap();
    (writer.finish().unwrap(), property)
}

/// Decodes `input` through `lzma-rust2`'s stream: what it decodes to and
/// consumes, or `None` if it refuses it.
fn theirs(
    mut process: impl FnMut(&[u8], &mut [u8], Action) -> Option<(usize, usize, bool)>,
    input: &[u8],
    cap: usize,
) -> Option<(Vec<u8>, usize)> {
    let (mut output, mut buf, mut at) = (Vec::new(), vec![0; 64 * 1024], 0);
    loop {
        let rest = &input[at..];
        let action = if rest.is_empty() {
            Action::Finish
        } else {
            Action::Run
        };
        let (consumed, produced, ended) = process(rest, &mut buf, action)?;
        at += consumed;
        output.extend_from_slice(&buf[..produced]);
        if ended {
            return Some((output, at));
        }
        if (consumed, produced) == (0, 0) || output.len() > cap {
            return None;
        }
    }
}

/// Decodes LZMA `input` with `properties`, of `size` if known, through
/// `lzma-rust2`, up to `CAP`.
pub fn theirs_lzma(
    input: &[u8],
    properties: [u8; 5],
    size: Option<u64>,
) -> Option<(Vec<u8>, usize)> {
    theirs_lzma_up_to(input, properties, size, CAP)
}

/// Decodes LZMA `input` as `theirs_lzma` does, up to `cap`.
pub fn theirs_lzma_up_to(
    input: &[u8],
    properties: [u8; 5],
    size: Option<u64>,
    cap: usize,
) -> Option<(Vec<u8>, usize)> {
    let [props, dictionary @ ..] = properties;
    let size = size.unwrap_or(u64::MAX);
    let dictionary = u32::from_le_bytes(dictionary);
    let mut stream = LzmaStream::new_with_props(size, props, dictionary, None).ok()?;
    theirs(
        |input, output, action| {
            let result = stream.process(input, output, action).ok()?;
            let ended = result.status == Status::StreamEnd;
            Some((result.bytes_consumed, result.bytes_produced, ended))
        },
        input,
        cap,
    )
}

/// Decodes LZMA2 `input` with the dictionary of `property` through
/// `lzma-rust2`.
pub fn theirs_lzma2(input: &[u8], property: u8) -> Option<(Vec<u8>, usize)> {
    let dictionary = (2 | u32::from(property & 1)) << (property / 2 + 11);
    let mut stream = Lzma2Stream::new(dictionary);
    theirs(
        |input, output, action| {
            let result = stream.process(input, output, action).ok()?;
            let ended = result.status == Status::StreamEnd;
            Some((result.bytes_consumed, result.bytes_produced, ended))
        },
        input,
        CAP,
    )
}

/// Text that compresses as text does: words, from a small vocabulary, in
/// lines.
pub fn text(len: usize, seed: u64) -> Vec<u8> {
    const WORDS: [&str; 16] = [
        "the",
        "archive",
        "decoder",
        "of",
        "a",
        "stream",
        "in",
        "dictionary",
        "match",
        "literal",
        "range",
        "and",
        "to",
        "chunk",
        "public",
        "domain",
    ];
    let mut state = seed | 1;
    let mut text = Vec::with_capacity(len + 16);
    while text.len() < len {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        text.extend_from_slice(WORDS[(state % 16) as usize].as_bytes());
        text.push(if state.is_multiple_of(11) {
            b'\n'
        } else {
            b' '
        });
    }
    text.truncate(len);
    text
}

/// Bytes that do not compress.
pub fn noise(len: usize, seed: u64) -> Vec<u8> {
    let mut state = seed | 1;
    (0..len)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state as u8
        })
        .collect()
}

/// What `numbers.lzma` and `numbers.lzma2` hold.
pub fn numbers() -> Vec<u8> {
    let mut numbers: Vec<u8> = (1..=3000)
        .flat_map(|n| format!("{n}\n").into_bytes())
        .collect();
    numbers.extend_from_slice(b"end\n");
    numbers
}

/// Returns options for a preset with these properties and dictionary.
pub fn options(preset: u32, lc: u32, lp: u32, pb: u32, dictionary: u32) -> LzmaOptions {
    let mut options = LzmaOptions::with_preset(preset);
    (options.lc, options.lp, options.pb, options.dict_size) = (lc, lp, pb, dictionary);
    options
}

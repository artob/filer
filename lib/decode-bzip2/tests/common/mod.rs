// This is free and unencumbered software released into the public domain.

//! What the tests and fuzzers share: a check that the decoder decodes the
//! same in pieces as whole, and another decoder to check it against.

// Each test binary compiles this module and uses only part of it.
#![allow(dead_code)]

use decode_bzip2::{Decoder, Error};

/// The most a check decodes of any input.
pub const CAP: usize = 1024 * 1024;

/// What `tests/data/numbers.bz2` holds: `seq 1 3000` from a file, then
/// `end\n` from a pipe, in two streams.
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

/// Decodes `input` whole into room for exactly `len` bytes, what it
/// decodes to, and then [`finish`](Decoder::finish)es it: a stream must end
/// though its last byte fills the room, and what follows, such as a check
/// or a count of none more, is still to be read.
pub fn exactly(input: &[u8], len: usize) -> Result<Vec<u8>, Error> {
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
    if let Ok(data) = &whole {
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

/// Returns whether `ours`, what this crate decodes `stream` to, is what
/// [`bzip2`] decodes it to, or refuses it as `bzip2` does. `bzip2` decodes
/// randomized blocks, which this crate finds unsupported.
pub fn agrees_with_bzip2(stream: &[u8], ours: &Result<Vec<u8>, Error>) -> bool {
    match ours {
        Ok(ours) => bzip2(stream).as_ref() == Some(ours),
        Err(Error::Corrupt) => bzip2(stream).is_none(),
        Err(_) => true,
    }
}

/// Decodes `input`, streams one after another, through `bzip2` over its
/// pure-Rust `libbz2-rs-sys`, which is bzip2's own decoder ported: what
/// it decodes to, or `None` if it refuses it. After the first stream,
/// what does not begin with a stream's magic number, `BZh` and a block
/// size, ends the input, as bzip2 ignores trailing garbage. `None` too
/// past `CAP`, where [`decode`] stops.
pub fn bzip2(input: &[u8]) -> Option<Vec<u8>> {
    let (mut output, mut rest) = (Vec::new(), input);
    let mut buf = vec![0; 64 << 10];
    loop {
        let mut decoder = bzip2::Decompress::new(false);
        loop {
            let (read, written) = (decoder.total_in(), decoder.total_out());
            let at = read as usize;
            let status = decoder.decompress(&rest[at..], &mut buf).ok()?;
            let produced = (decoder.total_out() - written) as usize;
            output.extend_from_slice(&buf[..produced]);
            if output.len() > CAP {
                return None;
            }
            if status == bzip2::Status::StreamEnd {
                break;
            }
            // Cut short.
            if decoder.total_in() == read && produced == 0 {
                return None;
            }
        }
        rest = &rest[decoder.total_in() as usize..];
        match rest {
            [b'B', b'Z', b'h', b'1'..=b'9', ..] => {},
            _ => return Some(output),
        }
    }
}

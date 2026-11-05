// This is free and unencumbered software released into the public domain.

//! What the tests and fuzzers share: a check that the decoder decodes the
//! same in pieces as whole, consuming the stream to its end and no more;
//! and another decoder to check it against, `lzma-rust2`'s.

// Each test binary compiles this module and uses only part of it.
#![allow(dead_code)]

use decode_xz::{Decoder, Error};
use lzma_rust2::{Action, Status, XzStream};

/// The most a check decodes of any input.
pub const CAP: usize = 4 * 1024 * 1024;

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

/// Decodes `input` through `lzma-rust2`'s xz stream: what it decodes to,
/// or `None` if it refuses it.
pub fn theirs(input: &[u8]) -> Option<Vec<u8>> {
    let mut stream = XzStream::new_mem_limit(true, u32::MAX);
    let (mut output, mut buf, mut at) = (Vec::new(), vec![0; 64 * 1024], 0);
    loop {
        let rest = &input[at..];
        let action = if rest.is_empty() {
            Action::Finish
        } else {
            Action::Run
        };
        let result = stream.process(rest, &mut buf, action).ok()?;
        at += result.bytes_consumed;
        output.extend_from_slice(&buf[..result.bytes_produced]);
        if result.status == Status::StreamEnd {
            return Some(output);
        }
        let stalled = (result.bytes_consumed, result.bytes_produced) == (0, 0);
        if stalled || output.len() > CAP {
            return None;
        }
    }
}

/// The CRC-32 that xz computes.
pub fn crc32(bytes: &[u8]) -> u32 {
    !bytes.iter().fold(!0_u32, |crc, &byte| {
        (0..8).fold(crc ^ u32::from(byte), |crc, _| {
            (crc >> 1) ^ (0xEDB8_8320 & (crc & 1).wrapping_neg())
        })
    })
}

/// What `numbers.xz` and `blocks.xz` hold: `seq 1 n`.
pub fn seq(n: u32) -> Vec<u8> {
    (1..=n)
        .flat_map(|n| format!("{n}\n").into_bytes())
        .collect()
}

/// What `numbers.xz` holds: `seq 1 3000`, then `end\n`, in two streams.
pub fn numbers() -> Vec<u8> {
    let mut numbers = seq(3000);
    numbers.extend_from_slice(b"end\n");
    numbers
}

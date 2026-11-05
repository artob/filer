// This is free and unencumbered software released into the public domain.

//! What the tests and fuzzers share: a check that the decoder decodes the
//! same in pieces as whole, consuming the stream to its end and no more;
//! another decoder to check it against; and members built by hand.

// Each test binary compiles this module and uses only part of it.
#![allow(dead_code)]

use decode_gzip::{Decoder, Error};

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

/// Decodes `input` member by member with zlib's semantics, through
/// `zlib-rs`, a port of it, while what follows a member starts as one
/// would: what it decodes to and consumes, or `None` if it refuses it.
pub fn zlib(input: &[u8]) -> Option<(Vec<u8>, usize)> {
    use zlib_rs::{Inflate, InflateFlush, Status};
    let (mut output, mut at) = (Vec::new(), 0);
    loop {
        let rest = &input[at..];
        if at > 0 && rest.first() != Some(&0x1F) {
            return Some((output, at));
        }
        let mut inflate = Inflate::new(true, 31);
        let mut member = vec![0; CAP - output.len()];
        match inflate.decompress(rest, &mut member, InflateFlush::Finish) {
            Ok(Status::StreamEnd) => {
                output.extend_from_slice(&member[..inflate.total_out() as usize]);
                at += inflate.total_in() as usize;
            },
            _ => return None,
        }
    }
}

/// The CRC-32 that gzip computes.
pub fn crc32(bytes: &[u8]) -> u32 {
    !bytes.iter().fold(!0_u32, |crc, &byte| {
        (0..8).fold(crc ^ u32::from(byte), |crc, _| {
            (crc >> 1) ^ (0xEDB8_8320 & (crc & 1).wrapping_neg())
        })
    })
}

/// A gzip member's optional header fields.
#[derive(Clone, Copy, Debug, Default)]
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

    /// Returns the member holding `data`, deflated at `level`.
    pub fn gzip_at(&self, data: &[u8], level: u8) -> Vec<u8> {
        let mut member = self.header();
        member.extend_from_slice(&miniz_oxide::deflate::compress_to_vec(data, level));
        member.extend_from_slice(&crc32(data).to_le_bytes());
        member.extend_from_slice(&(data.len() as u32).to_le_bytes());
        member
    }

    /// Returns the member holding `data`, deflated.
    pub fn gzip(&self, data: &[u8]) -> Vec<u8> {
        self.gzip_at(data, 6)
    }
}

/// What `numbers.gz` holds: `seq 1 3000` from a file, then `end\n` from a
/// pipe, in two members.
pub fn numbers() -> Vec<u8> {
    let mut numbers: Vec<u8> = (1..=3000)
        .flat_map(|n| format!("{n}\n").into_bytes())
        .collect();
    numbers.extend_from_slice(b"end\n");
    numbers
}

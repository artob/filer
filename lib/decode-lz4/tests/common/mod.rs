// This is free and unencumbered software released into the public domain.

//! What the tests and fuzzers share: a check that the decoder decodes the
//! same in pieces as whole; another decoder to check it against; and
//! frames built by hand.

// Each test binary compiles this module and uses only part of it.
#![allow(dead_code)]

use decode_lz4::{Decoder, Error};
use std::io::Read;

/// The most a check decodes of any input.
pub const CAP: usize = 32 << 20;

/// Decodes `input`, giving the decoder at most `piece` bytes of input and
/// room for as many of output at a time, up to `CAP` in all, and then
/// [`finish`](Decoder::finish)es it.
pub fn decode(input: &[u8], piece: usize) -> Result<Vec<u8>, Error> {
    let (output, decoder) = unfinished(input, piece)?;
    decoder.finish()?;
    Ok(output)
}

/// Decodes `input` as [`decode`] does, but returns what it decoded, and
/// the decoder, before it [`finish`](Decoder::finish)es.
pub fn unfinished(input: &[u8], piece: usize) -> Result<(Vec<u8>, Decoder), Error> {
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
    Ok((output, decoder))
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
    let whole = decode(input, input.len().max(1 << 20));
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

/// Decodes `input`, one or more frames, through `lz4_flex`: what it
/// decodes to, or `None` if it refuses it, or leaves bytes unread after
/// the last frame, which `lz4` itself refuses.
pub fn lz4_flex(input: &[u8]) -> Option<Vec<u8>> {
    let (mut output, mut rest) = (Vec::new(), input);
    let decoder = lz4_flex::frame::FrameDecoder::new(&mut rest);
    decoder.take(CAP as u64).read_to_end(&mut output).ok()?;
    rest.is_empty().then_some(output)
}

/// Decodes `block` through `lz4_flex`, into room for `max` bytes.
pub fn lz4_flex_block(block: &[u8], max: usize) -> Option<Vec<u8>> {
    let mut output = vec![0; max];
    let len = lz4_flex::block::decompress_into(block, &mut output).ok()?;
    output.truncate(len);
    Some(output)
}

/// The XXH32 of `bytes`, from a seed of zero, as `lz4_flex` computes it
/// with `twox-hash`: here, our own.
pub fn xxh32(bytes: &[u8]) -> u32 {
    compute_xxhash::xxh32(bytes, 0)
}

/// Returns a frame of `blocks`, each stored as is or compressed, with
/// the flags in `flags` besides the version: of linked blocks, unless
/// 0x20, block checksums 0x10, a content size 0x08, and a content
/// checksum 0x04; with blocks of at most 64 KiB.
pub fn frame(flags: u8, blocks: &[(bool, &[u8])], content: &[u8]) -> Vec<u8> {
    let mut descriptor = vec![0x40 | flags, 0x40];
    if flags & 0x08 != 0 {
        descriptor.extend_from_slice(&(content.len() as u64).to_le_bytes());
    }
    let mut frame = 0x184D_2204_u32.to_le_bytes().to_vec();
    frame.extend_from_slice(&descriptor);
    frame.push((xxh32(&descriptor) >> 8) as u8);
    for &(stored, data) in blocks {
        let size = data.len() as u32 | if stored { 0x8000_0000 } else { 0 };
        frame.extend_from_slice(&size.to_le_bytes());
        frame.extend_from_slice(data);
        if flags & 0x10 != 0 {
            frame.extend_from_slice(&xxh32(data).to_le_bytes());
        }
    }
    frame.extend_from_slice(&[0; 4]);
    if flags & 0x04 != 0 {
        frame.extend_from_slice(&xxh32(content).to_le_bytes());
    }
    frame
}

/// Returns a skippable frame of `bytes`.
pub fn skippable(bytes: &[u8]) -> Vec<u8> {
    let mut frame = 0x184D_2A53_u32.to_le_bytes().to_vec();
    frame.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    frame.extend_from_slice(bytes);
    frame
}

/// `seq 1 3000`.
pub fn numbers() -> Vec<u8> {
    (1..=3000)
        .flat_map(|n| format!("{n}\n").into_bytes())
        .collect()
}

/// The CRC-32 that `cksum -a crc32b` and zlib compute.
pub fn crc32(bytes: &[u8]) -> u32 {
    !bytes.iter().fold(!0_u32, |crc, &byte| {
        (0..8).fold(crc ^ u32::from(byte), |crc, _| {
            (crc >> 1) ^ (0xEDB8_8320 & (crc & 1).wrapping_neg())
        })
    })
}

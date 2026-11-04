// This is free and unencumbered software released into the public domain.

//! zstd streams, as zstd writes them and as built by hand.

#![cfg(feature = "zstd")]

mod common;

use common::{Block, With, agree, numbers, skippable, unpack, zstd};
use filer_api::{Error, ErrorKind};
use filer_pack::Zstd;

fn decode(stream: &[u8]) -> Result<Vec<u8>, Error> {
    let whole = agree(Zstd::new, stream, 1);
    assert_eq!(
        whole.as_ref().ok(),
        agree(Zstd::new, stream, 7).as_ref().ok()
    );
    whole
}

fn kind(stream: &[u8]) -> ErrorKind {
    decode(stream).unwrap_err().kind()
}

/// `numbers.zst` is by zstd 1.5.7: at `-19` with a 1 KiB window, so in 14
/// blocks, with a checksum and size; then at `-1` with neither.
#[test]
fn decodes_what_zstd_writes() {
    let stream = include_bytes!("data/numbers.zst");
    assert_eq!(decode(stream).unwrap(), numbers());
}

#[test]
fn decodes_each_kind_of_block() {
    let stream = zstd(&[
        Block::Raw(b"raw, "),
        Block::Rle(b'r', 70_000),
        Block::Raw(b""),
    ]);
    let mut expected = b"raw, ".to_vec();
    expected.resize(70_005, b'r');
    assert_eq!(decode(&stream).unwrap(), expected);
}

#[test]
fn reads_past_skippable_frames() {
    let stream = [
        skippable(b"before"),
        include_bytes!("data/numbers.zst").to_vec(),
        skippable(b""),
        zstd(&[Block::Raw(b"after")]),
        skippable(b"last"),
    ]
    .concat();
    let mut expected = numbers();
    expected.extend_from_slice(b"after");
    assert_eq!(decode(&stream).unwrap(), expected);
}

#[test]
fn refuses_malformed_streams() {
    let good = include_bytes!("data/numbers.zst");
    let corrupt = |at: usize, byte: u8| {
        let mut stream = good.to_vec();
        stream[at] = byte;
        kind(&stream)
    };
    // The magic number, the reserved bit, and the checksum.
    assert_eq!(corrupt(0, 0x29), ErrorKind::Corrupt);
    assert_eq!(corrupt(4, good[4] | 0x08), ErrorKind::Corrupt);
    assert_eq!(corrupt(5815, good[5815] ^ 1), ErrorKind::Corrupt);
    // A reserved block type, and a block too large.
    let mut reserved = zstd(&[Block::Raw(b"x")]);
    reserved[6] |= 0x06;
    assert_eq!(kind(&reserved), ErrorKind::Corrupt);
    let mut large = zstd(&[Block::Raw(&[0; 128 * 1024])]);
    large[8] += 1;
    assert_eq!(kind(&large), ErrorKind::Corrupt);
    // What follows the last frame must be one.
    let stream = [&good[..], b"garbage"].concat();
    assert_eq!(kind(&stream), ErrorKind::Corrupt);
    // Ending early, anywhere but after the first frame, or before any.
    let ends = (0..good.len())
        .step_by(7)
        .chain(good.len() - 20..good.len());
    for end in ends.filter(|&end| end != 5819) {
        assert_eq!(kind(&good[..end]), ErrorKind::Corrupt, "{end}");
    }
}

#[test]
fn refuses_what_it_cannot_decode() {
    // A window of 256 MiB, past the 128 MiB that zstd decodes by default.
    let mut large = zstd(&[Block::Raw(b"x")]);
    large[5] = 18 << 3;
    assert_eq!(kind(&large), ErrorKind::Unsupported);
    // A dictionary.
    let mut dictionary = zstd(&[Block::Raw(b"x")]);
    dictionary[4] = 0x01;
    dictionary.insert(6, 7);
    assert_eq!(kind(&dictionary), ErrorKind::Unsupported);
}

#[test]
fn checks_the_checksum_after_the_declared_size() {
    let read = |stream, size| unpack(stream, size, With(|| Box::new(Zstd::new())));
    let stream = include_bytes!("data/numbers.zst").to_vec();
    let numbers = numbers();
    assert_eq!(read(stream.clone(), numbers.len()).unwrap(), numbers);
    let error = read(stream.clone(), numbers.len() - 1).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Corrupt);
    // The first frame's checksum, which its data reaches.
    let mut bad = stream;
    bad[5815] ^= 1;
    let error = read(bad, numbers.len()).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Corrupt);
}

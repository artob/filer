// This is free and unencumbered software released into the public domain.

//! Raw LZ4 blocks, as `lz4_flex` writes them and as built by hand, checked
//! against its decoder.

mod common;

use common::{lz4_flex_block, numbers};
use decode_lz4::{Error, decode_block};

/// Decodes `block` into room for `max` bytes, and checks that `lz4_flex`
/// decodes it alike.
fn decode(block: &[u8], max: usize) -> Result<Vec<u8>, Error> {
    let mut output = vec![0; max];
    let ours = decode_block(block, &mut output).map(|len| output[..len].to_vec());
    assert_eq!(ours.as_ref().ok(), lz4_flex_block(block, max).as_ref());
    ours
}

#[test]
fn decodes_what_lz4_flex_writes() {
    let mut rng = 0x1234_5678_u32;
    let mut random = || {
        rng ^= rng << 13;
        rng ^= rng >> 17;
        rng ^= rng << 5;
        rng
    };
    let noise: Vec<u8> = (0..5000).map(|_| random() as u8).collect();
    let sparse: Vec<u8> = (0..70_000).map(|_| (random() % 4) as u8).collect();
    for data in [
        numbers(),
        numbers().repeat(30),
        noise,
        sparse,
        vec![7; 100_000],
        b"short".to_vec(),
        Vec::new(),
    ] {
        let block = lz4_flex::block::compress(&data);
        assert_eq!(decode(&block, data.len()).unwrap(), data);
        // No room for the last byte.
        if let Some(less) = data.len().checked_sub(1) {
            assert_eq!(decode(&block, less).unwrap_err(), Error::Corrupt);
        }
    }
}

#[test]
fn copies_matches_that_overlap() {
    // Offsets of 1, 2, 3, and 7, each copying 100 bytes.
    for offset in [1_u8, 2, 3, 7] {
        let literals: Vec<u8> = (b'a'..b'a' + offset).collect();
        let mut block = vec![offset << 4 | 15];
        block.extend_from_slice(&literals);
        block.extend_from_slice(&[offset, 0, 100 - 4 - 15, 0x50]);
        block.extend_from_slice(b"12345");
        let decoded = decode(&block, 1000).unwrap();
        let expected: Vec<u8> = literals
            .iter()
            .copied()
            .cycle()
            .take(offset as usize + 100)
            .collect();
        assert_eq!(decoded[..offset as usize + 100], expected, "{offset}");
        assert_eq!(&decoded[offset as usize + 100..], b"12345");
    }
}

#[test]
fn reads_long_lengths() {
    // 15 + 255 + 255 + 10 literals, and a match as long.
    let literals = vec![b'x'; 535];
    let mut block = vec![0xFF, 255, 255, 10];
    block.extend_from_slice(&literals);
    block.extend_from_slice(&[1, 0, 255, 255, 10, 0x50]);
    block.extend_from_slice(b"12345");
    let decoded = decode(&block, 2000).unwrap();
    assert_eq!(decoded.len(), 535 + 535 + 4 + 5);
}

#[test]
fn refuses_malformed_blocks() {
    for block in [
        // Nothing, not even a token.
        &[][..],
        // Literals cut short.
        &[0x30, b'a', b'b'],
        // An offset of zero, and from before the start.
        &[0x10, b'a', 0, 0, 0x50, 1, 2, 3, 4, 5],
        &[0x10, b'a', 2, 0, 0x50, 1, 2, 3, 4, 5],
        // An offset cut short, and a length.
        &[0x10, b'a', 1],
        &[0x1F, b'a', 1, 0, 255],
    ] {
        assert_eq!(decode(block, 100).unwrap_err(), Error::Corrupt, "{block:?}");
    }
    // An empty block is one token.
    assert_eq!(decode(&[0], 0).unwrap(), b"");
}

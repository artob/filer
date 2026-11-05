// This is free and unencumbered software released into the public domain.

//! Raw LZO1X blocks, as `lzokay-native` writes them and as built by hand,
//! checked against its decoder.

mod common;

use common::{lzokay, numbers};
use decode_lzo::{Error, decode_block};

/// Decodes `block` into room for `max` bytes, and checks that
/// `lzokay-native` decodes it alike.
fn decode(block: &[u8], max: usize) -> Result<Vec<u8>, Error> {
    let mut output = vec![0; max];
    let ours = decode_block(block, &mut output).map(|len| output[..len].to_vec());
    assert_eq!(ours.as_ref().ok(), lzokay(block, max).as_ref());
    ours
}

/// Returns a run of `n` literals, of 4 or more, as the first instruction
/// of a block or after a match with none: its lengths past 18 in zeros of
/// 255 each, and the rest.
fn literals(data: &[u8]) -> Vec<u8> {
    let extra = data.len() - 18;
    let mut run = vec![0];
    run.extend(std::iter::repeat_n(0, extra / 255));
    run.push((extra % 255) as u8);
    run.extend_from_slice(data);
    run
}

/// The end mark.
const END: [u8; 3] = [0x11, 0, 0];

#[test]
fn decodes_what_lzokay_writes() {
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
    ] {
        let block = lzokay_native::compress(&data).unwrap();
        assert_eq!(decode(&block, data.len()).unwrap(), data);
        // No room for the last byte.
        if let Some(less) = data.len().checked_sub(1) {
            assert_eq!(decode(&block, less).unwrap_err(), Error::Corrupt);
        }
    }
}

#[test]
fn decodes_each_instruction() {
    // A first run of 3 literals, 2 bytes from 2 back with a literal after
    // them, 8 from 1 back, and 300 from 2 back.
    let block = [
        &[20, b'a', b'b', b'c', 0x05, 0x00, b'x'][..],
        &[0xE0, 0],
        &[0x20, 0x00, 12, 4, 0],
        &END,
    ]
    .concat();
    let mut expected = b"abcbcx".to_vec();
    expected.extend_from_slice(&[b'x'; 8]);
    for _ in 0..300 {
        expected.push(expected[expected.len() - 2]);
    }
    assert_eq!(decode(&block, 1000).unwrap(), expected);
    // After a run of 4 or more, 3 bytes from 2100 back; 8 from 20,000
    // back, past 16 KiB, with two literals after them.
    let data: Vec<u8> = (0..20_000_u32).map(|n| (n * 7 / 3) as u8).collect();
    let block = [
        literals(&data),
        vec![0x0C, 12],
        vec![0x16, 0x82, 0x38, b'y', b'z'],
        END.to_vec(),
    ]
    .concat();
    let mut expected = data.clone();
    expected.extend_from_slice(&data[20_000 - 2100..][..3]);
    expected.extend_from_slice(&expected.clone()[expected.len() - 20_000..][..8]);
    expected.extend_from_slice(b"yz");
    assert_eq!(decode(&block, 30_000).unwrap(), expected);
}

#[test]
fn copies_matches_that_overlap() {
    // Offsets of 1, 2, 3, and 7, each copying 100 bytes, and a literal.
    for offset in [1_u8, 2, 3, 7] {
        let literals: Vec<u8> = (b'a'..b'a' + offset).collect();
        let mut block = vec![17 + offset];
        block.extend_from_slice(&literals);
        let back = u16::from(offset - 1) << 2 | 1;
        block.extend_from_slice(&[0x20, 100 - 33]);
        block.extend_from_slice(&back.to_le_bytes());
        block.push(b'.');
        block.extend_from_slice(&END);
        let decoded = decode(&block, 1000).unwrap();
        let expected: Vec<u8> = literals
            .iter()
            .copied()
            .cycle()
            .take(usize::from(offset) + 100)
            .chain(*b".")
            .collect();
        assert_eq!(decoded, expected, "{offset}");
    }
}

#[test]
fn ends_at_the_end_mark() {
    // An empty block is the end mark alone, which `lzokay-native` does
    // not write for nothing.
    assert_eq!(decode(&END, 0).unwrap(), b"");
    // The literals it says follow do not matter; its length does.
    let hello = [22, b'h', b'e', b'l', b'l', b'o'];
    for mark in [[0x11, 1, 0], [0x11, 3, 0]] {
        let block = [&hello[..], &mark].concat();
        assert_eq!(decode(&block, 10).unwrap(), b"hello", "{mark:?}");
    }
    for mark in [[0x12, 0, 0], [0x17, 0, 0]] {
        let block = [&hello[..], &mark].concat();
        assert_eq!(decode(&block, 10).unwrap_err(), Error::Corrupt, "{mark:?}");
    }
}

#[test]
fn refuses_blocks_that_name_a_version() {
    // A first byte of 17 names the version in the next, given five bytes
    // or more: version 1, the Linux kernel's `lzo-rle`, with a run of
    // zeros; or version 0, which only such a block begins so.
    for version in [1, 0] {
        let block = [17, version, 0x11, 0, 0];
        assert_eq!(decode(&block, 10).unwrap_err(), Error::Unsupported);
    }
    // Shorter, it is an instruction, which has nothing to copy.
    assert_eq!(decode(&[17, 1, 0, 0], 10).unwrap_err(), Error::Corrupt);
}

#[test]
fn refuses_malformed_blocks() {
    for block in [
        // Nothing, not even the end mark.
        &[][..],
        // No end mark, or one cut short.
        &[22, b'h', b'e', b'l', b'l', b'o'],
        &[22, b'h', b'e', b'l', b'l', b'o', 0x11, 0],
        // Literals cut short.
        &[22, b'h', b'e', b'l'],
        // A match from before the start.
        &[18, b'a', 0x90, 0, 0x11, 0, 0],
        &[0x12, 0x82, 0x38, 0x11, 0, 0],
        // A length cut short.
        &[18, b'a', 0x20, 0, 0],
    ] {
        assert_eq!(decode(block, 100).unwrap_err(), Error::Corrupt, "{block:?}");
    }
    // Bytes after the end mark, which `lzokay-native` ignores.
    let trailing = [22, b'h', b'e', b'l', b'l', b'o', 0x11, 0, 0, 0];
    assert_eq!(decode(&trailing, 100).unwrap_err(), Error::Corrupt);
    assert!(lzokay_native::decompress_all(&trailing, None).is_ok());
}

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
    // Each offset from 1 to 20, past as many literals, copying lengths on
    // either side of what is copied 8 and 16 bytes at a time, from within
    // 16 KiB, then a literal, into room for exactly what it decodes to,
    // and into more.
    for offset in 1..=20_u8 {
        for len in [3, 4, 8, 15, 16, 17, 18, 19, 33, 64, 65, 100] {
            let literals: Vec<u8> = (b'a'..b'a' + offset).collect();
            let mut block = vec![17 + offset];
            block.extend_from_slice(&literals);
            match len - 2 {
                field @ 1..=31 => block.push(0x20 | field as u8),
                more => block.extend_from_slice(&[0x20, (more - 31) as u8]),
            }
            let back = u16::from(offset - 1) << 2 | 1;
            block.extend_from_slice(&back.to_le_bytes());
            block.push(b'.');
            block.extend_from_slice(&END);
            let expected: Vec<u8> = literals
                .iter()
                .copied()
                .cycle()
                .take(usize::from(offset) + len)
                .chain(*b".")
                .collect();
            for room in [expected.len(), expected.len() + 64] {
                let decoded = decode(&block, room).unwrap();
                assert_eq!(decoded, expected, "{offset}, {len}, {room}");
            }
        }
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

/// Decodes `block`, which names its version, into room for `max` bytes:
/// `lzokay-native` knows no versions, and is not asked.
fn versioned(block: &[u8], max: usize) -> Result<Vec<u8>, Error> {
    let mut output = vec![0; max];
    decode_block(block, &mut output).map(|len| output[..len].to_vec())
}

/// A run of `n` zeros, 4 to 2,051, in version 1, with `next` literals,
/// 0 to 3, after it.
fn zeros(n: usize, next: u8) -> Vec<u8> {
    let n = n - 4;
    vec![0x18 | (n & 7) as u8, 0xFC | next, 0xFF, (n >> 3) as u8]
}

#[test]
fn decodes_blocks_that_name_a_version() {
    // A first byte of 17 names the version in the next, given five bytes
    // or more, and instructions follow as at a block's start: version 0,
    // which only such a block begins so, or 1, the Linux kernel's
    // `lzo-rle`. Five bytes are the least, an empty block's.
    for version in [0, 1] {
        assert_eq!(versioned(&[17, version, 0x11, 0, 0], 0).unwrap(), b"");
        let block = [&[17, version, 22, b'h', b'e', b'l', b'l', b'o'][..], &END].concat();
        assert_eq!(versioned(&block, 5).unwrap(), b"hello");
    }
    // Shorter, it is an instruction, which has nothing to copy.
    assert_eq!(decode(&[17, 1, 0, 0], 10).unwrap_err(), Error::Corrupt);
    // A version past 1 is not this decoder's.
    for version in [2, 255] {
        let block = [17, version, 0x11, 0, 0];
        assert_eq!(versioned(&block, 10).unwrap_err(), Error::Unsupported);
    }
}

#[test]
fn decodes_runs_of_zeros() {
    // In version 1: `a`, the fewest zeros, `b`, then the most, three
    // literals after them, 11 zeros more, and 3 bytes from 2 back.
    let block = [
        &[17, 1, 18, b'a'][..],
        &zeros(4, 1),
        b"b",
        &zeros(2051, 3),
        b"xyz",
        &zeros(11, 0),
        &[0x40 | 2 << 5, 0],
        &END,
    ]
    .concat();
    let mut expected = b"a".to_vec();
    expected.extend_from_slice(&[0; 4]);
    expected.push(b'b');
    expected.extend_from_slice(&[0; 2051]);
    expected.extend_from_slice(b"xyz");
    expected.extend_from_slice(&[0; 11]);
    expected.extend_from_slice(&[0; 3]);
    assert_eq!(versioned(&block, 3000).unwrap(), expected);
    // Its distance comes before any length's bytes: a first length field
    // of 0 is the run's own.
    let block = [&[17, 1, 18, b'a'][..], &[0x18, 0xFC, 0xFF, 1], &END].concat();
    assert_eq!(versioned(&block, 20).unwrap(), b"a\0\0\0\0\0\0\0\0\0\0\0\0");
    // No room for the last zero, or no byte of its length.
    let block = [&[17, 1, 18, b'a'][..], &zeros(8, 0), &END].concat();
    assert_eq!(versioned(&block, 8).unwrap_err(), Error::Corrupt);
    let block = [17, 1, 18, b'a', 0x1C, 0xFC, 0xFF];
    assert_eq!(versioned(&block, 100).unwrap_err(), Error::Corrupt);
}

#[test]
fn matches_from_48_kib_back_outside_version_1() {
    // The same bytes are a match from 49,151 back in a block that names
    // no version, or version 0: 6 bytes, one literal after.
    let data: Vec<u8> = (0..50_000_u32).map(|n| (n * 7 / 3) as u8).collect();
    let mut expected = data.clone();
    expected.extend_from_slice(&data[50_000 - 49_151..][..6]);
    expected.push(b'.');
    let body = [literals(&data), vec![0x1C, 0xFD, 0xFF, b'.'], END.to_vec()].concat();
    assert_eq!(decode(&body, 60_000).unwrap(), expected);
    let named = [&[17, 0][..], &body].concat();
    assert_eq!(versioned(&named, 60_000).unwrap(), expected);
    // In version 1, they are a run of zeros, and what follows misread:
    // only a distance of all ones with the 48 KiB bit set is one, and
    // 32,767 back, or 49,150, is a match still.
    let named = [&[17, 1][..], &body].concat();
    assert_ne!(versioned(&named, 60_000).ok(), Some(expected));
    for (op, word, back) in [(0x14, 0xFFFD_u16, 32_767), (0x1C, 0xFFF9, 49_150)] {
        let [low, high] = word.to_le_bytes();
        let block = [&[17, 1][..], &literals(&data), &[op, low, high, b'.'], &END].concat();
        let mut expected = data.clone();
        expected.extend_from_slice(&data[50_000 - back..][..6]);
        expected.push(b'.');
        assert_eq!(versioned(&block, 60_000).unwrap(), expected, "{back}");
    }
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

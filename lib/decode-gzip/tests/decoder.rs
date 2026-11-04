// This is free and unencumbered software released into the public domain.

//! gzip streams, as gzip writes them and as built by hand, each checked
//! against zlib's decoder, through `zlib-rs`.

mod common;

use common::{Member, agree, crc32, numbers, zlib};
use decode_gzip::{Decoder, Error};

/// Decodes `stream` in pieces of 1 and 7 bytes and whole, checks that zlib
/// decodes it alike, and returns what it decodes to and consumes.
fn decode(stream: &[u8]) -> Result<(Vec<u8>, usize), Error> {
    let whole = agree(stream, 1);
    assert_eq!(whole, agree(stream, 7));
    assert_eq!(whole.clone().ok(), zlib(stream), "zlib decodes otherwise");
    whole
}

/// Returns why `stream` does not decode.
fn error(stream: &[u8]) -> Error {
    decode(stream).unwrap_err()
}

/// `numbers.gz` is by Apple's gzip 479, at `-9` and then `-1`.
#[test]
fn decodes_what_gzip_writes() {
    let stream = include_bytes!("data/numbers.gz");
    assert_eq!(decode(stream).unwrap(), (numbers(), stream.len()));
}

#[test]
fn decodes_each_level() {
    let data: Vec<u8> = (0..100_000_u32)
        .flat_map(|n| (n % 1000).to_le_bytes())
        .collect();
    for level in 0..=10 {
        let stream = Member::default().gzip_at(&data, level);
        assert_eq!(decode(&stream).unwrap().0, data, "{level}");
    }
}

#[test]
fn reads_past_each_optional_field() {
    for fields in 0..16 {
        let member = Member {
            extra: (fields & 1 != 0).then_some(&b"AB\x02\x00hi"[..]),
            name: (fields & 2 != 0).then_some(&b"name.txt"[..]),
            comment: (fields & 4 != 0).then_some(&b"a comment"[..]),
            header_crc: fields & 8 != 0,
        };
        let stream = member.gzip(b"some data, some data");
        assert_eq!(
            decode(&stream).unwrap().0,
            b"some data, some data",
            "{fields}"
        );
    }
    // An empty extra field, name, and comment.
    let member = Member {
        extra: Some(b""),
        name: Some(b""),
        comment: Some(b""),
        header_crc: true,
    };
    assert_eq!(decode(&member.gzip(b"data")).unwrap().0, b"data");
}

#[test]
fn decodes_members_in_turn() {
    let mut stream = Member::default().gzip(b"one, ");
    stream.extend(Member::default().gzip(b""));
    stream.extend(Member::default().gzip(b"two"));
    assert_eq!(
        decode(&stream).unwrap(),
        (b"one, two".to_vec(), stream.len())
    );
}

#[test]
fn leaves_what_follows_the_last_member() {
    let member = Member::default().gzip(b"data");
    for garbage in [&b"garbage"[..], &[0; 512], b"\x00\x1F\x8B"] {
        let stream = [&member[..], garbage].concat();
        let decoded = decode(&stream).unwrap();
        assert_eq!(decoded, (b"data".to_vec(), member.len()), "{garbage:?}");
    }
    // But what starts as a member must be one.
    for garbage in [&b"\x1F\x8C"[..], b"\x1F", b"\x1F\x8B\x08"] {
        let stream = [&member[..], garbage].concat();
        assert_eq!(error(&stream), Error::Corrupt, "{garbage:?}");
    }
}

#[test]
fn refuses_malformed_streams() {
    let member = Member {
        name: Some(b"name"),
        header_crc: true,
        ..Member::default()
    };
    let good = member.gzip(b"data");
    let corrupt = |at: usize, byte: u8| {
        let mut stream = good.clone();
        stream[at] = byte;
        error(&stream)
    };
    // The magic number, the method, a reserved flag, and the header CRC.
    assert_eq!(corrupt(0, 0x1E), Error::Corrupt);
    assert_eq!(corrupt(1, 0x8C), Error::Corrupt);
    assert_eq!(corrupt(2, 7), Error::Corrupt);
    assert_eq!(corrupt(3, 0x0A | 0x20), Error::Corrupt);
    assert_eq!(corrupt(15, good[15] ^ 1), Error::Corrupt);
    // The trailer's CRC and length.
    let len = good.len();
    assert_eq!(corrupt(len - 8, good[len - 8] ^ 1), Error::Corrupt);
    assert_eq!(corrupt(len - 1, 1), Error::Corrupt);
    // Ending early, anywhere, or before any member.
    for end in 0..len {
        assert_eq!(error(&good[..end]), Error::Corrupt, "{end}");
    }
    // Deflate data that is not, behind a valid header and trailer.
    let mut stream = Member::default().header();
    stream.extend_from_slice(&[0x07]);
    stream.extend_from_slice(&crc32(b"").to_le_bytes());
    stream.extend_from_slice(&[0; 4]);
    assert_eq!(error(&stream), Error::Corrupt);
}

#[test]
fn offers_checkpoints_anywhere() {
    let data: Vec<u8> = (0..200_000_u32).map(|n| (n * 7 % 251) as u8).collect();
    let mut stream = Member::default().gzip(&data);
    stream.extend(Member::default().gzip(&data[..50_000]));
    let expected = [&data[..], &data[..50_000]].concat();
    let mut decoder = Decoder::new();
    let mut output = [0; 1000];
    let (mut at, mut decoded, mut resumed) = (0, Vec::new(), Vec::new());
    for call in 0.. {
        let end = stream.len().min(at + 37);
        let (consumed, produced) = decoder.decode(&stream[at..end], &mut output).unwrap();
        if (consumed, produced) == (0, 0) {
            break;
        }
        at += consumed;
        decoded.extend_from_slice(&output[..produced]);
        if call % 10 == 0 {
            resumed.push((
                at - decoder.buffered(),
                decoded.len(),
                decoder.checkpoint().unwrap(),
            ));
        }
    }
    decoder.finish().unwrap();
    assert!(decoded == expected);
    assert!(resumed.len() > 10, "{} checkpoints", resumed.len());
    for (from, len, mut checkpoint) in resumed {
        let (mut at, mut rest) = (from, Vec::new());
        loop {
            let (consumed, produced) = checkpoint.decode(&stream[at..], &mut output).unwrap();
            if (consumed, produced) == (0, 0) {
                break;
            }
            at += consumed;
            rest.extend_from_slice(&output[..produced]);
        }
        checkpoint.finish().unwrap();
        assert!(rest == expected[len..], "from {from}");
    }
}

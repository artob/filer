// This is free and unencumbered software released into the public domain.

//! gzip streams, as gzip writes them and as built by hand.

#![cfg(feature = "gzip")]

mod common;

use common::{Member, agree};
use filer_api::{BufResult, Error, ErrorKind, ReadAt};
use filer_pack::{Codec, Decoder, Gzip, Unpacked};
use filer_spec::block_on;

fn decode(stream: &[u8]) -> Result<Vec<u8>, Error> {
    let whole = agree(Gzip::new, stream, 1);
    assert_eq!(
        whole.as_ref().ok(),
        agree(Gzip::new, stream, 7).as_ref().ok()
    );
    whole
}

fn kind(stream: &[u8]) -> ErrorKind {
    decode(stream).unwrap_err().kind()
}

/// What `tests/data/numbers.gz` holds: `seq 1 3000` from a file, by
/// Apple's gzip 479 at `-9`, then `end\n` from a pipe at `-1`.
fn numbers() -> Vec<u8> {
    let mut numbers: Vec<u8> = (1..=3000)
        .flat_map(|n| format!("{n}\n").into_bytes())
        .collect();
    numbers.extend_from_slice(b"end\n");
    numbers
}

#[test]
fn decodes_what_gzip_writes() {
    let stream = include_bytes!("data/numbers.gz");
    assert_eq!(decode(stream).unwrap(), numbers());
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
            decode(&stream).unwrap(),
            b"some data, some data",
            "{fields}"
        );
    }
}

#[test]
fn decodes_members_in_turn() {
    let mut stream = Member::default().gzip(b"one, ");
    stream.extend(Member::default().gzip(b""));
    stream.extend(Member::default().gzip(b"two"));
    assert_eq!(decode(&stream).unwrap(), b"one, two");
}

#[test]
fn ignores_what_follows_the_last_member() {
    let member = Member::default().gzip(b"data");
    for garbage in [&b"garbage"[..], &[0; 512], b"\x00\x1F\x8B"] {
        let stream = [&member[..], garbage].concat();
        assert_eq!(decode(&stream).unwrap(), b"data", "{garbage:?}");
    }
    // But what starts as a member must be one.
    let stream = [&member[..], b"\x1F\x8C"].concat();
    assert_eq!(kind(&stream), ErrorKind::Corrupt);
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
        kind(&stream)
    };
    // The magic number, the method, a reserved flag, and the header CRC.
    assert_eq!(corrupt(0, 0x1E), ErrorKind::Corrupt);
    assert_eq!(corrupt(1, 0x8C), ErrorKind::Corrupt);
    assert_eq!(corrupt(2, 7), ErrorKind::Corrupt);
    assert_eq!(corrupt(3, 0x0A | 0x20), ErrorKind::Corrupt);
    assert_eq!(corrupt(15, good[15] ^ 1), ErrorKind::Corrupt);
    // The trailer's CRC and length.
    let len = good.len();
    assert_eq!(corrupt(len - 8, good[len - 8] ^ 1), ErrorKind::Corrupt);
    assert_eq!(corrupt(len - 1, 1), ErrorKind::Corrupt);
    // Ending early, anywhere, or before any member.
    for end in 0..len {
        assert_eq!(kind(&good[..end]), ErrorKind::Corrupt, "{end}");
    }
}

/// gzip as a codec, for a stream read through `Unpacked`.
#[derive(Clone, Copy)]
struct Gzipped;

impl Codec for Gzipped {
    fn is_stored(&self) -> bool {
        false
    }

    fn decoder(&self) -> Result<Box<dyn Decoder>, Error> {
        Ok(Box::new(Gzip::new()))
    }
}

#[test]
fn checks_the_trailer_after_the_declared_size() {
    let read = |stream: Vec<u8>, size: usize| {
        let len = stream.len() as u64;
        let unpacked = Unpacked::new(stream, 0..len, size as u64, Gzipped);
        let BufResult(result, data) = block_on(unpacked.read_exact_at(Vec::with_capacity(size), 0));
        result.map(|()| data)
    };
    let stream = include_bytes!("data/numbers.gz").to_vec();
    let numbers = numbers();
    assert_eq!(read(stream.clone(), numbers.len()).unwrap(), numbers);
    // Declaring less than the stream holds.
    let error = read(stream.clone(), numbers.len() - 1).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Corrupt);
    // A trailer that the data reaches but does not match.
    let mut bad = stream;
    let last = bad.len() - 5;
    bad[last] ^= 1;
    assert_eq!(
        read(bad, numbers.len()).unwrap_err().kind(),
        ErrorKind::Corrupt
    );
}

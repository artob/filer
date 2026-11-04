// This is free and unencumbered software released into the public domain.

//! gzip streams, as gzip writes them and as built by hand.

#![cfg(feature = "gzip")]

mod common;

use common::{Member, With, agree, numbers, unpack};
use core::sync::atomic::Ordering;
use filer_api::{BufResult, Error, ErrorKind, ReadAt};
use filer_pack::{Gzip, Unpacked};
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

/// `numbers.gz` is by Apple's gzip 479, at `-9` and then `-1`.
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

#[test]
fn checks_the_trailer_after_the_declared_size() {
    let read = |stream, size| unpack(stream, size, With(|| Box::new(Gzip::new())));
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

#[test]
fn resumes_within_a_member() {
    // One member of 12 MiB, deflated as stored blocks, so packed as long.
    let data: Vec<u8> = (0..12 << 20)
        .map(|n: u32| (n % 251) as u8 ^ (n >> 20) as u8)
        .collect();
    let mut stream = Member::default().header();
    stream.extend(miniz_oxide::deflate::compress_to_vec(&data, 0));
    stream.extend(filer_api::checksum::crc32(&data).to_le_bytes());
    stream.extend((data.len() as u32).to_le_bytes());
    let len = stream.len() as u64;
    let source = filer_spec::Counted::new(stream);
    let reads = source.reads();
    let codec = With(|| Box::new(Gzip::new()));
    let unpacked = block_on(Unpacked::measured(source, 0..len, codec, 12 << 20)).unwrap();
    // Reading back near the end reads on from the checkpoint before, at
    // most 4 MiB back, as do reads back from there.
    let all = reads.load(Ordering::Relaxed);
    for at in [(12 << 20) - 1000, (6 << 20) + 7, (11 << 20) + 3] {
        let before = reads.load(Ordering::Relaxed);
        let read = unpacked.read_exact_at(Vec::with_capacity(100), at);
        let BufResult(result, bytes) = block_on(read);
        result.unwrap();
        assert_eq!(bytes, data[at as usize..at as usize + 100]);
        let resumed = reads.load(Ordering::Relaxed) - before;
        assert!(
            resumed <= 66,
            "{resumed} reads at {at}, of {all} for the whole"
        );
    }
}

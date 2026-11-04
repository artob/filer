// This is free and unencumbered software released into the public domain.

//! gzip streams through `decode-gzip`'s decoder, as `Unpacked` reads them;
//! the decoder's own tests are that crate's.

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
fn refuses_corrupt_streams() {
    let good = Member::default().gzip(b"data");
    let mut bad = good.clone();
    bad[good.len() - 8] ^= 1;
    assert_eq!(kind(&bad), ErrorKind::Corrupt);
    assert_eq!(kind(&good[..good.len() - 1]), ErrorKind::Corrupt);
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

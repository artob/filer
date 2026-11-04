// This is free and unencumbered software released into the public domain.

//! zstd streams through `decode-zstd`'s decoder, as `Unpacked` reads them;
//! the decoder's own tests are that crate's.

#![cfg(feature = "zstd")]

mod common;

use common::{Block, With, agree, numbers, unpack, zstd};
use core::sync::atomic::Ordering;
use filer_api::{BufResult, Error, ErrorKind, ReadAt};
use filer_pack::{Unpacked, Zstd};
use filer_spec::block_on;

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

#[test]
fn resumes_between_frames() {
    // Twelve frames of 1 MiB, stored as they are, so packed as long.
    let frame = |i: u8| -> Vec<u8> { (0..1 << 20).map(|n| (n % 251) as u8 ^ i).collect() };
    let mut stream = Vec::new();
    for i in 0..12 {
        let data = frame(i);
        let blocks: Vec<_> = data.chunks(128 * 1024).map(Block::Raw).collect();
        stream.extend(zstd(&blocks));
    }
    let len = stream.len() as u64;
    let source = filer_spec::Counted::new(stream);
    let reads = source.reads();
    let codec = With(|| Box::new(Zstd::new()));
    let unpacked = block_on(Unpacked::measured(source, 0..len, codec, 12 << 20)).unwrap();
    // The measuring read every 64 KiB of it; reading back near the end
    // reads on from the checkpoint before, at most 4 MiB back.
    let all = reads.load(Ordering::Relaxed);
    let at = (11 << 20) + 1000;
    let read = unpacked.read_exact_at(Vec::with_capacity(100), at);
    let BufResult(result, data) = block_on(read);
    result.unwrap();
    assert_eq!(data, frame(11)[1000..1100]);
    let resumed = reads.load(Ordering::Relaxed) - all;
    assert!(resumed <= 66, "{resumed} reads, of {all} for the whole");
}

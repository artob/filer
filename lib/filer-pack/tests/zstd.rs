// This is free and unencumbered software released into the public domain.

//! zstd streams, as zstd writes them and as built by hand.

#![cfg(feature = "zstd")]

mod common;

use common::{Block, With, agree, numbers, skippable, unpack, zstd};
use core::sync::atomic::Ordering;
use filer_api::{BufResult, Error, ErrorKind, ReadAt, checksum::crc32};
use filer_pack::{Decoder, Unpacked, Zstd};
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

/// `raw.zst` is by zstd 1.5.7, of random bytes it cannot compress: 5000
/// with a 1 KiB window, in five raw blocks; 100 in a single segment; and
/// 2000 from a pipe, with neither size nor checksum.
#[test]
fn decodes_raw_blocks_as_zstd_writes_them() {
    let data = decode(include_bytes!("data/raw.zst")).unwrap();
    assert_eq!((data.len(), crc32(&data)), (7100, 0x6E88_D59A));
}

/// `literals.zst` is by zstd 1.5.7, of bytes drawn at random, unevenly,
/// from an alphabet, and too short a window to repeat: so they code as
/// literals alone, in one Huffman-coded stream or four, of each size,
/// with trees whose weights are coded with FSE or not, or reused.
#[test]
fn decodes_literals_as_zstd_writes_them() {
    let data = decode(include_bytes!("data/literals.zst")).unwrap();
    assert_eq!((data.len(), crc32(&data)), (28_200, 0x2265_4C06));
}

/// `sequences.zst` is by zstd 1.5.7: text at levels 1, 19, and 22 with a
/// 1 KiB window; data shaped as LZ77 makes it, its offsets often repeated,
/// at 3, and at 19 with a 64 KiB window, which it outgrows; records alike
/// but for a counter; and a short pattern, repeated. They take tables of
/// each mode, and repeated offsets of each kind.
#[test]
fn decodes_sequences_as_zstd_writes_them() {
    let data = decode(include_bytes!("data/sequences.zst")).unwrap();
    assert_eq!((data.len(), crc32(&data)), (471_800, 0x717B_B20A));
}

/// Decodes each `.zst` file in the directory `FILER_PACK_ZSTD_CORPUS`
/// names, whole, and times it: zstd's output of a varied corpus, at many
/// levels and windows, checked by their checksums, checks more than the
/// fixtures can, and in a release build, measures.
#[test]
#[ignore = "needs a corpus of zstd streams"]
fn decodes_a_corpus() {
    let corpus = std::env::var("FILER_PACK_ZSTD_CORPUS").unwrap();
    let mut paths: Vec<_> = std::fs::read_dir(corpus)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "zst"))
        .collect();
    paths.sort();
    let (mut bytes, start) = (0, std::time::Instant::now());
    let mut buf = vec![0; 1 << 20];
    for path in &paths {
        let stream = std::fs::read(path).unwrap();
        let (mut zstd, mut at) = (Zstd::new(), 0);
        loop {
            let (consumed, produced) = zstd.decode(&stream[at..], &mut buf).unwrap();
            if (consumed, produced) == (0, 0) {
                break;
            }
            (at, bytes) = (at + consumed, bytes + produced);
        }
        zstd.finish()
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    }
    let elapsed = start.elapsed();
    println!("{} streams, {bytes} bytes, in {elapsed:?}", paths.len());
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

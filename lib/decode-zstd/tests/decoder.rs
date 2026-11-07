// This is free and unencumbered software released into the public domain.

//! zstd streams, as zstd writes them and as built by hand.

mod common;

use common::{Block, agree, agrees_with_ruzstd, numbers, skippable, zstd};
use decode_zstd::{Decoder, Error};

/// Decodes `stream` whole and in pieces, and checks that `ruzstd` decodes
/// it alike, or refuses it too.
fn decode(stream: &[u8]) -> Result<Vec<u8>, Error> {
    let whole = agree(stream, 1);
    assert_eq!(whole.as_ref().ok(), agree(stream, 7).as_ref().ok());
    let ours = whole.as_deref().ok();
    assert!(agrees_with_ruzstd(stream, ours), "disagrees with ruzstd");
    whole
}

fn kind(stream: &[u8]) -> Error {
    decode(stream).unwrap_err()
}

/// The CRC-32 that `cksum -a crc32b` and zlib compute.
fn crc32(bytes: &[u8]) -> u32 {
    !bytes.iter().fold(!0_u32, |crc, &byte| {
        (0..8).fold(crc ^ u32::from(byte), |crc, _| {
            (crc >> 1) ^ (0xEDB8_8320 & (crc & 1).wrapping_neg())
        })
    })
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

/// Decodes each `.zst` file in the directory `DECODE_ZSTD_CORPUS`
/// names, whole, and times it: zstd's output of a varied corpus, at many
/// levels and windows, checked by their checksums, checks more than the
/// fixtures can, and in a release build, measures.
#[test]
#[ignore = "needs a corpus of zstd streams"]
fn decodes_a_corpus() {
    let corpus = std::env::var("DECODE_ZSTD_CORPUS").unwrap();
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
        let (mut decoder, mut at) = (Decoder::new(), 0);
        loop {
            let (consumed, produced) = decoder.decode(&stream[at..], &mut buf).unwrap();
            if (consumed, produced) == (0, 0) {
                break;
            }
            (at, bytes) = (at + consumed, bytes + produced);
        }
        decoder
            .finish()
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
    assert_eq!(corrupt(0, 0x29), Error::Corrupt);
    assert_eq!(corrupt(4, good[4] | 0x08), Error::Corrupt);
    assert_eq!(corrupt(5815, good[5815] ^ 1), Error::Corrupt);
    // A reserved block type, and a block too large.
    let mut reserved = zstd(&[Block::Raw(b"x")]);
    reserved[6] |= 0x06;
    assert_eq!(kind(&reserved), Error::Corrupt);
    let mut large = zstd(&[Block::Raw(&[0; 128 * 1024])]);
    large[8] += 1;
    assert_eq!(kind(&large), Error::Corrupt);
    // What `ruzstd` lets pass, checksums and all: a content size of five
    // bytes, of a frame of four; and a block of 2000 literals, all `z`, in
    // a 1 KiB window.
    let checked = |frame: &[u8], data: &[u8]| {
        let sum = compute_xxhash::xxh64(data, 0) as u32;
        let mut frame = [frame, &sum.to_le_bytes()].concat();
        frame[4] |= 0x04;
        frame
    };
    let sized = [
        0x28, 0xB5, 0x2F, 0xFD, 0x20, 5, 0x21, 0, 0, b'f', b'o', b'u', b'r',
    ];
    assert_eq!(kind(&checked(&sized, b"four")), Error::Corrupt);
    let literals = [
        0x28, 0xB5, 0x2F, 0xFD, 0, 0, 0x25, 0, 0, 0x05, 0x7D, b'z', 0,
    ];
    assert_eq!(kind(&checked(&literals, &[b'z'; 2000])), Error::Corrupt);
    // What follows the last frame must be one.
    let stream = [&good[..], b"garbage"].concat();
    assert_eq!(kind(&stream), Error::Corrupt);
    // Ending early, anywhere but after the first frame, or before any.
    let ends = (0..good.len())
        .step_by(7)
        .chain(good.len() - 20..good.len());
    for end in ends.filter(|&end| end != 5819) {
        assert_eq!(kind(&good[..end]), Error::Corrupt, "{end}");
    }
}

#[test]
fn refuses_what_it_cannot_decode() {
    // A window of 256 MiB, past the 128 MiB that zstd decodes by default.
    let mut large = zstd(&[Block::Raw(b"x")]);
    large[5] = 18 << 3;
    assert_eq!(kind(&large), Error::Unsupported);
    // A dictionary.
    let mut dictionary = zstd(&[Block::Raw(b"x")]);
    dictionary[4] = 0x01;
    dictionary.insert(6, 7);
    assert_eq!(kind(&dictionary), Error::Unsupported);
}

/// Decodes `stream` from each checkpoint that a decoder given `piece`
/// bytes at a time and room for 16 offers, every `every` calls, and
/// checks that each decodes on to the same end; returns how many it took.
fn resume_from_checkpoints(stream: &[u8], piece: usize, every: usize) -> usize {
    let mut decoder = Decoder::new();
    let mut output = [0; 16];
    let (mut at, mut decoded, mut calls) = (0, Vec::new(), 0);
    let mut resumed = Vec::new();
    loop {
        let input = &stream[at..stream.len().min(at + piece)];
        let (consumed, produced) = decoder.decode(input, &mut output).unwrap();
        if (consumed, produced) == (0, 0) && at == stream.len() {
            break;
        }
        at += consumed;
        decoded.extend_from_slice(&output[..produced]);
        calls += 1;
        if calls % every == 0 {
            let checkpoint = decoder.checkpoint().unwrap();
            resumed.push((at - decoder.buffered(), decoded.len(), checkpoint));
        }
    }
    decoder.finish().unwrap();
    let taken = resumed.len();
    // Each goes on from where it was taken to the same end, as whole.
    for (from, len, mut checkpoint) in resumed {
        let mut rest = Vec::new();
        let mut at = from;
        let mut output = [0; 4096];
        loop {
            let (consumed, produced) = checkpoint.decode(&stream[at..], &mut output).unwrap();
            if (consumed, produced) == (0, 0) {
                break;
            }
            at += consumed;
            rest.extend_from_slice(&output[..produced]);
        }
        checkpoint.finish().unwrap();
        assert!(rest == decoded[len..], "from {from}");
    }
    taken
}

/// Decodes `stream`, given whole, into room for `room` bytes at a time,
/// and returns what it decoded and how many calls produced or consumed.
fn into_room(stream: &[u8], room: usize) -> (Vec<u8>, usize) {
    let (mut decoder, mut buf) = (Decoder::new(), vec![0; room]);
    let (mut decoded, mut at, mut calls) = (Vec::new(), 0, 0);
    loop {
        let (consumed, produced) = decoder.decode(&stream[at..], &mut buf).unwrap();
        if (consumed, produced) == (0, 0) {
            break;
        }
        decoded.extend_from_slice(&buf[..produced]);
        (at, calls) = (at + consumed, calls + 1);
    }
    decoder.finish().unwrap();
    (decoded, calls)
}

#[test]
fn decodes_whole_frames_straight_into_the_output() {
    // A frame given whole, which says what it decodes to and has room
    // for it, decodes straight into the output; others, or with less
    // room, onto a window, alike. `numbers.zst` has a frame of each kind,
    // `raw.zst` three, two of them sized, `literals.zst` five, and
    // `sequences.zst` seven.
    for (stream, frames) in [
        (&include_bytes!("data/numbers.zst")[..], 2),
        (include_bytes!("data/raw.zst"), 3),
        (include_bytes!("data/literals.zst"), 5),
        (include_bytes!("data/sequences.zst"), 7),
    ] {
        let expected = common::decode(stream, 7).unwrap();
        for room in [1, 1000, expected.len() / 2, expected.len()] {
            assert_eq!(into_room(stream, room).0, expected, "{room}");
        }
        // With room for all, each call ends a frame.
        let (decoded, calls) = into_room(stream, expected.len());
        assert!(decoded == expected);
        assert_eq!(calls, frames);
    }
}

#[test]
fn offers_checkpoints_between_frames() {
    let stream = [
        zstd(&[Block::Raw(b"one")]),
        skippable(b"skipped"),
        zstd(&[Block::Rle(b'2', 3), Block::Raw(b"!")]),
    ]
    .concat();
    assert_eq!(decode(&stream).unwrap(), b"one222!");
    // A fresh decoder, before any frame, as a checkpoint is.
    let checkpoint = Decoder::new().checkpoint().unwrap();
    assert!(checkpoint.finish().is_err());
    // In whole pieces, each call ends a frame; byte by byte, it is
    // anywhere within one, gathering a header or block.
    assert_eq!(resume_from_checkpoints(&stream, stream.len(), 1), 3);
    assert_eq!(resume_from_checkpoints(&stream, 1, 1), stream.len());
}

#[test]
fn offers_checkpoints_within_frames() {
    // Compressed blocks, between which their tables, repeat offsets,
    // window, and hash so far go on; and within which output is pending.
    for (stream, piece, every) in [
        (&include_bytes!("data/sequences.zst")[..], 100, 97),
        (include_bytes!("data/numbers.zst"), 7, 29),
    ] {
        assert!(resume_from_checkpoints(stream, piece, every) > 50);
    }
}

#[test]
fn copies_only_the_window_into_checkpoints() {
    // `numbers.zst`'s first frame has a 1 KiB window.
    let stream = include_bytes!("data/numbers.zst");
    let mut decoder = Decoder::new();
    let mut output = vec![0; 30_000];
    // Past its header, its tables and no window yet.
    decoder.decode(&stream[..6], &mut output).unwrap();
    let empty = decoder.footprint();
    decoder.decode(&stream[6..3000], &mut output).unwrap();
    let checkpoint = decoder.checkpoint().unwrap();
    assert!(checkpoint.footprint() < decoder.footprint());
    let window = checkpoint.footprint() - empty;
    assert!(window <= 1024, "{window}");
}

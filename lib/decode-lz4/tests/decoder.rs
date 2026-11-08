// This is free and unencumbered software released into the public domain.

//! LZ4 frames, as `lz4` writes them and as built by hand, checked against
//! `lz4_flex`'s decoder where it reads them.

mod common;

use common::{agree, crc32, frame, lz4_flex, numbers, skippable};
use decode_lz4::{Decoder, Error};

/// Decodes `stream` in pieces of 1 and 7 bytes and whole.
fn decode(stream: &[u8]) -> Result<Vec<u8>, Error> {
    let whole = agree(stream, 1);
    assert_eq!(whole, agree(stream, 7));
    whole
}

/// Returns why `stream` does not decode.
fn error(stream: &[u8]) -> Error {
    decode(stream).unwrap_err()
}

/// `seq 1 3000`, 20 times over: 277,860 bytes.
fn twenty() -> Vec<u8> {
    numbers().repeat(20)
}

/// `linked.lz4` and `independent.lz4` are by lz4 1.10, of [`twenty`] in
/// blocks of 64 KiB: at `-9`, linked, with a checksum each, and the
/// content's size and checksum; and at `-1`, independent, with neither.
#[test]
fn decodes_what_lz4_writes() {
    for stream in [
        &include_bytes!("data/linked.lz4")[..],
        include_bytes!("data/independent.lz4"),
    ] {
        assert_eq!(decode(stream).unwrap(), twenty());
        assert_eq!(lz4_flex(stream).unwrap(), twenty());
    }
}

/// `random.lz4` is by lz4 1.10, of 70,000 random bytes, which it stores
/// as is, in blocks of 64 KiB, with a checksum each.
#[test]
fn decodes_stored_blocks() {
    let data = decode(include_bytes!("data/random.lz4")).unwrap();
    assert_eq!((data.len(), crc32(&data)), (70_000, 0x2D44_8504));
}

/// `legacy.lz4` is by `lz4 -l -9`, of `seq 1 3000` 650 times over, in two
/// blocks, the first of 8 MiB.
#[test]
fn decodes_legacy_frames() {
    let stream = include_bytes!("data/legacy.lz4");
    let data = numbers().repeat(650);
    assert_eq!(agree(stream, 4096).unwrap(), data);
    // Ended by the Linux kernel's mark, or by another frame.
    let marked = [&stream[..], &[0; 4]].concat();
    assert_eq!(agree(&marked, 4096).unwrap(), data);
    let hello = include_bytes!("data/independent.lz4");
    for next in [&hello[..], &marked] {
        let both = [&stream[..], next].concat();
        let decoded = agree(&both, 4096).unwrap();
        assert!(decoded[..data.len()] == data);
    }
    // A block too large to be one.
    let mut large = stream[..8].to_vec();
    large[4..8].copy_from_slice(&(9_u32 << 20).to_le_bytes());
    assert_eq!(error(&large), Error::Corrupt);
    // Cut short, within a block.
    assert_eq!(agree(&stream[..1000], 4096).unwrap_err(), Error::Corrupt);
}

#[test]
fn decodes_frames_built_by_hand() {
    let content = b"stored, then stored again";
    let blocks: [(bool, &[u8]); 2] = [(true, b"stored, "), (true, b"then stored again")];
    for flags in [0x20, 0x30, 0x28, 0x24, 0x3C, 0x00, 0x1C] {
        let stream = frame(flags, &blocks, content);
        assert_eq!(decode(&stream).unwrap(), content, "{flags:#x}");
    }
    // Empty, and empty blocks, stored or compressed.
    assert_eq!(decode(&frame(0x3C, &[], b"")).unwrap(), b"");
    let empty: [(bool, &[u8]); 2] = [(true, b""), (false, &[0])];
    assert_eq!(decode(&frame(0x3C, &empty, b"")).unwrap(), b"");
    // A linked block copies from the one before; an independent one may
    // not.
    let linked: [(bool, &[u8]); 2] = [(true, b"abcdefgh"), (false, &[0x04, 8, 0, 0])];
    let stream = frame(0x00, &linked, b"");
    assert_eq!(decode(&stream).unwrap(), b"abcdefghabcdefgh");
    assert_eq!(error(&frame(0x20, &linked, b"")), Error::Corrupt);
}

#[test]
fn checks_frames() {
    let content = b"some data, some data";
    let blocks: [(bool, &[u8]); 1] = [(true, content)];
    let stream = frame(0x3C, &blocks, content);
    // Any byte changed, of the header or its checksum, the block's size,
    // or a checksum.
    let at = stream.len();
    for byte in (4..11).chain([19, 23, 24]).chain(at - 8..at) {
        let mut bad = stream.clone();
        bad[byte] ^= 0x01;
        assert_eq!(error(&bad), Error::Corrupt, "{byte}");
    }
    // A content size that is not the content's.
    let short = frame(0x28, &blocks, &content[1..]);
    assert_eq!(error(&short), Error::Corrupt);
    // A dictionary, which this decoder is not given.
    let mut dictionary = frame(0x21, &[], b"");
    dictionary.splice(6..6, [1, 2, 3, 4]);
    let descriptor = dictionary[4..10].to_vec();
    dictionary[10] = (common::xxh32(&descriptor) >> 8) as u8;
    assert_eq!(error(&dictionary), Error::Unsupported);
    // Each block size: too large for the frame's, or not one.
    for bd in [0x30, 0x00, 0x80, 0x41] {
        let mut bad = frame(0x20, &[], b"");
        bad[5] = bd;
        bad[6] = (common::xxh32(&bad[4..6]) >> 8) as u8;
        assert_eq!(error(&bad), Error::Corrupt, "{bd:#x}");
    }
    // Other versions.
    for version in [0x00, 0x80, 0xC0] {
        let mut bad = frame(0x20, &[], b"");
        bad[4] = version | 0x20;
        bad[6] = (common::xxh32(&bad[4..6]) >> 8) as u8;
        assert_eq!(error(&bad), Error::Corrupt, "{version:#x}");
    }
}

#[test]
fn reads_past_skippable_frames() {
    let hello = frame(0x24, &[(true, b"hello")], b"hello");
    let stream = [skippable(b"skipped"), hello.clone(), skippable(b""), hello].concat();
    assert_eq!(decode(&stream).unwrap(), b"hellohello");
    // What follows a frame must be another.
    let trailing = [&stream[..], b"garbage"].concat();
    assert_eq!(error(&trailing), Error::Corrupt);
    // And there must be one.
    assert_eq!(error(b""), Error::Corrupt);
    assert_eq!(error(&skippable(b"only")[..5]), Error::Corrupt);
}

#[test]
fn resumes_from_checkpoints_anywhere() {
    for (stream, piece) in [
        (&include_bytes!("data/linked.lz4")[..], 333),
        (include_bytes!("data/independent.lz4"), 97),
    ] {
        let mut decoder = Decoder::new();
        let mut output = [0; 1000];
        let (mut at, mut decoded) = (0, Vec::new());
        let mut resumed = Vec::new();
        loop {
            let end = stream.len().min(at + piece);
            let (consumed, produced) = decoder.decode(&stream[at..end], &mut output).unwrap();
            if (consumed, produced) == (0, 0) && at == stream.len() {
                break;
            }
            at += consumed;
            decoded.extend_from_slice(&output[..produced]);
            if resumed.len() < 60 && decoded.len() % 7 == 0 {
                let checkpoint = decoder.checkpoint().unwrap();
                assert!(checkpoint.footprint() < 200_000);
                resumed.push((at - decoder.buffered(), decoded.len(), checkpoint));
            }
        }
        decoder.finish().unwrap();
        assert_eq!(decoded, twenty());
        assert!(resumed.len() > 20);
        for (from, len, mut checkpoint) in resumed {
            let mut output = vec![0; decoded.len()];
            let (mut at, mut done) = (from, 0);
            loop {
                let (consumed, produced) = checkpoint
                    .decode(&stream[at..], &mut output[done..])
                    .unwrap();
                if (consumed, produced) == (0, 0) {
                    break;
                }
                (at, done) = (at + consumed, done + produced);
            }
            checkpoint.finish().unwrap();
            assert!(output[..done] == decoded[len..], "from {from}");
        }
    }
}

/// Decodes each `.lz4` file in the directory `DECODE_LZ4_CORPUS` names,
/// whole, as `lz4_flex` decodes it, and times it against `lz4_flex`, in a
/// release build.
#[test]
#[ignore = "needs a corpus of .lz4 files"]
fn decodes_a_corpus() {
    use std::{
        io::Read as _,
        time::{Duration, Instant},
    };
    let corpus = std::env::var("DECODE_LZ4_CORPUS").unwrap();
    let mut paths: Vec<_> = std::fs::read_dir(corpus)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "lz4"))
        .collect();
    paths.sort();
    let (mut bytes, mut times) = (0, [Duration::ZERO; 2]);
    for path in &paths {
        let stream = std::fs::read(path).unwrap();
        let mut output = vec![0; 1 << 30];
        let start = Instant::now();
        let (mut decoder, mut at, mut len) = (Decoder::new(), 0, 0);
        loop {
            let (consumed, produced) = decoder.decode(&stream[at..], &mut output[len..]).unwrap();
            if (consumed, produced) == (0, 0) {
                break;
            }
            (at, len) = (at + consumed, len + produced);
        }
        decoder.finish().unwrap();
        times[0] += start.elapsed();
        let start = Instant::now();
        let mut theirs = Vec::with_capacity(len);
        lz4_flex::frame::FrameDecoder::new(&stream[..])
            .read_to_end(&mut theirs)
            .unwrap();
        times[1] += start.elapsed();
        assert!(output[..len] == theirs, "{}", path.display());
        bytes += len;
    }
    let [ours, theirs] = times;
    println!(
        "{} streams, {bytes} bytes: ours in {ours:?}, lz4_flex's in {theirs:?}",
        paths.len()
    );
}

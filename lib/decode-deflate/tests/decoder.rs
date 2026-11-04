// This is free and unencumbered software released into the public domain.

//! Deflate streams, as zlib and `miniz_oxide` write them and as built by
//! hand, checked against zlib's semantics through `zlib-rs`.

mod common;

use common::{agree, crc32, zlib};
use decode_deflate::{Decoder, Error};

/// The fixtures are raw Deflate by zlib 1.2.12: text at levels 1, 6, and
/// 9, with each strategy, and with a 512-byte window; data shaped as LZ77
/// makes it, with run-length matches too; bytes it cannot compress, so
/// stored; text stored at level 0; zeros; and nothing. Each with what it
/// decodes to: its length and CRC-32.
const FIXTURES: [(&str, &[u8], usize, u32); 14] = [
    (
        "text-1",
        include_bytes!("data/text-1.deflate"),
        12000,
        0x6FF5_0EE1,
    ),
    (
        "text-6",
        include_bytes!("data/text-6.deflate"),
        12000,
        0x6FF5_0EE1,
    ),
    (
        "text-9",
        include_bytes!("data/text-9.deflate"),
        12000,
        0x6FF5_0EE1,
    ),
    (
        "text-filtered",
        include_bytes!("data/text-filtered.deflate"),
        12000,
        0x6FF5_0EE1,
    ),
    (
        "text-huffman",
        include_bytes!("data/text-huffman.deflate"),
        12000,
        0x6FF5_0EE1,
    ),
    (
        "text-fixed",
        include_bytes!("data/text-fixed.deflate"),
        12000,
        0x6FF5_0EE1,
    ),
    (
        "text-small-window",
        include_bytes!("data/text-small-window.deflate"),
        12000,
        0x6FF5_0EE1,
    ),
    (
        "lz-6",
        include_bytes!("data/lz-6.deflate"),
        60000,
        0x49D9_7A9B,
    ),
    (
        "lz-9",
        include_bytes!("data/lz-9.deflate"),
        60000,
        0x49D9_7A9B,
    ),
    (
        "lz-rle",
        include_bytes!("data/lz-rle.deflate"),
        60000,
        0x49D9_7A9B,
    ),
    (
        "noise-6",
        include_bytes!("data/noise-6.deflate"),
        8000,
        0x5B16_7E76,
    ),
    (
        "stored",
        include_bytes!("data/stored.deflate"),
        5000,
        0x4D2E_4348,
    ),
    (
        "zeros-9",
        include_bytes!("data/zeros-9.deflate"),
        100_000,
        0xD411_957D,
    ),
    ("empty", include_bytes!("data/empty.deflate"), 0, 0),
];

fn kind(stream: &[u8]) -> Error {
    agree(stream, 3).unwrap_err()
}

#[test]
fn decodes_what_zlib_writes() {
    for (name, stream, len, crc) in FIXTURES {
        for piece in [1, 7, 4096] {
            let (data, consumed) = agree(stream, piece).unwrap();
            assert_eq!((data.len(), crc32(&data)), (len, crc), "{name}");
            assert_eq!(consumed, stream.len(), "{name}");
        }
        assert_eq!(
            zlib(stream).map(|(data, _)| crc32(&data)),
            Some(crc),
            "{name}"
        );
    }
}

#[test]
fn consumes_the_stream_and_no_more() {
    for (name, stream, _, _) in FIXTURES {
        let followed = [stream, b"what follows"].concat();
        for piece in [1, 2, 5, 64, 1 << 20] {
            let (_, consumed) = agree(&followed, piece).unwrap();
            assert_eq!(consumed, stream.len(), "{name} in pieces of {piece}");
        }
    }
}

#[test]
fn decodes_what_miniz_oxide_writes() {
    let mut data: Vec<u8> = (0..50_000_u32)
        .map(|n| (n * 7 % 253) as u8 ^ (n >> 9) as u8)
        .collect();
    data.extend(b"Deflate, deflate, deflate. ".repeat(500));
    for level in 0..=10 {
        let stream = miniz_oxide::deflate::compress_to_vec(&data, level);
        let (decoded, consumed) = agree(&stream, 13).unwrap();
        assert!(decoded == data, "level {level}");
        assert_eq!(consumed, stream.len());
        assert_eq!(
            zlib(&stream),
            Some((data.clone(), stream.len())),
            "level {level}"
        );
    }
}

/// Bits written as Deflate packs them: fields from their lowest bit, and
/// Huffman codes from their highest.
#[derive(Default)]
struct Bits {
    bytes: Vec<u8>,
    count: usize,
}

impl Bits {
    fn bit(mut self, bit: u32) -> Self {
        if self.count.is_multiple_of(8) {
            self.bytes.push(0);
        }
        *self.bytes.last_mut().unwrap() |= ((bit & 1) as u8) << (self.count % 8);
        self.count += 1;
        self
    }

    fn field(self, value: u32, n: u32) -> Self {
        (0..n).fold(self, |bits, i| bits.bit(value >> i))
    }

    fn code(self, code: u32, n: u32) -> Self {
        (0..n).rev().fold(self, |bits, i| bits.bit(code >> i))
    }

    /// A last block of fixed codes, begun.
    fn fixed() -> Self {
        Self::default().field(1, 1).field(1, 2)
    }

    /// The fixed code of literal `byte`, of 8 bits for those below 144.
    fn literal(self, byte: u8) -> Self {
        self.code(0x30 + u32::from(byte), 8)
    }
}

#[test]
fn refuses_malformed_streams() {
    // "aaaa" as a literal and a match of 3 from 1 back, then the end.
    let good = Bits::fixed().literal(b'a').code(1, 7).code(0, 5).code(0, 7);
    assert_eq!(agree(&good.bytes, 1).unwrap().0, b"aaaa");
    // A reserved block type.
    assert_eq!(
        kind(&Bits::default().field(1, 1).field(3, 2).bytes),
        Error::Corrupt
    );
    // A stored block whose length's complement disagrees.
    assert_eq!(kind(&[0x01, 0x05, 0x00, 0xFA, 0xEE, b'h']), Error::Corrupt);
    // A match before anything to copy, or from too far back.
    assert_eq!(
        kind(&Bits::fixed().code(1, 7).code(0, 5).bytes),
        Error::Corrupt
    );
    let far = Bits::fixed().literal(b'a').code(1, 7).code(1, 5).code(0, 7);
    assert_eq!(kind(&far.bytes), Error::Corrupt);
    // The codes that never occur: literal/length 286, and distance 30.
    assert_eq!(kind(&Bits::fixed().code(0xC6, 8).bytes), Error::Corrupt);
    let thirty = Bits::fixed()
        .literal(b'a')
        .code(1, 7)
        .code(30, 5)
        .code(0, 7);
    assert_eq!(kind(&thirty.bytes), Error::Corrupt);
    // More than 286 literal/length codes, or 30 distance codes.
    let dynamic = || Bits::default().field(1, 1).field(2, 2);
    assert_eq!(
        kind(&dynamic().field(30, 5).field(0, 5).field(0, 4).bytes),
        Error::Corrupt
    );
    assert_eq!(
        kind(&dynamic().field(0, 5).field(30, 5).field(0, 4).bytes),
        Error::Corrupt
    );
    // Ending early, anywhere.
    for (name, stream, _, _) in FIXTURES {
        for end in (0..stream.len()).step_by(stream.len() / 50 + 1) {
            assert_eq!(kind(&stream[..end]), Error::Corrupt, "{name} at {end}");
        }
    }
    // zlib agrees, on each.
    assert!(zlib(&good.bytes).is_some());
    for stream in [&far.bytes, &thirty.bytes] {
        assert!(zlib(stream).is_none());
    }
}

#[test]
fn offers_checkpoints_anywhere() {
    let (_, stream, len, _) = FIXTURES[2];
    let expected = agree(stream, 1).unwrap().0;
    let mut decoder = Decoder::new();
    let mut output = [0; 100];
    let (mut at, mut decoded, mut resumed) = (0, Vec::new(), Vec::new());
    loop {
        let end = stream.len().min(at + 37);
        let (consumed, produced) = decoder.decode(&stream[at..end], &mut output).unwrap();
        if (consumed, produced) == (0, 0) {
            break;
        }
        at += consumed;
        decoded.extend_from_slice(&output[..produced]);
        if decoded.len().is_multiple_of(7) {
            resumed.push((
                at - decoder.buffered(),
                decoded.len(),
                decoder.checkpoint().unwrap(),
            ));
        }
    }
    decoder.finish().unwrap();
    assert_eq!(decoded.len(), len);
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

/// Decodes each `.deflate` file in the directory `DECODE_DEFLATE_CORPUS`
/// names, whole, as zlib decodes it, and times it against `zlib-rs` and
/// `miniz_oxide`, in a release build.
#[test]
#[ignore = "needs a corpus of Deflate streams"]
fn decodes_a_corpus() {
    use std::time::{Duration, Instant};
    let corpus = std::env::var("DECODE_DEFLATE_CORPUS").unwrap();
    let mut paths: Vec<_> = std::fs::read_dir(corpus)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "deflate")
        })
        .collect();
    paths.sort();
    let (mut bytes, mut times) = (0, [Duration::ZERO; 3]);
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
        let mut inflate = zlib_rs::Inflate::new(false, 15);
        let mut theirs = vec![0; len];
        inflate
            .decompress(&stream, &mut theirs, zlib_rs::InflateFlush::Finish)
            .unwrap();
        times[1] += start.elapsed();
        assert!(output[..len] == theirs, "{}", path.display());
        let start = Instant::now();
        let miniz = miniz_oxide::inflate::decompress_to_vec(&stream).unwrap();
        times[2] += start.elapsed();
        assert_eq!(miniz.len(), len);
        bytes += len;
    }
    let [ours, zlib, miniz] = times;
    println!(
        "{} streams, {bytes} bytes: ours in {ours:?}, zlib-rs's in {zlib:?}, miniz_oxide's in {miniz:?}",
        paths.len()
    );
}

// This is free and unencumbered software released into the public domain.

//! Deflate streams, as zlib and `miniz_oxide` write them and as built by
//! hand, checked against zlib's semantics through `zlib-rs`.

mod common;

use common::{agree, crc32, decode, zlib};
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

/// What a call decodes before its input runs short goes out in that
/// call, not the next.
#[test]
fn produces_before_it_waits_for_input() {
    let (_, stream, len, _) = FIXTURES[0];
    let expected = agree(stream, 1).unwrap().0;
    let mut output = vec![0; len];
    let (consumed, produced) = Decoder::new()
        .decode(&stream[..stream.len() / 2], &mut output)
        .unwrap();
    assert!(consumed > 0 && produced > 0, "{consumed}, {produced}");
    assert!(output[..produced] == expected[..produced]);
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

/// Returns a stream of three blocks: one of dynamic codes, `a` A's then
/// the end, each a code of one bit; `stored` as it is; and "hi" in fixed
/// codes, last.
fn short_end_then_stored(a: usize, stored: &[u8]) -> Vec<u8> {
    // The code length code: 0 as `0`, 1 as `10`, and 18 as `11`.
    let length = |bits: Bits, symbol: u32| match symbol {
        0 => bits.code(0, 1),
        1 => bits.code(2, 2),
        _ => bits.code(3, 2),
    };
    // Not last, dynamic: 257 literal/length codes, 1 distance code, and
    // the 18 code length code lengths, in their order, up to symbol 1's.
    let mut bits = Bits::default()
        .field(0, 1)
        .field(2, 2)
        .field(0, 5)
        .field(0, 5)
        .field(14, 4);
    for len in [0, 0, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2] {
        bits = bits.field(len, 3);
    }
    // 65 zeros, A of one bit, 190 zeros, the end of one bit, and no
    // distance code.
    bits = length(bits, 18).field(65 - 11, 7);
    bits = length(bits, 1);
    bits = length(bits, 18).field(138 - 11, 7);
    bits = length(bits, 18).field(52 - 11, 7);
    bits = length(bits, 1);
    bits = length(bits, 0);
    bits = (0..a).fold(bits, |bits, _| bits.code(0, 1)).code(1, 1);
    // Not last, stored: the rest of the byte, the length and its
    // complement, and the bytes.
    bits = bits.field(0, 1).field(0, 2);
    while !bits.count.is_multiple_of(8) {
        bits = bits.bit(0);
    }
    let len = stored.len() as u32;
    bits = bits.field(len, 16).field(!len & 0xFFFF, 16);
    bits = stored
        .iter()
        .fold(bits, |bits, &byte| bits.field(u32::from(byte), 8));
    // Last, fixed.
    bits.field(1, 1)
        .field(1, 2)
        .literal(b'h')
        .literal(b'i')
        .code(0, 7)
        .bytes
}

/// A stored block after a short end code, with the input well ahead, once
/// decoded otherwise than in pieces: the fast path left the next byte's
/// bits above those held, the stored block copied it straight from the
/// input, and the block after it read its header with them.
#[test]
fn reads_stored_blocks_after_short_end_codes() {
    let stored = [&[1, 2, 3, 0xFF][..], &[0x55; 60]].concat();
    for a in 0..64 {
        let stream = short_end_then_stored(a, &stored);
        let expected = [&b"A".repeat(a)[..], &stored, b"hi"].concat();
        let (data, consumed) = agree(&stream, 1).unwrap();
        assert!(data == expected, "{a} A's");
        assert_eq!(consumed, stream.len(), "{a} A's");
        assert_eq!(zlib(&stream), Some((expected, stream.len())), "{a} A's");
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

/// Runs, for which decoding goes round its buffer rather than move the
/// window: matches that then reach past the buffer's start, wholly or in
/// part, copied from the window left at its end; and checkpoints taken
/// there, which resume.
#[test]
fn goes_round_for_runs() {
    let period: Vec<u8> = (0..1000_u32).map(|n| (n * 7 % 251) as u8).collect();
    let data = period.repeat(200);
    let stream = miniz_oxide::deflate::compress_to_vec(&data, 6);
    for piece in [100, 4096] {
        let (decoded, consumed) = agree(&stream, piece).unwrap();
        assert!(decoded == data, "in pieces of {piece}");
        assert_eq!(consumed, stream.len());
    }
    assert_eq!(zlib(&stream), Some((data.clone(), stream.len())));
    let mut decoder = Decoder::new();
    let mut output = [0; 100];
    let (mut at, mut decoded, mut resumed) = (0, 0, Vec::new());
    loop {
        let (consumed, produced) = decoder.decode(&stream[at..], &mut output).unwrap();
        if (consumed, produced) == (0, 0) {
            break;
        }
        at += consumed;
        if decoded / 9973 != (decoded + produced) / 9973 {
            let checkpoint = decoder.checkpoint().unwrap();
            resumed.push((at - decoder.buffered(), decoded + produced, checkpoint));
        }
        decoded += produced;
    }
    decoder.finish().unwrap();
    assert_eq!(decoded, data.len());
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
        assert!(rest == data[len..], "from {from}");
    }
}

/// Writes the header of a dynamic block, last or not, whose codes'
/// lengths, of 0 or 1 alone, are `literals` and `distances`: in the code
/// length code, 0 as `0`, 1 as `10`, and 18, a run of zeros, as `11`.
fn dynamic(bits: Bits, last: bool, literals: &[u32], distances: &[u32]) -> Bits {
    let mut bits = bits
        .field(u32::from(last), 1)
        .field(2, 2)
        .field(literals.len() as u32 - 257, 5)
        .field(distances.len() as u32 - 1, 5)
        .field(14, 4);
    for len in [0, 0, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2] {
        bits = bits.field(len, 3);
    }
    let lengths = [literals, distances].concat();
    let mut at = 0;
    while at < lengths.len() {
        let zeros = lengths[at..].iter().take_while(|&&len| len == 0).count();
        if zeros >= 11 {
            let zeros = zeros.min(138);
            bits = bits.code(3, 2).field(zeros as u32 - 11, 7);
            at += zeros;
        } else {
            bits = match lengths[at] {
                0 => bits.code(0, 1),
                _ => bits.code(2, 2),
            };
            at += 1;
        }
    }
    bits
}

/// Blocks built by hand: one whose literal/length code is the end's
/// alone, of one bit, whose other bit is no code; and one of matches
/// alone, which has no literal to pair, after a literal, with more input
/// after it than pairing at once asks.
#[test]
fn decodes_blocks_of_one_or_two_codes() {
    let mut end = vec![0; 257];
    end[256] = 1;
    // With bytes after it, so that the bits tell it is no code.
    let invalid = dynamic(Bits::default(), true, &end, &[0]).code(1, 1);
    let invalid = [&invalid.bytes[..], &[0; 8]].concat();
    assert_eq!(kind(&invalid), Error::Corrupt);
    assert!(zlib(&invalid).is_none());
    let mut matches = vec![0; 286];
    (matches[256], matches[285]) = (1, 1);
    let first = Bits::default()
        .field(0, 1)
        .field(1, 2)
        .literal(b'a')
        .code(0, 7);
    let mut bits = dynamic(first, true, &matches, &[1]);
    for _ in 0..1000 {
        bits = bits.code(1, 1).code(0, 1);
    }
    let stream = bits.code(0, 1).bytes;
    let after = [&stream[..], &[7; 4096]].concat();
    let expected = b"a".repeat(1 + 258 * 1000);
    let (decoded, consumed) = decode(&after, 1 << 20).unwrap();
    assert!(decoded == expected);
    assert_eq!(consumed, stream.len());
    assert!(agree(&after, 13).unwrap() == (expected.clone(), stream.len()));
    assert!(zlib(&after) == Some((expected, stream.len())));
}

/// Writes a stored block, last or not, of `bytes`, from a byte's start.
fn stored(bits: Bits, last: bool, bytes: &[u8], nlen: u32) -> Bits {
    let mut bits = bits.field(u32::from(last), 1).field(0, 2);
    while !bits.count.is_multiple_of(8) {
        bits = bits.bit(0);
    }
    let len = bytes.len() as u32;
    bits = bits.field(len, 16).field(nlen, 16);
    bytes
        .iter()
        .fold(bits, |bits, &byte| bits.field(u32::from(byte), 8))
}

/// Stored blocks, straight to an output with room: a second whose length's
/// complement disagrees, read where the first is copied; one of more than
/// a window, of which a match after it copies; and one after codes that
/// went straight to the output, with less room left than a window.
#[test]
fn copies_stored_blocks_straight() {
    let first = stored(Bits::default(), false, b"hello", !5 & 0xFFFF);
    let bad = stored(first, true, b"world", 5);
    assert_eq!(decode(&bad.bytes, 1 << 20), Err(Error::Corrupt));
    assert!(zlib(&bad.bytes).is_none());
    let long: Vec<u8> = (0..40_000_u32).map(|n| (n * 7 % 251) as u8).collect();
    let block = stored(Bits::default(), false, &long, !40_000 & 0xFFFF);
    let stream = block
        .field(1, 1)
        .field(1, 2)
        .code(1, 7)
        .code(0, 5)
        .code(0, 7);
    let expected = [&long[..], &[long[39_999]; 3]].concat();
    let decoded = decode(&stream.bytes, 1 << 20).unwrap();
    assert!(decoded == (expected.clone(), stream.bytes.len()));
    assert!(agree(&stream.bytes, 13).unwrap() == (expected, stream.bytes.len()));
    let mut runs = Bits::default().field(0, 1).field(1, 2).literal(b'a');
    for _ in 0..400 {
        runs = runs.code(0xC5, 8).code(0, 5);
    }
    let tail: Vec<u8> = (0..1000_u32).map(|n| n as u8).collect();
    let stream = stored(runs.code(0, 7), true, &tail, !1000 & 0xFFFF).bytes;
    let expected = [&b"a".repeat(1 + 258 * 400)[..], &tail].concat();
    let mut decoder = Decoder::new();
    let mut output = vec![0; expected.len()];
    let (mut at, mut end) = (0, 0);
    while end < output.len() {
        let (consumed, produced) = decoder.decode(&stream[at..], &mut output[end..]).unwrap();
        assert!(consumed + produced > 0, "stuck at {at}");
        (at, end) = (at + consumed, end + produced);
    }
    decoder.finish().unwrap();
    assert!(output == expected);
    assert_eq!(zlib(&stream), Some((expected, stream.len())));
}

/// A stored block of more than a window, begun into little room, which a
/// first chunk's buffer holds, then copied straight into ample room until
/// the input runs out: the window keeps the latest of it, growing the
/// buffer first, which a match a window back then copies from.
#[test]
fn keeps_a_window_of_stored_blocks_cut_short() {
    let long: Vec<u8> = (0..60_000_u32).map(|n| (n * 7 % 251) as u8).collect();
    let block = stored(Bits::default(), false, &long, !60_000 & 0xFFFF);
    let stream = block
        .field(1, 1)
        .field(1, 2)
        .code(1, 7)
        .code(29, 5)
        .field(8191, 13)
        .code(0, 7)
        .bytes;
    let far = 60_000 - 32_768;
    let expected = [&long[..], &long[far..far + 3]].concat();
    let mut decoder = Decoder::new();
    let mut output = vec![0; expected.len()];
    let (mut at, mut end) = decoder.decode(&stream, &mut output[..1000]).unwrap();
    let cut = &stream[at..at + 40_000];
    let (consumed, produced) = decoder.decode(cut, &mut output[end..]).unwrap();
    (at, end) = (at + consumed, end + produced);
    while end < output.len() {
        let (consumed, produced) = decoder.decode(&stream[at..], &mut output[end..]).unwrap();
        assert!(consumed + produced > 0, "stuck at {at}");
        (at, end) = (at + consumed, end + produced);
    }
    assert_eq!(at, stream.len());
    decoder.finish().unwrap();
    assert!(output == expected);
    assert_eq!(zlib(&stream), Some((expected, stream.len())));
}

/// A stream cut short, decoded into ample room, where codes go straight
/// to the output once it holds a window: the input ends within a block,
/// and `finish` finds it cut short. And one whole in a byte of room: it
/// has ended, but not produced all it decoded.
#[test]
fn finishes_only_whole_and_produced() {
    let (_, stream, len, _) = FIXTURES[8];
    let cut = &stream[..stream.len() * 9 / 10];
    let mut decoder = Decoder::new();
    let mut output = vec![0; 1 << 20];
    let (_, produced) = decoder.decode(cut, &mut output).unwrap();
    assert!(produced > 32 << 10 && produced < len, "{produced}");
    assert_eq!(decoder.finish(), Err(Error::Corrupt));
    let good = Bits::fixed().literal(b'a').code(1, 7).code(0, 5).code(0, 7);
    let mut decoder = Decoder::default();
    let mut output = [0; 1];
    let (consumed, produced) = decoder.decode(&good.bytes, &mut output).unwrap();
    assert_eq!((consumed, produced), (good.bytes.len(), 1));
    assert_eq!(decoder.finish(), Err(Error::Corrupt));
    assert_eq!(Error::Corrupt.to_string(), "corrupt Deflate stream");
}

/// Checkpoints taken within dynamic blocks' headers past the first, given
/// a byte of input at a time, which go on to the same end.
#[test]
fn offers_checkpoints_within_block_headers() {
    let data: Vec<u8> = (0..200_000_u32)
        .map(|n| (n * 7 % 253) as u8 ^ (n >> 9) as u8)
        .collect();
    // Of two blocks, at level 1.
    let stream = miniz_oxide::deflate::compress_to_vec(&data, 1);
    let mut decoder = Decoder::new();
    let mut output = [0; 4096];
    let (mut at, mut decoded, mut resumed) = (0, Vec::new(), Vec::new());
    loop {
        let end = stream.len().min(at + 1);
        let (consumed, produced) = decoder.decode(&stream[at..end], &mut output).unwrap();
        if (consumed, produced) == (0, 0) {
            break;
        }
        at += consumed;
        decoded.extend_from_slice(&output[..produced]);
        let header = format!("{decoder:?}").contains("Dynamic");
        if header && !decoded.is_empty() && resumed.len() < 40 {
            resumed.push((at, decoded.len(), decoder.checkpoint().unwrap()));
        }
    }
    decoder.finish().unwrap();
    assert!(decoded == data);
    assert_eq!(resumed.len(), 40);
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
        assert!(rest == data[len..], "from {from}");
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

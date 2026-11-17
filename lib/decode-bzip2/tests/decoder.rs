// This is free and unencumbered software released into the public domain.

//! bzip2 streams, as bzip2 writes them.

mod common;

use common::{agree, agrees_with_bzip2, numbers};
use decode_bzip2::{Decoder, Error};

/// Decodes `stream` whole and in pieces, and checks that `bzip2` decodes
/// it alike, or refuses it too.
fn decode(stream: &[u8]) -> Result<Vec<u8>, Error> {
    let whole = agree(stream, 1);
    assert_eq!(whole.as_ref().ok(), agree(stream, 7).as_ref().ok());
    assert!(agrees_with_bzip2(stream, &whole), "disagrees with bzip2");
    whole
}

fn kind(stream: &[u8]) -> Error {
    decode(stream).unwrap_err()
}

/// What `runs.bz2` holds: 300,000 zeros, then `filer\n` 50,000 times.
fn runs() -> Vec<u8> {
    let mut runs = vec![0; 300_000];
    runs.extend(b"filer\n".repeat(50_000));
    runs
}

/// `printf '' | bzip2`: a stream of no blocks.
const EMPTY: [u8; 14] = [
    0x42, 0x5A, 0x68, 0x39, 0x17, 0x72, 0x45, 0x38, 0x50, 0x90, 0, 0, 0, 0,
];

/// The fixtures are by bzip2 1.0.8. `numbers.bz2` holds two streams, at
/// `-9` and `-1`; `runs.bz2`, one at `-2`, of two blocks, whose runs are
/// encoded twice over.
#[test]
fn decodes_what_bzip2_writes() {
    assert_eq!(
        decode(include_bytes!("data/numbers.bz2")).unwrap(),
        numbers()
    );
    // In pieces of a few dozen bytes, as a byte at a time takes long.
    let runs_bz2 = include_bytes!("data/runs.bz2");
    let decoded = agree(runs_bz2, 61);
    assert!(
        agrees_with_bzip2(runs_bz2, &decoded),
        "disagrees with bzip2"
    );
    assert_eq!(decoded.unwrap(), runs());
    assert_eq!(decode(&EMPTY).unwrap(), b"");
}

/// A block that ends with a run of four, which a count of none more
/// follows, as bzip2 writes Canterbury's `sum` and Silesia's `nci`, ends
/// though that count follows the byte that fills the room (DEC-16).
#[test]
fn ends_blocks_whose_last_byte_fills_the_room() {
    for data in [&b"aaaa"[..], b"filer: aaaa", b"bbbbbbbb", b"bbbbbbbbb"] {
        let level = bzip2::Compression::best();
        let mut encoder = bzip2::write::BzEncoder::new(Vec::new(), level);
        std::io::Write::write_all(&mut encoder, data).unwrap();
        let packed = encoder.finish().unwrap();
        let (mut decoder, mut output) = (Decoder::new(), vec![0; data.len()]);
        let (mut at, mut written) = (0, 0);
        loop {
            let (consumed, produced) = decoder
                .decode(&packed[at..], &mut output[written..])
                .unwrap();
            (at, written) = (at + consumed, written + produced);
            if (consumed, produced) == (0, 0) {
                break;
            }
        }
        assert_eq!(decoder.finish(), Ok(()), "{data:?}");
        assert_eq!(output, data);
    }
}

#[test]
fn ignores_what_follows_the_last_stream() {
    let numbers_bz2 = include_bytes!("data/numbers.bz2");
    for garbage in [&b"garbage"[..], b"BZ", b"BZh0", &[0; 100]] {
        let stream = [&numbers_bz2[..], garbage].concat();
        assert_eq!(decode(&stream).unwrap(), numbers(), "{garbage:?}");
    }
}

#[test]
fn refuses_malformed_streams() {
    let good = include_bytes!("data/numbers.bz2");
    let corrupt = |at: usize, mask: u8| {
        let mut stream = good.to_vec();
        stream[at] ^= mask;
        kind(&stream)
    };
    // The magic numbers, the first block's CRC, and the second stream's.
    assert_eq!(corrupt(0, 1), Error::Corrupt);
    assert_eq!(corrupt(4, 1), Error::Corrupt);
    assert_eq!(corrupt(10, 1), Error::Corrupt);
    assert_eq!(corrupt(good.len() - 2, 1), Error::Corrupt);
    // A randomized block, after its magic number and CRC.
    assert_eq!(corrupt(14, 0x80), Error::Unsupported);
    // A block larger than its stream says blocks are.
    let mut runs = include_bytes!("data/runs.bz2").to_vec();
    runs[3] = b'1';
    assert_eq!(kind(&runs), Error::Corrupt);
    // Ending early, anywhere but after the first stream, or within the
    // second's header, which is then garbage after it; or before any.
    let second = good.windows(4).rposition(|bytes| bytes == b"BZh1").unwrap();
    let ends = (0..good.len()).step_by(61).chain(second..good.len());
    for end in ends.filter(|end| !(second..second + 4).contains(end)) {
        assert_eq!(kind(&good[..end]), Error::Corrupt, "{end}");
    }
}

/// Bits written most significant first, as bzip2 packs them.
struct Bits(Vec<u8>, usize);

impl Bits {
    fn push(&mut self, value: u64, n: u32) {
        for i in (0..n).rev() {
            if self.1.is_multiple_of(8) {
                self.0.push(0);
            }
            *self.0.last_mut().unwrap() |= (((value >> i) & 1) as u8) << (7 - self.1 % 8);
            self.1 += 1;
        }
    }
}

/// Returns a stream of `len` bytes: a block whose first table's code
/// lengths never end, the first stepping up and down forever, two bits a
/// step, which bzip2 would read on without end.
fn endless_lengths(len: usize) -> Vec<u8> {
    let mut bits = Bits(Vec::new(), 0);
    bits.push(u64::from_be_bytes(*b"\0\0\0\0BZh1"), 32);
    bits.push(0x3141_5926_5359, 48);
    // The CRC, no randomization, the origin, byte 0 in use, two tables, one
    // selector, of the first, and its first length, 1.
    bits.push(0, 32);
    bits.push(0, 1);
    bits.push(0, 24);
    bits.push(0x8000, 16);
    bits.push(0x8000, 16);
    bits.push(2, 3);
    bits.push(1, 15);
    bits.push(0, 1);
    bits.push(1, 5);
    // Up, then down, forever: to the byte's end, then by whole bytes, as
    // the pattern's eight bits repeat from any offset.
    let pattern = [1, 0, 1, 1];
    let mut i = 0;
    while !bits.1.is_multiple_of(8) {
        bits.push(pattern[i % 4], 1);
        i += 1;
    }
    let byte = (0..8).fold(0, |byte, k| byte << 1 | pattern[(i + k) % 4] as u8);
    let mut stream = bits.0;
    stream.resize(len, byte);
    stream
}

/// Returns a stream of a block built by hand, of `a` alone: `run` of it,
/// 1, or 4 with no count after them, which bzip2 never writes; its codes,
/// two tables', chosen by `selectors` selectors of the first. Unless
/// `used`, the block says it uses no byte at all.
fn one_byte(run: usize, selectors: u64, used: bool) -> Vec<u8> {
    let crc = compute_crc::crc32_bzip2(&b"a".repeat(run));
    let mut bits = Bits(Vec::new(), 0);
    bits.push(u64::from_be_bytes(*b"\0\0\0\0BZh1"), 32);
    bits.push(0x3141_5926_5359, 48);
    // The CRC, no randomization, the origin, and the bytes in use: of the
    // seventh range of sixteen, the second, `a`.
    bits.push(u64::from(crc), 32);
    bits.push(0, 1);
    bits.push(0, 24);
    if used {
        bits.push(0x0200, 16);
        bits.push(0x4000, 16);
    } else {
        bits.push(0, 16);
    }
    bits.push(2, 3);
    bits.push(selectors, 15);
    for _ in 0..selectors {
        bits.push(0, 1);
    }
    // Each table's lengths of the symbols: runs of one, of two, and the
    // end: 1, then up a step to 2, and 2, so that their codes are 0, 10,
    // and 11.
    for _ in 0..2 {
        bits.push(1, 5);
        bits.push(0b01000, 5);
    }
    // A run of `run` as runs of one and of two by place, and the end.
    match run {
        1 => bits.push(0b0, 1),
        _ => bits.push(0b100, 3),
    }
    bits.push(0b11, 2);
    bits.push(0x1772_4538_5090, 48);
    bits.push(u64::from(crc), 32);
    bits.0
}

/// A block that ends on a run of four, with no count after it, which
/// bzip2 refuses as it reads one past the block's end.
#[test]
fn refuses_a_block_that_ends_on_a_run_of_four() {
    assert_eq!(decode(&one_byte(1, 2, true)).unwrap(), b"a");
    assert_eq!(kind(&one_byte(4, 2, true)), Error::Corrupt);
}

/// Blocks built by hand, which it decodes or refuses as bzip2 does: one
/// of more selectors than bzip2 keeps, the rest ignored, and one that
/// uses no byte.
#[test]
fn decodes_blocks_built_by_hand() {
    assert_eq!(decode(&one_byte(1, 18_010, true)).unwrap(), b"a");
    assert_eq!(kind(&one_byte(1, 2, false)), Error::Corrupt);
}

/// A decoder by default is a new one. The input may not end while a block
/// has bytes left to write, and a checkpoint is offered neither there nor
/// past the last stream. Its errors say what they mean, and it says where
/// it is.
#[test]
fn defaults_to_a_new_decoder() {
    let stream = [&include_bytes!("data/numbers.bz2")[..], b"garbage"].concat();
    let mut decoder = Decoder::default();
    let mut output = vec![0; 1 << 20];
    let (mut at, mut end) = decoder.decode(&stream, &mut output[..1]).unwrap();
    assert_eq!(end, 1);
    assert_eq!(decoder.finish(), Err(Error::Corrupt));
    assert!(decoder.checkpoint().is_none());
    loop {
        let (consumed, produced) = decoder.decode(&stream[at..], &mut output[end..]).unwrap();
        if (consumed, produced) == (0, 0) {
            break;
        }
        (at, end) = (at + consumed, end + produced);
    }
    decoder.finish().unwrap();
    assert_eq!(output[..end], numbers());
    assert!(decoder.checkpoint().is_none());
    assert!(format!("{decoder:?}").starts_with("Decoder"));
    assert_eq!(Error::Corrupt.to_string(), "corrupt bzip2 stream");
    assert_eq!(Error::Unsupported.to_string(), "unsupported bzip2 stream");
}

/// It gathers at most 4 MiB toward a block, more than any takes, and
/// refuses one that needs more rather than gather without bound.
#[test]
fn bounds_what_it_gathers_toward_a_block() {
    let stream = endless_lengths(5 << 20);
    let mut decoder = Decoder::new();
    let mut output = [0; 64];
    assert_eq!(decoder.decode(&stream, &mut output), Err(Error::Corrupt));
    assert_eq!(agree(&stream, 1 << 16).unwrap_err(), Error::Corrupt);
}

#[test]
fn offers_checkpoints_between_blocks() {
    // Two streams, of a block each: a checkpoint follows each block.
    let stream = include_bytes!("data/numbers.bz2");
    let mut decoder = Decoder::new();
    assert!(decoder.checkpoint().is_some(), "before any stream");
    let mut output = [0; 4096];
    let (mut at, mut decoded, mut resumed) = (0, Vec::new(), Vec::new());
    loop {
        let (consumed, produced) = decoder.decode(&stream[at..], &mut output).unwrap();
        if (consumed, produced) == (0, 0) {
            break;
        }
        at += consumed;
        decoded.extend_from_slice(&output[..produced]);
        if let Some(checkpoint) = decoder.checkpoint() {
            resumed.push((at - decoder.buffered(), decoded.len(), checkpoint));
        }
    }
    decoder.finish().unwrap();
    assert_eq!(decoded, numbers());
    assert!(resumed.len() >= 2, "{} checkpoints", resumed.len());
    // Each goes on from where it was taken to the same end.
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
        assert_eq!(rest, decoded[len..], "from {from}");
    }
}

/// Decodes each `.bz2` file in the directory `DECODE_BZIP2_CORPUS` names,
/// whole, as `bzip2` decodes it, and times it against `bzip2`, in a
/// release build.
#[test]
#[ignore = "needs a corpus of .bz2 files"]
fn decodes_a_corpus() {
    use std::{
        io::Read as _,
        time::{Duration, Instant},
    };
    let corpus = std::env::var("DECODE_BZIP2_CORPUS").unwrap();
    let mut paths: Vec<_> = std::fs::read_dir(corpus)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "bz2"))
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
        bzip2::read::MultiBzDecoder::new(&stream[..])
            .read_to_end(&mut theirs)
            .unwrap();
        times[1] += start.elapsed();
        assert!(output[..len] == theirs, "{}", path.display());
        bytes += len;
    }
    let [ours, theirs] = times;
    println!(
        "{} streams, {bytes} bytes: ours in {ours:?}, bzip2's in {theirs:?}",
        paths.len()
    );
}

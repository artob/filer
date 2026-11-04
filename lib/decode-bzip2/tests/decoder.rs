// This is free and unencumbered software released into the public domain.

//! bzip2 streams, as bzip2 writes them.

mod common;

use common::{agree, numbers};
use decode_bzip2::{Decoder, Error};

fn decode(stream: &[u8]) -> Result<Vec<u8>, Error> {
    let whole = agree(stream, 1);
    assert_eq!(whole.as_ref().ok(), agree(stream, 7).as_ref().ok());
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
    assert_eq!(agree(runs_bz2, 61).unwrap(), runs());
    assert_eq!(decode(&EMPTY).unwrap(), b"");
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

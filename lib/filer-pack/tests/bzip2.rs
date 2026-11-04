// This is free and unencumbered software released into the public domain.

//! bzip2 streams, as bzip2 writes them.

#![cfg(feature = "bzip2")]

mod common;

use common::{With, agree, numbers, unpack};
use filer_api::{Error, ErrorKind};
use filer_pack::Bzip2;

fn decode(stream: &[u8]) -> Result<Vec<u8>, Error> {
    let whole = agree(Bzip2::new, stream, 1);
    assert_eq!(
        whole.as_ref().ok(),
        agree(Bzip2::new, stream, 7).as_ref().ok()
    );
    whole
}

fn kind(stream: &[u8]) -> ErrorKind {
    decode(stream).unwrap_err().kind()
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
    assert_eq!(agree(Bzip2::new, runs_bz2, 61).unwrap(), runs());
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
    assert_eq!(corrupt(0, 1), ErrorKind::Corrupt);
    assert_eq!(corrupt(4, 1), ErrorKind::Corrupt);
    assert_eq!(corrupt(10, 1), ErrorKind::Corrupt);
    assert_eq!(corrupt(good.len() - 2, 1), ErrorKind::Corrupt);
    // A randomized block, after its magic number and CRC.
    assert_eq!(corrupt(14, 0x80), ErrorKind::Unsupported);
    // A block larger than its stream says blocks are.
    let mut runs = include_bytes!("data/runs.bz2").to_vec();
    runs[3] = b'1';
    assert_eq!(kind(&runs), ErrorKind::Corrupt);
    // Ending early, anywhere but after the first stream, or within the
    // second's header, which is then garbage after it; or before any.
    let second = good.windows(4).rposition(|bytes| bytes == b"BZh1").unwrap();
    let ends = (0..good.len()).step_by(61).chain(second..good.len());
    for end in ends.filter(|end| !(second..second + 4).contains(end)) {
        assert_eq!(kind(&good[..end]), ErrorKind::Corrupt, "{end}");
    }
}

#[test]
fn checks_the_crc_after_the_declared_size() {
    let read = |stream, size| unpack(stream, size, With(|| Box::new(Bzip2::new())));
    let stream = include_bytes!("data/numbers.bz2").to_vec();
    let numbers = numbers();
    assert_eq!(read(stream.clone(), numbers.len()).unwrap(), numbers);
    let error = read(stream.clone(), numbers.len() - 1).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Corrupt);
    // The last stream's CRC, past all it decodes to.
    let mut bad = stream;
    let at = bad.len() - 2;
    bad[at] ^= 1;
    let error = read(bad, numbers.len()).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Corrupt);
}

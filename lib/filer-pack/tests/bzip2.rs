// This is free and unencumbered software released into the public domain.

//! bzip2 streams through `decode-bzip2`'s decoder, as `Unpacked` reads
//! them; the decoder's own tests are that crate's.

#![cfg(feature = "bzip2")]

mod common;

use common::{Counting, PRODUCED, With, agree, numbers, unpack};
use core::sync::atomic::Ordering;
use filer_api::{BufResult, Error, ErrorKind, ReadAt};
use filer_pack::{Bzip2, Unpacked};
use filer_spec::block_on;

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

/// The fixture is by bzip2 1.0.8: `numbers.bz2` holds two streams, at
/// `-9` and `-1`.
#[test]
fn decodes_what_bzip2_writes() {
    assert_eq!(
        decode(include_bytes!("data/numbers.bz2")).unwrap(),
        numbers()
    );
    // A randomized block, after its magic number and CRC.
    let mut randomized = include_bytes!("data/numbers.bz2").to_vec();
    randomized[14] ^= 0x80;
    assert_eq!(kind(&randomized), ErrorKind::Unsupported);
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

/// `blocks.bz2` is by bzip2 1.0.8 at `-9`: 12 MiB of a pattern, in 14
/// blocks.
#[test]
fn resumes_between_blocks() {
    let data: Vec<u8> = (0..12_u32 << 20)
        .map(|n| ((n % 251) ^ (n >> 20)) as u8)
        .collect();
    let stream = include_bytes!("data/blocks.bz2").to_vec();
    let len = stream.len() as u64;
    let codec = With(|| Box::new(Counting(Bzip2::new())));
    let unpacked = block_on(Unpacked::measured(stream, 0..len, codec, 12 << 20)).unwrap();
    // Each read decodes on from the checkpoint before it: at most 4 MiB
    // back, and the rest of a block of 900,000 bytes.
    for at in [(12 << 20) - 1000, (6 << 20) + 7, (11 << 20) + 3] {
        let before = PRODUCED.load(Ordering::Relaxed);
        let read = unpacked.read_exact_at(Vec::with_capacity(100), at);
        let BufResult(result, bytes) = block_on(read);
        result.unwrap();
        assert_eq!(bytes, data[at as usize..at as usize + 100], "{at}");
        let decoded = PRODUCED.load(Ordering::Relaxed) - before;
        assert!(
            decoded <= (4 << 20) + 1_000_000,
            "{decoded} bytes decoded at {at}"
        );
    }
}

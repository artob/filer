// This is free and unencumbered software released into the public domain.

//! xz streams, and raw LZMA and LZMA2 streams, as xz writes them.

#![cfg(feature = "xz")]

mod common;

use common::{Counting, PRODUCED, With, agree, numbers, unpack};
use core::sync::atomic::Ordering;
use filer_api::{BufResult, Error, ErrorKind, ReadAt, checksum::crc32};
use filer_pack::{Decoder, Lzma, Lzma2, Unpacked, Xz};
use filer_spec::block_on;

fn decode<D: Decoder>(new: impl Fn() -> D, stream: &[u8]) -> Result<Vec<u8>, Error> {
    let whole = agree(&new, stream, 1);
    assert_eq!(whole.as_ref().ok(), agree(&new, stream, 7).as_ref().ok());
    whole
}

fn kind(stream: &[u8]) -> ErrorKind {
    decode(Xz::new, stream).unwrap_err().kind()
}

/// `printf hello | xz -0 -C crc32`.
const HELLO: [u8; 64] = [
    0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00, 0x00, 0x01, 0x69, 0x22, 0xDE, 0x36, 0x03, 0xC0, 0x09, 0x05,
    0x21, 0x01, 0x0C, 0x00, 0x00, 0x00, 0x00, 0x00, 0x6D, 0xFF, 0x09, 0xFF, 0x01, 0x00, 0x04, 0x68,
    0x65, 0x6C, 0x6C, 0x6F, 0x00, 0x00, 0x00, 0x00, 0x86, 0xA6, 0x10, 0x36, 0x00, 0x01, 0x1D, 0x05,
    0xB8, 0x2D, 0x80, 0xAF, 0x90, 0x42, 0x99, 0x0D, 0x01, 0x00, 0x00, 0x00, 0x00, 0x01, 0x59, 0x5A,
];

/// The fixtures are by xz 5.8.4. `numbers.xz` holds two streams, at `-6`
/// with SHA-256 and at `-0` with CRC-32; `checks.xz`, a stream with each
/// check, padding, and one through the Delta filter.
#[test]
fn decodes_what_xz_writes() {
    let numbers_xz = include_bytes!("data/numbers.xz");
    assert_eq!(decode(Xz::new, numbers_xz).unwrap(), numbers());
    let checks = include_bytes!("data/checks.xz");
    let decoded = decode(Xz::new, checks).unwrap();
    assert_eq!(decoded, b"crc32\ncrc64\nsha256\nnone\ndelta\n");
}

/// `numbers.lzma` is by `xz --format=lzma -6`: a header, of the
/// properties and an unknown size, then a raw stream with an end marker;
/// `numbers.lzma2`, by `xz --format=raw --lzma2=preset=6`.
#[test]
fn decodes_raw_streams() {
    let numbers = numbers();
    let lzma = include_bytes!("data/numbers.lzma");
    let properties = lzma[..5].try_into().unwrap();
    let unknown = || Lzma::new(properties, None).unwrap();
    assert_eq!(decode(unknown, &lzma[13..]).unwrap(), numbers);
    // Of a known size, it ends there, before the end marker.
    let size = Some(numbers.len() as u64);
    let known = || Lzma::new(properties, size).unwrap();
    assert_eq!(decode(known, &lzma[13..]).unwrap(), numbers);
    // A dictionary of 8 MiB.
    let lzma2 = include_bytes!("data/numbers.lzma2");
    assert_eq!(decode(|| Lzma2::new(22).unwrap(), lzma2).unwrap(), numbers);
}

#[test]
fn refuses_what_it_cannot_decode() {
    // Properties past `lc` 8, `lp` 4, and `pb` 4, and dictionaries over
    // 128 MiB, unless the size bounds them.
    let lzma = |properties, size| Lzma::new(properties, size).unwrap_err().kind();
    assert_eq!(lzma([225, 0, 0, 1, 0], None), ErrorKind::Corrupt);
    let large = [0x5D, 0, 0, 0, 0x10];
    assert_eq!(lzma(large, None), ErrorKind::Unsupported);
    assert!(Lzma::new(large, Some(1 << 20)).is_ok());
    let lzma2 = |property| Lzma2::new(property).unwrap_err().kind();
    assert_eq!(lzma2(41), ErrorKind::Corrupt);
    assert_eq!(lzma2(31), ErrorKind::Unsupported);
    assert!(Lzma2::new(30).is_ok());
    // A block whose dictionary is 256 MiB, its header resealed.
    let mut large = HELLO;
    large[18] = 32;
    let crc = crc32(&large[12..24]);
    large[24..28].copy_from_slice(&crc.to_le_bytes());
    assert_eq!(
        decode(Xz::new, &large).unwrap_err().kind(),
        ErrorKind::Unsupported
    );
}

#[test]
fn refuses_malformed_streams() {
    assert_eq!(decode(Xz::new, &HELLO).unwrap(), b"hello");
    let corrupt = |at: usize| {
        let mut stream = HELLO;
        stream[at] ^= 1;
        kind(&stream)
    };
    // The magic number, the block header, the data, its CRC-32, the
    // index, and the footer.
    for at in [0, 13, 33, 41, 46, 63] {
        assert_eq!(corrupt(at), ErrorKind::Corrupt, "{at}");
    }
    // Padding after a stream comes in fours, of zeros; and what follows
    // the last stream must be another.
    let padded = [&HELLO[..], &[0; 8]].concat();
    assert_eq!(decode(Xz::new, &padded).unwrap(), b"hello");
    for after in [&[0; 3][..], b"garbage!"] {
        assert_eq!(kind(&[&HELLO[..], after].concat()), ErrorKind::Corrupt);
    }
    // Ending early, anywhere.
    for end in 0..HELLO.len() {
        assert_eq!(kind(&HELLO[..end]), ErrorKind::Corrupt, "{end}");
    }
}

/// `blocks.xz` is by xz 5.8.4 at `-6` with blocks of 1 MiB: 12 MiB of a
/// pattern, in 12 blocks.
#[test]
fn resumes_between_blocks() {
    let data: Vec<u8> = (0..12_u32 << 20)
        .map(|n| ((n % 251) ^ (n >> 20)) as u8)
        .collect();
    let stream = include_bytes!("data/blocks.xz").to_vec();
    let len = stream.len() as u64;
    let codec = With(|| Box::new(Counting(Xz::new())));
    let unpacked = block_on(Unpacked::measured(stream, 0..len, codec, 12 << 20)).unwrap();
    // Each read decodes on from the checkpoint before it: at most 4 MiB
    // back, and the rest of a block of 1 MiB.
    for at in [(12 << 20) - 1000, (6 << 20) + 7, (11 << 20) + 3] {
        let before = PRODUCED.load(Ordering::Relaxed);
        let read = unpacked.read_exact_at(Vec::with_capacity(100), at);
        let BufResult(result, bytes) = block_on(read);
        result.unwrap();
        assert_eq!(bytes, data[at as usize..at as usize + 100], "{at}");
        let decoded = PRODUCED.load(Ordering::Relaxed) - before;
        assert!(
            decoded <= (4 << 20) + (1 << 20),
            "{decoded} bytes decoded at {at}"
        );
    }
}

#[test]
fn checks_the_index_after_the_declared_size() {
    let read = |stream, size| unpack(stream, size, With(|| Box::new(Xz::new())));
    let stream = include_bytes!("data/numbers.xz").to_vec();
    let numbers = numbers();
    assert_eq!(read(stream.clone(), numbers.len()).unwrap(), numbers);
    let error = read(stream.clone(), numbers.len() - 1).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Corrupt);
    // The last stream's footer, past all it decodes to.
    let mut bad = stream;
    let last = bad.len() - 1;
    bad[last] ^= 1;
    let error = read(bad, numbers.len()).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Corrupt);
}

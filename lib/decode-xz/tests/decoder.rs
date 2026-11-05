// This is free and unencumbered software released into the public domain.

//! xz files as xz writes them, and as `lzma-rust2`'s encoder writes them
//! with every filter and check, each checked against `lzma-rust2`'s
//! decoder.

mod common;

use common::{agree, crc32, numbers, seq, theirs};
use decode_xz::{Decoder, Error};
use lzma_rust2::{CheckType, XzOptions, XzWriter, filter::FilterType};
use std::{io::Write, num::NonZeroU64};

/// Decodes `stream` in pieces of 1 and 7 bytes and whole, and returns what
/// it decodes to and consumes.
fn decode(stream: &[u8]) -> common::Decoded {
    let whole = agree(stream, 1);
    assert_eq!(whole, agree(stream, 7));
    whole
}

/// The fixtures are by xz 5.8.4. `numbers.xz` holds two streams, at `-6`
/// with SHA-256 and at `-0` with CRC-32; `checks.xz`, a stream with each
/// check, padding, and one through the Delta filter; `blocks.xz`, `seq 1
/// 30000` at `-9e` in blocks of 20,000 bytes, with SHA-256; and
/// `filters.xz`, `code.bin` through each branch converter, Delta, start
/// offsets, and a chain of two, a stream each, with CRC-64.
#[test]
fn decodes_what_xz_writes() {
    for (name, stream, expected) in [
        ("numbers", &include_bytes!("data/numbers.xz")[..], numbers()),
        (
            "checks",
            include_bytes!("data/checks.xz"),
            b"crc32\ncrc64\nsha256\nnone\ndelta\n".to_vec(),
        ),
        ("blocks", include_bytes!("data/blocks.xz"), seq(30000)),
        (
            "filters",
            include_bytes!("data/filters.xz"),
            include_bytes!("data/code.bin").repeat(11),
        ),
    ] {
        let (data, consumed) = decode(stream).unwrap();
        assert!(data == expected, "{name}");
        assert_eq!(consumed, stream.len(), "{name}");
        // `lzma-rust2` refuses the last stream of `filters.xz`, of two
        // filters, which xz writes and decodes; each stream's CRC-64 checks
        // what this crate decodes.
        if name != "filters" {
            assert_eq!(theirs(stream).as_ref(), Some(&expected), "{name}");
        }
    }
}

/// Returns `data` as xz by `lzma-rust2`'s encoder, with `filter`, if any,
/// and its `property`, `check`, and blocks of `block` bytes, if given.
fn xz(
    data: &[u8],
    filter: Option<(FilterType, u32)>,
    check: CheckType,
    block: Option<u64>,
) -> Vec<u8> {
    chained(data, filter.as_slice(), check, block)
}

/// Returns `data` as xz by `lzma-rust2`'s encoder, through `filters`, each
/// with its property, in the order the encoder applies them.
fn chained(
    data: &[u8],
    filters: &[(FilterType, u32)],
    check: CheckType,
    block: Option<u64>,
) -> Vec<u8> {
    let mut options = XzOptions::with_preset(6);
    for &(filter, property) in filters.iter().rev() {
        options.prepend_pre_filter(filter, property);
    }
    options.set_check_sum_type(check);
    options.set_block_size(block.and_then(NonZeroU64::new));
    let mut writer = XzWriter::new(Vec::new(), options).unwrap();
    writer.write_all(data).unwrap();
    writer.finish().unwrap()
}

#[test]
fn decodes_every_filter_and_check() {
    let code = include_bytes!("data/code.bin");
    let data: Vec<u8> = (0..12)
        .flat_map(|n| [&code[n * 97..], &code[..n * 97]].concat())
        .collect();
    let filters = [
        None,
        Some((FilterType::Delta, 1)),
        Some((FilterType::Delta, 256)),
        Some((FilterType::BcjX86, 0)),
        Some((FilterType::BcjX86, 7)),
        Some((FilterType::BcjPpc, 0)),
        Some((FilterType::BcjIa64, 0)),
        Some((FilterType::BcjArm, 0)),
        Some((FilterType::BcjArmThumb, 0)),
        Some((FilterType::BcjSparc, 0)),
        Some((FilterType::BcjArm64, 0)),
        Some((FilterType::BcjArm64, 1 << 20)),
        Some((FilterType::BcjRiscv, 0)),
    ];
    for filter in filters {
        for (check, block) in [
            (CheckType::Crc64, None),
            (CheckType::Crc32, Some(30_000)),
            (CheckType::Sha256, Some(4096)),
            (CheckType::None, Some(70_000)),
        ] {
            let stream = xz(&data, filter, check, block);
            let name = format!("{filter:?} {check:?} {block:?}");
            let (decoded, consumed) = agree(&stream, 4093).unwrap();
            assert!(decoded == data, "{name}");
            assert_eq!(consumed, stream.len(), "{name}");
            assert!(theirs(&stream).as_ref() == Some(&data), "{name}");
        }
    }
}

/// A stage of the filter chain holds 64 KiB: past that, what a filter has
/// yet to convert stays while what it has converted moves on, however few
/// bytes a call takes.
#[test]
fn converts_more_than_a_stage_holds() {
    let code = include_bytes!("data/code.bin");
    let data: Vec<u8> = (0..60)
        .flat_map(|n| [&code[n * 61..], &code[..n * 61]].concat())
        .collect();
    assert!(data.len() > 3 * 64 * 1024);
    for filters in [
        &[(FilterType::BcjX86, 0)][..],
        &[(FilterType::Delta, 1), (FilterType::BcjX86, 0)],
        &[(FilterType::BcjArm, 0), (FilterType::Delta, 3)],
    ] {
        let stream = chained(&data, filters, CheckType::Crc32, None);
        for piece in [1, 7, 4093] {
            let (decoded, consumed) = agree(&stream, piece).unwrap();
            assert!(decoded == data, "{filters:?} in pieces of {piece}");
            assert_eq!(consumed, stream.len(), "{filters:?}");
        }
    }
}

#[test]
fn takes_padding_and_streams_in_turn() {
    let one = xz(b"one, ", None, CheckType::Crc64, None);
    let two = xz(b"two", None, CheckType::Crc32, None);
    let stream = [&one[..], &[0; 8], &two, &[0; 4]].concat();
    assert_eq!(
        decode(&stream).unwrap(),
        (b"one, two".to_vec(), stream.len())
    );
    // Padding of other than a multiple of four, or other than zeros.
    for after in [&[0_u8; 3][..], &[0; 5], &[1], &[0, 0, 0, 0, 7]] {
        let stream = [&one[..], after].concat();
        assert_eq!(decode(&stream), Err(Error::Corrupt), "{after:?}");
    }
    // No stream at all.
    assert_eq!(decode(&[]), Err(Error::Corrupt));
    assert_eq!(decode(&[0; 4]), Err(Error::Corrupt));
}

#[test]
fn refuses_malformed_streams() {
    let stream = include_bytes!("data/numbers.xz");
    // Ending early, anywhere but after its first stream, at 1300.
    for end in (0..stream.len()).step_by(13).filter(|&end| end != 1300) {
        assert_eq!(decode(&stream[..end]), Err(Error::Corrupt), "{end}");
    }
    // Any byte changed, which a check, a size, or the index catches.
    let blocks = include_bytes!("data/blocks.xz");
    for at in (0..blocks.len()).step_by(61) {
        let mut bad = blocks.to_vec();
        bad[at] ^= 0x20;
        assert!(decode(&bad).is_err(), "{at}");
    }
}

/// Returns `stream`, a single block's, with its block header's first
/// filter ID set to `id`, and the header's CRC32 made to match.
fn with_filter(stream: &[u8], id: u8) -> Vec<u8> {
    let mut stream = stream.to_vec();
    let size = (usize::from(stream[12]) + 1) * 4;
    let header = 12..12 + size;
    stream[14] = id;
    let crc = crc32(&stream[header.start..header.end - 4]);
    stream[header.end - 4..header.end].copy_from_slice(&crc.to_le_bytes());
    stream
}

#[test]
fn refuses_what_it_cannot_decode() {
    let stream = xz(
        b"data",
        Some((FilterType::BcjX86, 0)),
        CheckType::Crc64,
        None,
    );
    assert_eq!(stream[14], 0x04, "the x86 filter's ID");
    // A filter it does not know, and LZMA2 other than last.
    assert_eq!(decode(&with_filter(&stream, 0x30)), Err(Error::Unsupported));
    assert_eq!(decode(&with_filter(&stream, 0x21)), Err(Error::Corrupt));
}

#[test]
fn offers_checkpoints_between_blocks() {
    let stream = include_bytes!("data/blocks.xz");
    let expected = seq(30000);
    let mut decoder = Decoder::new();
    let mut output = [0; 1000];
    let (mut at, mut decoded, mut resumed) = (0, Vec::new(), Vec::new());
    loop {
        let end = stream.len().min(at + 37);
        let (consumed, produced) = decoder.decode(&stream[at..end], &mut output).unwrap();
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
    assert!(decoded == expected);
    // One at each of the nine blocks, at least, and past the stream.
    assert!(resumed.len() >= 10, "{} checkpoints", resumed.len());
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

/// Decodes each `.xz` file in the directory `DECODE_XZ_CORPUS`, and
/// times it against `lzma-rust2`, in a release build.
#[test]
#[ignore = "needs a corpus of .xz files"]
fn decodes_a_corpus() {
    use std::time::{Duration, Instant};
    let corpus = std::env::var("DECODE_XZ_CORPUS").unwrap();
    let mut paths: Vec<_> = std::fs::read_dir(corpus)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "xz"))
        .collect();
    paths.sort();
    let (mut bytes, mut times) = (0, [Duration::ZERO; 2]);
    for path in &paths {
        let stream = std::fs::read(path).unwrap();
        let mut output = vec![0; 1 << 30];
        let start = Instant::now();
        let mut decoder = Decoder::new();
        let (mut at, mut len) = (0, 0);
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
        let mut theirs = lzma_rust2::XzStream::new_mem_limit(true, u32::MAX);
        let mut out = vec![0; len + 1];
        let mut done = (0, 0);
        loop {
            let action = if done.0 == stream.len() {
                lzma_rust2::Action::Finish
            } else {
                lzma_rust2::Action::Run
            };
            let result = theirs
                .process(&stream[done.0..], &mut out[done.1..], action)
                .unwrap();
            done = (
                done.0 + result.bytes_consumed,
                done.1 + result.bytes_produced,
            );
            if result.status == lzma_rust2::Status::StreamEnd {
                break;
            }
        }
        times[1] += start.elapsed();
        assert!(output[..len] == out[..done.1], "{}", path.display());
        bytes += len;
    }
    let [ours, theirs] = times;
    println!(
        "{} streams, {bytes} bytes: ours in {ours:?}, lzma-rust2's in {theirs:?}",
        paths.len()
    );
}

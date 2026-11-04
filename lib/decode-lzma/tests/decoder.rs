// This is free and unencumbered software released into the public domain.

//! LZMA and LZMA2 streams, as xz writes them and as `lzma-rust2`'s encoder
//! writes them in every variant, each checked against `lzma-rust2`'s
//! decoder.

mod common;

use common::{Lzma, agree, lzma, lzma2, noise, numbers, options, text, theirs_lzma, theirs_lzma2};
use decode_lzma::{Decoder, Error};
use lzma_rust2::Lzma2Options;
use std::num::NonZeroU64;

/// Decodes LZMA `stream` with `properties`, of `size` if known, in pieces
/// of 1 and 13 bytes and whole, and returns what it decodes to and
/// consumes.
fn decode_lzma(stream: &[u8], properties: [u8; 5], size: Option<u64>) -> common::Decoded {
    let new = || Decoder::lzma(properties, size);
    let whole = agree(new, stream, 1);
    assert_eq!(whole, agree(new, stream, 13));
    whole
}

/// Decodes LZMA2 `stream` with the dictionary of `property`, as
/// `decode_lzma` does.
fn decode_lzma2(stream: &[u8], property: u8) -> common::Decoded {
    let new = || Decoder::lzma2(property);
    let whole = agree(new, stream, 1);
    assert_eq!(whole, agree(new, stream, 13));
    whole
}

/// `numbers.lzma` is by `xz --format=lzma -6`: a header, of the
/// properties and an unknown size, then a raw stream with an end marker;
/// `numbers.lzma2`, by `xz --format=raw --lzma2=preset=6`.
#[test]
fn decodes_what_xz_writes() {
    let numbers = numbers();
    let lzma = include_bytes!("data/numbers.lzma");
    let (properties, stream) = (lzma[..5].try_into().unwrap(), &lzma[13..]);
    let decoded = decode_lzma(stream, properties, None).unwrap();
    assert_eq!(decoded, (numbers.clone(), stream.len()));
    // Of a known size, the end marker may follow.
    let size = Some(numbers.len() as u64);
    let decoded = decode_lzma(stream, properties, size).unwrap();
    assert_eq!(decoded, (numbers.clone(), stream.len()));
    // A dictionary of 8 MiB.
    let lzma2 = include_bytes!("data/numbers.lzma2");
    let decoded = decode_lzma2(lzma2, 22).unwrap();
    assert_eq!(decoded, (numbers, lzma2.len()));
}

#[test]
fn decodes_lzma_in_every_variant() {
    let data = [text(150_000, 1), noise(5000, 2), text(50_000, 3)].concat();
    // Presets, and properties: lc, lp, and pb.
    for (preset, lc, lp, pb) in [
        (0, 3, 0, 2),
        (1, 0, 0, 0),
        (6, 3, 0, 2),
        (6, 0, 4, 4),
        (6, 4, 0, 0),
        (6, 1, 2, 3),
    ] {
        for dictionary in [4096, 1 << 16, 1 << 22] {
            let options = options(preset, lc, lp, pb, dictionary);
            let name = format!("{preset} {lc} {lp} {pb} {dictionary}");
            let size = Some(data.len() as u64);
            // Of a known size, without an end marker, and with one.
            let Lzma { stream, properties } = lzma(&data, &options, false);
            let decoded = decode_lzma(&stream, properties, size).unwrap();
            assert!(decoded == (data.clone(), stream.len()), "{name}");
            let theirs = theirs_lzma(&stream, properties, size).unwrap();
            assert!(theirs.0 == data, "{name}");
            let Lzma { stream, properties } = lzma(&data, &options, true);
            for size in [size, None] {
                let decoded = decode_lzma(&stream, properties, size).unwrap();
                assert!(decoded == (data.clone(), stream.len()), "{name}");
            }
        }
    }
}

#[test]
fn decodes_lzma2_in_every_variant() {
    let data = [
        text(300_000, 4),
        noise(100_000, 5),
        text(100_000, 6),
        noise(3, 7),
        text(10, 8),
    ]
    .concat();
    for preset in [0, 1, 6] {
        for chunk in [None, Some(1 << 16), Some(1000)] {
            let mut options = Lzma2Options::with_preset(preset);
            options.set_chunk_size(chunk.and_then(NonZeroU64::new));
            let (stream, property) = lzma2(&data, options);
            let decoded = decode_lzma2(&stream, property).unwrap();
            assert!(
                decoded == (data.clone(), stream.len()),
                "{preset} {chunk:?}"
            );
            let theirs = theirs_lzma2(&stream, property).unwrap();
            assert!(theirs == (data.clone(), stream.len()), "{preset} {chunk:?}");
        }
    }
}

#[test]
fn decodes_empty_streams() {
    let options = options(6, 3, 0, 2, 1 << 16);
    let Lzma { stream, properties } = lzma(b"", &options, true);
    assert_eq!(
        decode_lzma(&stream, properties, None).unwrap(),
        (vec![], stream.len())
    );
    let Lzma { stream, properties } = lzma(b"", &options, false);
    assert_eq!(
        decode_lzma(&stream, properties, Some(0)).unwrap(),
        (vec![], stream.len())
    );
    assert_eq!(decode_lzma2(&[0], 0).unwrap(), (vec![], 1));
}

#[test]
fn consumes_no_byte_past_the_end() {
    let data = text(10_000, 9);
    let options = options(6, 3, 0, 2, 1 << 16);
    let Lzma { stream, properties } = lzma(&data, &options, true);
    let after = [&stream[..], b"after"].concat();
    let decoded = decode_lzma(&after, properties, None).unwrap();
    assert_eq!(decoded, (data.clone(), stream.len()));
    let (stream, property) = lzma2(&data, Lzma2Options::with_preset(6));
    let after = [&stream[..], b"after"].concat();
    assert_eq!(
        decode_lzma2(&after, property).unwrap(),
        (data, stream.len())
    );
}

#[test]
fn refuses_malformed_streams() {
    let data = text(20_000, 10);
    let options = options(6, 3, 0, 2, 1 << 16);
    let Lzma { stream, properties } = lzma(&data, &options, true);
    let size = Some(data.len() as u64);
    // Ending early, anywhere.
    for end in (0..stream.len()).step_by(97) {
        assert_eq!(
            decode_lzma(&stream[..end], properties, None),
            Err(Error::Corrupt),
            "{end}"
        );
    }
    // Data that does not begin with a zero.
    let mut bad = stream.clone();
    bad[0] = 1;
    assert_eq!(decode_lzma(&bad, properties, None), Err(Error::Corrupt));
    // A size short of the data, which then must end.
    let short = Some(data.len() as u64 - 100);
    assert_eq!(decode_lzma(&stream, properties, short), Err(Error::Corrupt));
    // An end marker before the size.
    let long = size.map(|size| size + 1);
    assert_eq!(decode_lzma(&stream, properties, long), Err(Error::Corrupt));
    // Properties past their range, and dictionaries too large.
    let lzma = |properties, size| Decoder::lzma(properties, size).unwrap_err();
    assert_eq!(lzma([225, 0, 0, 1, 0], None), Error::Corrupt);
    assert_eq!(lzma([0x5D, 0, 0, 0, 0x10], None), Error::Unsupported);
    assert!(Decoder::lzma([0x5D, 0, 0, 0, 0x10], Some(1 << 20)).is_ok());
    assert_eq!(Decoder::lzma2(41).unwrap_err(), Error::Corrupt);
    assert_eq!(Decoder::lzma2(40).unwrap_err(), Error::Unsupported);
    // 192 MiB, and 128 MiB.
    assert_eq!(Decoder::lzma2(31).unwrap_err(), Error::Unsupported);
    assert!(Decoder::lzma2(30).is_ok());
}

#[test]
fn refuses_lzma2_chunks_out_of_order() {
    let (stream, property) = lzma2(&text(5000, 11), Lzma2Options::with_preset(6));
    // The first chunk resets the dictionary, and sets the properties.
    assert_eq!(stream[0], 0xE0);
    for control in [0x02, 0x03, 0x7F, 0x80, 0xA0, 0xC0] {
        let mut bad = stream.clone();
        bad[0] = control;
        assert_eq!(
            decode_lzma2(&bad, property),
            Err(Error::Corrupt),
            "{control}"
        );
    }
    // Properties with more than 4 bits of literal context.
    let mut bad = stream.clone();
    bad[5] = 9 * 5 + 1;
    assert_eq!(decode_lzma2(&bad, property), Err(Error::Corrupt));
    // A stored chunk resetting the dictionary must be followed by a coded
    // chunk that sets the properties.
    let stored = [1, 0, 1, b'h', b'i'];
    let mut after = stored.to_vec();
    after.extend_from_slice(&stream);
    after[5] = 0xA0;
    assert_eq!(decode_lzma2(&after, property), Err(Error::Corrupt));
    let mut after = stored.to_vec();
    after.extend_from_slice(&stream);
    let decoded = decode_lzma2(&after, property).unwrap().0;
    assert!(decoded.starts_with(b"hi"));
    // A chunk's packed size larger or smaller than its data.
    for delta in [1_i32, -1] {
        let mut bad = stream.clone();
        let packed = u16::from_be_bytes([bad[3], bad[4]]) as i32 + delta;
        [bad[3], bad[4]] = (packed as u16).to_be_bytes();
        assert_eq!(decode_lzma2(&bad, property), Err(Error::Corrupt), "{delta}");
    }
}

#[test]
fn offers_checkpoints_anywhere() {
    let data = [text(200_000, 12), noise(20_000, 13), text(80_000, 14)].concat();
    let mut options = Lzma2Options::with_preset(6);
    options.set_chunk_size(NonZeroU64::new(1 << 16));
    let (stream, property) = lzma2(&data, options);
    let mut decoder = Decoder::lzma2(property).unwrap();
    let mut output = [0; 1000];
    let (mut at, mut decoded, mut resumed) = (0, Vec::new(), Vec::new());
    for call in 0.. {
        let end = stream.len().min(at + 37);
        let (consumed, produced) = decoder.decode(&stream[at..end], &mut output).unwrap();
        if (consumed, produced) == (0, 0) {
            break;
        }
        at += consumed;
        decoded.extend_from_slice(&output[..produced]);
        if call % 97 == 0 {
            resumed.push((
                at - decoder.buffered(),
                decoded.len(),
                decoder.checkpoint().unwrap(),
            ));
        }
    }
    decoder.finish().unwrap();
    assert!(decoded == data);
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
        assert!(rest == data[len..], "from {from}");
    }
}

/// Decodes each `.lzma` file in the directory `DECODE_LZMA_CORPUS`, as
/// `xz --format=lzma` writes them, and times it against `lzma-rust2`, in a
/// release build.
#[test]
#[ignore = "needs a corpus of .lzma files"]
fn decodes_a_corpus() {
    use std::time::{Duration, Instant};
    let corpus = std::env::var("DECODE_LZMA_CORPUS").unwrap();
    let mut paths: Vec<_> = std::fs::read_dir(corpus)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "lzma")
        })
        .collect();
    paths.sort();
    let (mut bytes, mut times) = (0, [Duration::ZERO; 2]);
    for path in &paths {
        let file = std::fs::read(path).unwrap();
        let properties = file[..5].try_into().unwrap();
        let stream = &file[13..];
        let mut output = vec![0; 1 << 30];
        let start = Instant::now();
        let mut decoder = Decoder::lzma(properties, None).unwrap();
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
        let theirs = common::theirs_lzma_up_to(stream, properties, None, usize::MAX).unwrap();
        times[1] += start.elapsed();
        assert!(output[..len] == theirs.0, "{}", path.display());
        bytes += len;
    }
    let [ours, theirs] = times;
    println!(
        "{} streams, {bytes} bytes: ours in {ours:?}, lzma-rust2's in {theirs:?}",
        paths.len()
    );
}

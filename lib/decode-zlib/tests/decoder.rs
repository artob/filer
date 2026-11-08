// This is free and unencumbered software released into the public domain.

//! zlib streams, as zlib writes them and as altered by hand, each checked
//! against zlib's decoder, through `zlib-rs`.

mod common;

use common::{adler32, agree, numbers, with_cmf, zlib, zlib_at};
use decode_zlib::{Decoder, Error};

/// Decodes `stream` in pieces of 1 and 7 bytes and whole, checks that zlib
/// decodes it alike, and returns what it decodes to and consumes.
fn decode(stream: &[u8]) -> Result<(Vec<u8>, usize), Error> {
    let whole = agree(stream, 1);
    assert_eq!(whole, agree(stream, 7));
    assert_eq!(whole.clone().ok(), zlib(stream), "zlib decodes otherwise");
    whole
}

/// Returns why `stream` does not decode.
fn error(stream: &[u8]) -> Error {
    decode(stream).unwrap_err()
}

/// `numbers.zz` is by zlib 1.2.12, at level 9 with a 32 KiB window, and
/// `window.zz` at level 1 with a 512-byte window.
#[test]
fn decodes_what_zlib_writes() {
    for stream in [
        &include_bytes!("data/numbers.zz")[..],
        include_bytes!("data/window.zz"),
    ] {
        assert_eq!(decode(stream).unwrap(), (numbers(), stream.len()));
    }
}

#[test]
fn decodes_each_level() {
    let data: Vec<u8> = (0..100_000_u32)
        .flat_map(|n| (n % 1000).to_le_bytes())
        .collect();
    for level in 0..=10 {
        let stream = zlib_at(&data, level);
        assert_eq!(decode(&stream).unwrap().0, data, "{level}");
    }
    assert_eq!(decode(&zlib_at(b"", 6)).unwrap().0, b"");
}

#[test]
fn checks_the_header() {
    let stream = zlib_at(b"some data, some data", 6);
    // Each window size, from 256 bytes to 32 KiB, and each level hint.
    for cinfo in 0..8 {
        for level in 0..4 {
            let mut stream = with_cmf(&stream, cinfo << 4 | 8);
            stream[1] = stream[1] & 0x1F | level << 6;
            let stream = with_cmf(&stream, stream[0]);
            assert_eq!(decode(&stream).unwrap().0, b"some data, some data");
        }
    }
    // Larger windows, and other methods.
    for cmf in [0x88, 0xF8, 0x77, 0x7F, 0x09, 0x0F] {
        assert_eq!(error(&with_cmf(&stream, cmf)), Error::Corrupt, "{cmf:#x}");
    }
    // A check that fails, by one either way.
    for flg in [stream[1] - 1, stream[1] + 1] {
        let mut bad = stream.clone();
        bad[1] = flg;
        assert_eq!(error(&bad), Error::Corrupt, "{flg:#x}");
    }
    // A preset dictionary, which zlib asks for and this decoder lacks.
    let mut preset = stream.clone();
    preset[1] |= 0x20;
    let preset = with_cmf(&preset, preset[0]);
    assert_eq!(agree(&preset, 1).unwrap_err(), Error::Unsupported);
    assert_eq!(zlib(&preset), None);
}

#[test]
fn checks_the_adler32() {
    let data = b"some data, some data";
    let stream = zlib_at(data, 6);
    let at = stream.len() - 4;
    assert_eq!(stream[at..], adler32(data).to_be_bytes());
    for byte in at..stream.len() {
        let mut bad = stream.clone();
        bad[byte] ^= 0x10;
        assert_eq!(error(&bad), Error::Corrupt, "{byte}");
    }
}

#[test]
fn leaves_what_follows_unread() {
    let mut stream = zlib_at(b"data", 6);
    let len = stream.len();
    stream.extend_from_slice(&zlib_at(b"another", 6));
    assert_eq!(decode(&stream).unwrap(), (b"data".to_vec(), len));
    // More input after its end changes nothing.
    let mut decoder = Decoder::new();
    let mut output = [0; 16];
    assert_eq!(decoder.decode(&stream, &mut output).unwrap(), (len, 4));
    assert_eq!(decoder.decode(&stream[len..], &mut output).unwrap(), (0, 0));
    decoder.finish().unwrap();
}

#[test]
fn refuses_streams_cut_short() {
    let stream = include_bytes!("data/numbers.zz");
    for len in [0, 1, 2, 3, 100, stream.len() - 5, stream.len() - 1] {
        assert_eq!(error(&stream[..len]), Error::Corrupt, "{len}");
    }
}

#[test]
fn resumes_from_checkpoints_anywhere() {
    let stream = include_bytes!("data/numbers.zz");
    let mut decoder = Decoder::new();
    let mut output = [0; 100];
    let (mut at, mut decoded) = (0, Vec::new());
    let mut resumed = Vec::new();
    loop {
        let end = stream.len().min(at + 37);
        let (consumed, produced) = decoder.decode(&stream[at..end], &mut output).unwrap();
        if (consumed, produced) == (0, 0) {
            break;
        }
        at += consumed;
        decoded.extend_from_slice(&output[..produced]);
        let checkpoint = decoder.checkpoint().unwrap();
        resumed.push((at - decoder.buffered(), decoded.len(), checkpoint));
    }
    decoder.finish().unwrap();
    assert_eq!(decoded, numbers());
    assert!(resumed.len() > 100);
    for (from, len, mut checkpoint) in resumed {
        let mut output = vec![0; decoded.len()];
        let (consumed, produced) = checkpoint.decode(&stream[from..], &mut output).unwrap();
        assert_eq!(from + consumed, stream.len());
        assert!(output[..produced] == decoded[len..], "from {from}");
        checkpoint.finish().unwrap();
    }
}

/// Decodes each `.zz` file in the directory `DECODE_ZLIB_CORPUS` names,
/// whole, as zlib decodes it, and times it against `zlib-rs`, in a
/// release build.
#[test]
#[ignore = "needs a corpus of .zz files"]
fn decodes_a_corpus() {
    use std::time::{Duration, Instant};
    use zlib_rs::{Inflate, InflateFlush, Status};
    let corpus = std::env::var("DECODE_ZLIB_CORPUS").unwrap();
    let mut paths: Vec<_> = std::fs::read_dir(corpus)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "zz"))
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
        let (mut inflate, mut theirs) = (Inflate::new(true, 15), vec![0; len]);
        let status = inflate.decompress(&stream, &mut theirs, InflateFlush::Finish);
        times[1] += start.elapsed();
        assert_eq!(status, Ok(Status::StreamEnd), "{}", path.display());
        assert_eq!(inflate.total_out(), u64::try_from(len).unwrap());
        assert!(output[..len] == theirs, "{}", path.display());
        bytes += len;
    }
    let [ours, theirs] = times;
    println!(
        "{} streams, {bytes} bytes: ours in {ours:?}, zlib-rs's in {theirs:?}",
        paths.len()
    );
}

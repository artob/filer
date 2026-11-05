// This is free and unencumbered software released into the public domain.

//! lzop files, as `lzop` writes them and as built by hand.

mod common;

use common::{
    ADLER32_C, ADLER32_D, CRC32_C, CRC32_D, HEADER_CRC32, Header, agree, block, file, numbers,
};
use decode_lzo::{Decoder, Error};

/// Decodes `stream` in pieces of 1 and 7 bytes and whole.
fn decode(stream: &[u8]) -> Result<Vec<u8>, Error> {
    let whole = agree(stream, 1);
    assert_eq!(whole, agree(stream, 7));
    whole
}

/// Returns why `stream` does not decode.
fn error(stream: &[u8]) -> Error {
    decode(stream).unwrap_err()
}

/// The CRC-32 that `cksum -a crc32b` and zlib compute.
fn crc32(bytes: &[u8]) -> u32 {
    compute_crc::crc32(bytes)
}

/// The files in `data` are by lzop 1.04: `best.lzo` of `seq 1 3000` 100
/// times over, in six blocks, at `-9`, LZO1X-999; `crc32.lzo` of it 20
/// times over at `-9 --crc32`, its checksums CRC-32s; `fast.lzo` of it 3
/// times over at `-1 -F`, LZO1X-1(15), unchecked; `random.lzo` of 20,000
/// random bytes, stored; and `empty.lzo` of nothing.
#[test]
fn decodes_what_lzop_writes() {
    let best = include_bytes!("data/best.lzo");
    assert_eq!(agree(best, 4096).unwrap(), numbers().repeat(100));
    assert_eq!(
        decode(include_bytes!("data/crc32.lzo")).unwrap(),
        numbers().repeat(20)
    );
    assert_eq!(
        decode(include_bytes!("data/fast.lzo")).unwrap(),
        numbers().repeat(3)
    );
    let random = decode(include_bytes!("data/random.lzo")).unwrap();
    assert_eq!((random.len(), crc32(&random)), (20_000, 0x22D7_CC19));
    assert_eq!(decode(include_bytes!("data/empty.lzo")).unwrap(), b"");
}

#[test]
fn decodes_files_built_by_hand() {
    let data = numbers();
    let blocks: [(bool, &[u8]); 3] = [(true, &data), (false, b"stored"), (true, &data[..999])];
    let expected = [&data[..], b"stored", &data[..999]].concat();
    for flags in [
        0,
        ADLER32_D,
        CRC32_D,
        ADLER32_D | ADLER32_C,
        CRC32_D | CRC32_C | HEADER_CRC32,
        ADLER32_D | ADLER32_C | CRC32_D | CRC32_C,
    ] {
        for method in [1, 2, 3] {
            let header = Header {
                flags,
                method,
                ..Header::default()
            };
            assert_eq!(
                decode(&file(&header, &blocks)).unwrap(),
                expected,
                "{flags:#x}"
            );
        }
    }
    // Before version 0.94, with fewer fields; a name; an extra field.
    let older = Header {
        version: 0x0930,
        ..Header::default()
    };
    let named = Header {
        name: vec![b'n'; 255],
        ..Header::default()
    };
    let extra = Header {
        extra: Some(b"extra".to_vec()),
        ..Header::default()
    };
    for header in [older, named, extra] {
        assert_eq!(decode(&file(&header, &blocks)).unwrap(), expected);
    }
}

#[test]
fn reads_files_one_after_another() {
    let hello = file(&Header::default(), &[(false, b"hello")]);
    let empty = include_bytes!("data/empty.lzo");
    let stream = [&hello[..], empty, &hello].concat();
    assert_eq!(decode(&stream).unwrap(), b"hellohello");
    // Zero bytes may follow, as `lzop` ignores them, but nothing else.
    let padded = [&stream[..], &[0; 9]].concat();
    assert_eq!(decode(&padded).unwrap(), b"hellohello");
    for trailing in [&b"garbage"[..], &[0, 0, 1], &[0, 0, 0x89]] {
        let bad = [&stream[..], trailing].concat();
        assert_eq!(error(&bad), Error::Corrupt, "{trailing:?}");
    }
    // And there must be one, ended.
    assert_eq!(error(b""), Error::Corrupt);
    assert_eq!(error(&[0; 4]), Error::Corrupt);
    assert_eq!(error(&hello[..hello.len() - 4]), Error::Corrupt);
    assert_eq!(error(&hello[..30]), Error::Corrupt);
}

#[test]
fn checks_files() {
    let data = numbers();
    let header = Header {
        flags: ADLER32_D | ADLER32_C | CRC32_D | CRC32_C,
        ..Header::default()
    };
    let stream = file(&header, &[(true, &data)]);
    let start = header.bytes().len();
    // Any byte changed, of the header, the block's sizes or checksums, or
    // what it holds.
    for byte in 9..start + 8 + 16 + 20 {
        let mut bad = stream.clone();
        bad[byte] ^= 0x01;
        assert!(decode(&bad).is_err(), "{byte}");
    }
    // Packed larger than what it decodes to, or not at all; too large.
    for (size, packed) in [(10_u32, 11_u32), (10, 0), ((64 << 20) + 1, 10)] {
        let mut bad = header.bytes();
        bad.extend_from_slice(&size.to_be_bytes());
        bad.extend_from_slice(&packed.to_be_bytes());
        assert_eq!(error(&bad), Error::Corrupt, "{size} {packed}");
    }
    // A block that decodes to less than its size says.
    let mut short = block(0, &data, true);
    short[..4].copy_from_slice(&(data.len() as u32 + 1).to_be_bytes());
    let short = [
        &Header {
            flags: 0,
            ..Header::default()
        }
        .bytes()[..],
        &short,
        &[0; 4],
    ]
    .concat();
    assert_eq!(error(&short), Error::Corrupt);
    // Not lzop, or too old.
    assert_eq!(error(&stream[1..]), Error::Corrupt);
    let old = Header {
        version: 0x0800,
        ..Header::default()
    };
    assert_eq!(error(&file(&old, &[])), Error::Corrupt);
}

#[test]
fn refuses_what_it_does_not_read() {
    let unsupported = [
        // NRV2B, and zlib.
        Header {
            method: 0x1F,
            ..Header::default()
        },
        Header {
            method: 128,
            ..Header::default()
        },
        // A filter, part of a multipart archive, and a flag yet to mean
        // anything.
        Header {
            flags: 0x800,
            ..Header::default()
        },
        Header {
            flags: 0x400,
            ..Header::default()
        },
        Header {
            flags: 0x4000,
            ..Header::default()
        },
        // A newer lzop's.
        Header {
            needed: 0x1050,
            ..Header::default()
        },
    ];
    for header in unsupported {
        let stream = file(&header, &[(false, b"hello")]);
        assert_eq!(error(&stream), Error::Unsupported);
    }
}

#[test]
fn resumes_from_checkpoints_anywhere() {
    for (stream, piece) in [
        (&include_bytes!("data/best.lzo")[..], 333),
        (include_bytes!("data/crc32.lzo"), 97),
    ] {
        let mut decoder = Decoder::new();
        let mut output = [0; 1000];
        let (mut at, mut decoded) = (0, Vec::new());
        let mut resumed = Vec::new();
        loop {
            let end = stream.len().min(at + piece);
            let (consumed, produced) = decoder.decode(&stream[at..end], &mut output).unwrap();
            if (consumed, produced) == (0, 0) && at == stream.len() {
                break;
            }
            at += consumed;
            decoded.extend_from_slice(&output[..produced]);
            if resumed.len() < 60 && decoded.len() % 7 == 0 {
                let checkpoint = decoder.checkpoint().unwrap();
                assert!(checkpoint.footprint() < 300_000);
                resumed.push((at - decoder.buffered(), decoded.len(), checkpoint));
            }
        }
        decoder.finish().unwrap();
        assert!(resumed.len() > 20);
        for (from, len, mut checkpoint) in resumed {
            let mut output = vec![0; decoded.len()];
            let (mut at, mut done) = (from, 0);
            loop {
                let (consumed, produced) = checkpoint
                    .decode(&stream[at..], &mut output[done..])
                    .unwrap();
                if (consumed, produced) == (0, 0) {
                    break;
                }
                (at, done) = (at + consumed, done + produced);
            }
            checkpoint.finish().unwrap();
            assert!(output[..done] == decoded[len..], "from {from}");
        }
    }
}

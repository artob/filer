// This is free and unencumbered software released into the public domain.

//! Deflate64 streams, as 7-Zip writes them, checked against the
//! `deflate64` crate's decoder.

#![cfg(feature = "deflate64")]

mod common;

use common::{agree, agree_with, crc32, deflate64};
use decode_deflate::{Decoder, Error};
use filer_spec::{
    Image, Outcome,
    fuzz::{Fuzz, Mutations},
};
use std::sync::Arc;

/// The fixtures are ZIP members of method 9 by 7-Zip 26.00 at `-mx=9`:
/// text; text that repeats more than 32 KiB back, so with distance codes
/// 30 and 31; and data that repeats for far longer than 258 bytes, so with
/// length code 285's extra bits. Each with what it decodes to.
const FIXTURES: [(&str, &[u8], usize, u32); 3] = [
    (
        "text",
        include_bytes!("data/text.deflate64"),
        12000,
        0xDF56_59A7,
    ),
    (
        "far",
        include_bytes!("data/far.deflate64"),
        71001,
        0x7A7D_F202,
    ),
    (
        "long",
        include_bytes!("data/long.deflate64"),
        122_000,
        0x0C86_D892,
    ),
];

#[test]
fn decodes_what_7zip_writes() {
    for (name, stream, len, crc) in FIXTURES {
        for piece in [1, 7, 4096] {
            let (data, consumed) = agree_with(Decoder::deflate64, stream, piece).unwrap();
            assert_eq!((data.len(), crc32(&data)), (len, crc), "{name}");
            assert_eq!(consumed, stream.len(), "{name}");
        }
        assert_eq!(
            deflate64(stream).map(|data| crc32(&data)),
            Some(crc),
            "{name}"
        );
    }
}

#[test]
fn tells_deflate64_from_deflate() {
    // Distance codes 30 and 31 are Deflate64's alone.
    let (_, far, _, _) = FIXTURES[1];
    assert_eq!(agree(far, 64).unwrap_err(), Error::Corrupt);
    // Length code 285 is 258 in Deflate, and 3 and 16 bits in Deflate64,
    // so that zlib's runs of 258 decode otherwise, or not at all.
    let zeros = include_bytes!("data/zeros-9.deflate");
    let deflate = agree(zeros, 64).unwrap().0;
    let as64 = agree_with(Decoder::deflate64, zeros, 64);
    assert!(as64.is_err() || as64.unwrap().0 != deflate);
}

/// Mutates Deflate64 streams, and checks that each decodes the same in
/// pieces as whole, and, when it decodes, as the `deflate64` crate decodes
/// it. The other way is no check: that crate decodes what zlib refuses in
/// Deflate, as this crate does in both, matches from before the stream's
/// start, as if a window of zeros preceded it.
#[test]
fn decodes_mutations_as_deflate64_does() {
    const BYTES: [u8; 8] = [0, 1, 2, 3, 0x1F, 0x80, 0xFE, 0xFF];
    const WORDS: [u32; 4] = [0, 0xFFFF, 0x8000, u32::MAX];
    const MUTATIONS: Mutations = Mutations::new(&BYTES, &WORDS).aligned(false).most(2);
    let seeds = FIXTURES.map(|(_, stream, _, _)| Arc::new(stream.to_vec()));
    Fuzz::new("DECODE_DEFLATE_FUZZ", 0x6464_5EED)
        .iterations(200)
        .run(
            seeds.len(),
            |source, rng| {
                let seed = &seeds[source];
                let regions = [0..64.min(seed.len() as u64), 0..seed.len() as u64];
                let input = MUTATIONS.mutate(seed, &regions, rng);
                let mut stream = vec![0; input.length() as usize];
                input.copy(0, &mut stream);
                (stream, 1 + rng.below(64) as usize)
            },
            |(stream, piece)| {
                let ours = agree_with(Decoder::deflate64, &stream, piece).ok();
                let theirs = deflate64(&stream);
                let ours = ours.map(|(data, _)| data);
                if ours.is_some() {
                    assert!(
                        ours == theirs,
                        "ours: {:?}; deflate64's: {:?}",
                        ours.as_ref().map(Vec::len),
                        theirs.as_ref().map(Vec::len),
                    );
                }
                Outcome {
                    opened: ours.is_some(),
                    entries: 1,
                    bytes: ours.as_ref().map_or(0, |data| data.len() as u64),
                    errors: usize::from(ours.is_none()),
                    ..Outcome::default()
                }
            },
        );
}

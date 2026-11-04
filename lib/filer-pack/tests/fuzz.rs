// This is free and unencumbered software released into the public domain.

//! A dependency-free, structure-aware fuzzer of the decoders: it mutates
//! streams where their headers and trailers are, with values chosen to hit
//! boundaries, and checks that each decodes the same in pieces of a random
//! size as whole, failing only as `Corrupt`, or `Unsupported` for what a
//! decoder declines.
//!
//! Each `cargo test` runs a few hundred inputs per decoder. For longer
//! runs, set `FILER_PACK_FUZZ_ITERATIONS`; `FILER_PACK_FUZZ_SEED` picks
//! another sequence, and a failure prints the seed and iteration that
//! reproduce it. `fuzz/` has a coverage-guided `cargo-fuzz` target per
//! decoder over the same check.

#![cfg(any(feature = "gzip", feature = "zstd"))]

mod common;

use common::agree;
use filer_api::ErrorKind;
use filer_pack::Decoder;
use filer_spec::{
    Image, Outcome,
    fuzz::{Fuzz, Mutations},
};
use std::{ops::Range, sync::Arc};

/// A stream to mutate, and where its headers and trailers are.
struct Seed {
    stream: Arc<Vec<u8>>,
    regions: Vec<Range<u64>>,
}

impl Seed {
    fn new(stream: Vec<u8>, mut regions: Vec<Range<u64>>) -> Self {
        regions.push(0..stream.len() as u64);
        Self {
            stream: Arc::new(stream),
            regions,
        }
    }
}

/// Mutates streams of `corpus` with `mutations`, from `seed`, and checks
/// that each decodes alike with decoders from `new` in pieces and whole,
/// failing only as one of `kinds`.
fn run<D: Decoder>(
    seed: u64,
    corpus: &[Seed],
    mutations: Mutations,
    new: fn() -> D,
    kinds: &[ErrorKind],
) {
    Fuzz::new("FILER_PACK_FUZZ", seed).iterations(400).run(
        corpus.len(),
        |source, rng| {
            let seed = &corpus[source];
            let input = mutations.mutate(&seed.stream, &seed.regions, rng);
            let mut bytes = vec![0; input.length() as usize];
            input.copy(0, &mut bytes);
            (bytes, 1 + rng.below(64) as usize)
        },
        |(stream, piece)| match agree(new, &stream, piece) {
            Ok(data) => Outcome {
                opened: true,
                entries: 1,
                bytes: data.len() as u64,
                ..Outcome::default()
            },
            Err(error) => {
                assert!(kinds.contains(&error.kind()), "{error:?}");
                Outcome {
                    errors: 1,
                    ..Outcome::default()
                }
            },
        },
    );
}

#[cfg(feature = "gzip")]
mod gzip {
    use super::*;
    use common::Member;
    use filer_pack::Gzip;

    /// Values at the edges of what gzip headers and trailers hold: the
    /// magic number's bytes, the method, and each flag.
    const BYTES: [u8; 12] = [0, 1, 7, 8, 0x02, 0x04, 0x08, 0x10, 0x1F, 0x8B, 0xE0, 0xFF];
    const WORDS: [u32; 7] = [0, 1, 0xFF, 0xFFFF, 0x8B1F, 0x7FFF_FFFF, u32::MAX];

    /// Fields lie anywhere after a name; the checksums catch most
    /// mutations, so few at once.
    const MUTATIONS: Mutations = Mutations::new(&BYTES, &WORDS).aligned(false).most(2);

    /// Returns the stream of `members`, each with its data, and `garbage`
    /// after.
    fn seed(members: &[(Member<'_>, &[u8])], garbage: &[u8]) -> Seed {
        let mut stream = Vec::new();
        let mut regions = Vec::new();
        for (member, data) in members {
            let at = stream.len() as u64;
            stream.extend(member.gzip(data));
            regions.push(at..at + member.header().len() as u64);
            regions.push(stream.len() as u64 - 8..stream.len() as u64);
        }
        stream.extend_from_slice(garbage);
        Seed::new(stream, regions)
    }

    #[test]
    fn survives_mutated_gzip_streams() {
        let full = Member {
            extra: Some(b"AB\x02\x00hi"),
            name: Some(b"name.txt"),
            comment: Some(b"comment"),
            header_crc: true,
        };
        let numbers = include_bytes!("data/numbers.gz").to_vec();
        let len = numbers.len() as u64;
        let corpus = [
            Seed::new(numbers, vec![0..32, len - 30..len]),
            seed(&[(full, b"some data, some data")], b""),
            seed(
                &[
                    (Member::default(), b"one"),
                    (full, b""),
                    (Member::default(), b"two"),
                ],
                b"garbage",
            ),
        ];
        run(
            0x6219_5EED,
            &corpus,
            MUTATIONS,
            Gzip::new,
            &[ErrorKind::Corrupt],
        );
    }
}

#[cfg(feature = "zstd")]
mod zstd {
    use super::*;
    use common::{Block, skippable, zstd};
    use filer_pack::Zstd;

    /// Values at the edges of what zstd frame and block headers hold: the
    /// magic numbers' bytes, each flag, and block types.
    const BYTES: [u8; 12] = [0, 1, 2, 4, 6, 0x08, 0x20, 0x28, 0x50, 0xC0, 0xFD, 0xFF];
    const WORDS: [u32; 8] = [
        0,
        1,
        0xFFFF,
        0x2_0000 << 3,
        0x184D_2A50,
        0xFD2F_B528,
        0x7FFF_FFFF,
        u32::MAX,
    ];

    /// Headers are packed; the checksums catch most mutations, so few at
    /// once.
    const MUTATIONS: Mutations = Mutations::new(&BYTES, &WORDS).aligned(false).most(2);

    /// Returns `stream` and where its frame headers, block headers, and
    /// checksums are.
    fn seed(stream: Vec<u8>) -> Seed {
        let mut regions = Vec::new();
        let mut at = 0;
        let mut region = |at: usize, len: usize| regions.push(at as u64..(at + len) as u64);
        while at < stream.len() {
            let magic = u32::from_le_bytes(stream[at..at + 4].try_into().unwrap());
            if magic & !0x0F == 0x184D_2A50 {
                region(at, 8);
                at += 8 + u32::from_le_bytes(stream[at + 4..at + 8].try_into().unwrap()) as usize;
                continue;
            }
            let descriptor = stream[at + 4];
            let single = descriptor & 0x20 != 0;
            let header = 5
                + usize::from(!single)
                + [0, 1, 2, 4][usize::from(descriptor & 3)]
                + [usize::from(single), 2, 4, 8][usize::from(descriptor >> 6)];
            region(at, header);
            at += header;
            loop {
                let block = u32::from_le_bytes([stream[at], stream[at + 1], stream[at + 2], 0]);
                region(at, 3);
                at += 3 + if (block >> 1) & 3 == 1 {
                    1
                } else {
                    block as usize >> 3
                };
                if block & 1 != 0 {
                    break;
                }
            }
            if descriptor & 0x04 != 0 {
                region(at, 4);
                at += 4;
            }
        }
        Seed::new(stream, regions)
    }

    #[test]
    fn survives_mutated_zstd_streams() {
        let numbers = include_bytes!("data/numbers.zst").to_vec();
        let built = [
            skippable(b"skipped"),
            zstd(&[Block::Raw(b"raw"), Block::Rle(b'r', 300), Block::Raw(b"")]),
            zstd(&[Block::Rle(0, 0)]),
        ]
        .concat();
        let corpus = [seed(numbers), seed(built)];
        let kinds = [ErrorKind::Corrupt, ErrorKind::Unsupported];
        run(0x2575_5EED, &corpus, MUTATIONS, Zstd::new, &kinds);
    }
}

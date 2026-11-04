// This is free and unencumbered software released into the public domain.

//! A dependency-free, structure-aware fuzzer of the decoders: it mutates
//! streams where their headers and trailers are, with values chosen to hit
//! boundaries, and checks that each decodes the same in pieces of a random
//! size as whole, failing only as `Corrupt`.
//!
//! Each `cargo test` runs a few hundred inputs. For longer runs, set
//! `FILER_PACK_FUZZ_ITERATIONS`; `FILER_PACK_FUZZ_SEED` picks another
//! sequence, and a failure prints the seed and iteration that reproduce
//! it. `fuzz/` has a coverage-guided `cargo-fuzz` target per decoder over
//! the same check.

#![cfg(feature = "gzip")]

mod common;

use common::{Member, agree};
use filer_api::ErrorKind;
use filer_pack::Gzip;
use filer_spec::{
    Image, Outcome,
    fuzz::{Fuzz, Mutations},
};
use std::{ops::Range, sync::Arc};

/// Values at the edges of what gzip headers and trailers hold: the magic
/// number's bytes, the method, and each flag.
const BYTES: [u8; 12] = [0, 1, 7, 8, 0x02, 0x04, 0x08, 0x10, 0x1F, 0x8B, 0xE0, 0xFF];
const WORDS: [u32; 7] = [0, 1, 0xFF, 0xFFFF, 0x8B1F, 0x7FFF_FFFF, u32::MAX];

/// Fields lie anywhere after a name; the checksums catch most mutations,
/// so few at once.
const MUTATIONS: Mutations = Mutations::new(&BYTES, &WORDS).aligned(false).most(2);

/// A stream to mutate, and where its headers and trailers are.
struct Seed {
    stream: Arc<Vec<u8>>,
    regions: Vec<Range<u64>>,
}

impl Seed {
    /// Returns the stream of `members`, each with its data, and `garbage`
    /// after.
    fn gzip(members: &[(Member<'_>, &[u8])], garbage: &[u8]) -> Self {
        let mut stream = Vec::new();
        let mut regions = Vec::new();
        for (member, data) in members {
            let at = stream.len() as u64;
            stream.extend(member.gzip(data));
            regions.push(at..at + member.header().len() as u64);
            regions.push(stream.len() as u64 - 8..stream.len() as u64);
        }
        stream.extend_from_slice(garbage);
        regions.push(0..stream.len() as u64);
        Self {
            stream: Arc::new(stream),
            regions,
        }
    }
}

fn corpus() -> Vec<Seed> {
    let full = Member {
        extra: Some(b"AB\x02\x00hi"),
        name: Some(b"name.txt"),
        comment: Some(b"comment"),
        header_crc: true,
    };
    let numbers = include_bytes!("data/numbers.gz").to_vec();
    let len = numbers.len() as u64;
    vec![
        Seed {
            stream: Arc::new(numbers),
            regions: vec![0..32, len - 30..len, 0..len],
        },
        Seed::gzip(&[(full, b"some data, some data")], b""),
        Seed::gzip(
            &[
                (Member::default(), b"one"),
                (full, b""),
                (Member::default(), b"two"),
            ],
            b"garbage",
        ),
    ]
}

#[test]
fn survives_mutated_gzip_streams() {
    let corpus = corpus();
    Fuzz::new("FILER_PACK_FUZZ", 0x6219_5EED)
        .iterations(400)
        .run(
            corpus.len(),
            |source, rng| {
                let seed = &corpus[source];
                let input = MUTATIONS.mutate(&seed.stream, &seed.regions, rng);
                let mut bytes = vec![0; input.length() as usize];
                input.copy(0, &mut bytes);
                (bytes, 1 + rng.below(64) as usize)
            },
            |(stream, piece)| match agree(Gzip::new, &stream, piece) {
                Ok(data) => Outcome {
                    opened: true,
                    entries: 1,
                    bytes: data.len() as u64,
                    ..Outcome::default()
                },
                Err(error) => {
                    assert_eq!(error.kind(), ErrorKind::Corrupt);
                    Outcome {
                        errors: 1,
                        ..Outcome::default()
                    }
                },
            },
        );
}

// This is free and unencumbered software released into the public domain.

//! A structure-aware fuzzer of the decoder: it mutates streams where their
//! headers, tables, and CRCs are, with values chosen to hit boundaries, and
//! checks that each decodes the same in pieces of a random size as whole.
//!
//! Each `cargo test` runs a few hundred inputs. For longer runs, set
//! `DECODE_BZIP2_FUZZ_ITERATIONS`; `DECODE_BZIP2_FUZZ_SEED` picks another
//! sequence, and a failure prints the seed and iteration that reproduce
//! it. `fuzz/` has a coverage-guided `cargo-fuzz` target over the same
//! check.

mod common;

use common::agree;
use filer_spec::{
    Image, Outcome,
    fuzz::{Fuzz, Mutations, Order},
};
use std::{ops::Range, sync::Arc};

/// A stream to mutate, and where its headers and CRCs are.
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
/// that each decodes alike in pieces and whole.
fn run(seed: u64, corpus: &[Seed], mutations: Mutations) {
    Fuzz::new("DECODE_BZIP2_FUZZ", seed).iterations(400).run(
        corpus.len(),
        |source, rng| {
            let seed = &corpus[source];
            let input = mutations.mutate(&seed.stream, &seed.regions, rng);
            let mut bytes = vec![0; input.length() as usize];
            input.copy(0, &mut bytes);
            (bytes, 1 + rng.below(64) as usize)
        },
        |(stream, piece)| match agree(&stream, piece) {
            Ok(data) => Outcome {
                opened: true,
                entries: 1,
                bytes: data.len() as u64,
                ..Outcome::default()
            },
            Err(_) => Outcome {
                errors: 1,
                ..Outcome::default()
            },
        },
    );
}

/// Values at the edges of what bzip2 headers hold: the magic numbers'
/// bytes, block sizes, and table and selector counts.
const BYTES: [u8; 12] = [0, 1, 2, 6, 7, 0x17, 0x31, 0x80, b'1', b'9', b'h', 0xFF];
const WORDS: [u32; 6] = [0, 1, 0xFFFF, 0x3141_5926, 0x1772_4538, u32::MAX];

/// Fields are of bits, most significant first; the CRCs catch most
/// mutations, so few at once.
const MUTATIONS: Mutations = Mutations::new(&BYTES, &WORDS)
    .order(Order::Big)
    .aligned(false)
    .most(2);

/// Returns `stream`, with where its first block's header and tables
/// are, as far as its first 64 bytes go, and its end.
fn seed(stream: &[u8]) -> Seed {
    let len = stream.len() as u64;
    Seed::new(
        stream.to_vec(),
        vec![0..len.min(64), len.saturating_sub(16)..len],
    )
}

#[test]
fn survives_mutated_bzip2_streams() {
    // Only a small stream: a mutated block decodes whole, in pieces,
    // before its CRC fails it.
    let corpus = [seed(include_bytes!("data/numbers.bz2"))];
    run(0xB212_5EED, &corpus, MUTATIONS);
}

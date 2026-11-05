// This is free and unencumbered software released into the public domain.

//! A dependency-free, structure-aware fuzzer: it mutates real volumes
//! where their structures are (the anchor, the volume descriptor
//! sequence, and the partition's first blocks, of the file set, the file
//! entries, and the directories), with values chosen to hit boundaries,
//! and walks each result with `common::exercise`, which checks that every
//! answer is consistent.
//!
//! Each `cargo test` runs a couple of hundred inputs. For longer runs, set
//! `FILER_UDF_FUZZ_ITERATIONS`; `FILER_UDF_FUZZ_SEED` picks another
//! sequence, and a failure prints the seed and iteration that reproduce
//! it. `fuzz/` has coverage-guided `cargo-fuzz` targets over the same
//! walk.

mod common;

use common::{exercise, image};
use filer_spec::fuzz::{Fuzz, Mutations};
use std::{ops::Range, sync::Arc};

/// A volume to mutate, and where its structures are.
struct Seed {
    image: Arc<Vec<u8>>,
    regions: Vec<Range<u64>>,
}

impl Seed {
    /// Returns the seed of `image`, of blocks of `block` bytes, whose
    /// partition starts at block `start`.
    fn new(image: &[u8], block: u64, start: u64) -> Self {
        let len = image.len() as u64;
        Self {
            image: Arc::new(image.to_vec()),
            regions: vec![
                // The anchor, the main volume descriptor sequence, the
                // partition's first blocks, and anywhere, now and then.
                256 * block..257 * block,
                96 * block..112 * block,
                start * block..((start + 160) * block).min(len),
                0..len,
            ],
        }
    }
}

/// Values at the edges of what UDF structures hold: tags' identifiers,
/// file types, flags, lengths, and blocks.
const BYTES: [u8; 12] = [0, 1, 2, 3, 4, 5, 8, 12, 16, 0x80, 0xFE, 0xFF];
const WORDS: [u32; 12] = [
    0,
    1,
    2,
    256,
    257,
    258,
    261,
    266,
    0x4000_0000,
    0xC000_0000,
    0x7FFF_FFFF,
    0xFFFF_FFFF,
];

fn corpus() -> Vec<Seed> {
    vec![
        Seed::new(&image(include_bytes!("fixtures/spec.sparse")), 512, 257),
        Seed::new(&image(include_bytes!("fixtures/udf-1.02.sparse")), 512, 257),
        Seed::new(&image(include_bytes!("fixtures/udf-2.01.sparse")), 512, 257),
        Seed::new(
            &image(include_bytes!("fixtures/udf-2.50.sparse")),
            2048,
            288,
        ),
    ]
}

/// The mutations to make, with those values.
const MUTATIONS: Mutations = Mutations::new(&BYTES, &WORDS);

#[test]
fn survives_mutated_volumes() {
    let corpus = corpus();
    Fuzz::new("FILER_UDF_FUZZ", 0x0D_F5ED).run(
        corpus.len(),
        |source, rng| MUTATIONS.mutate(&corpus[source].image, &corpus[source].regions, rng),
        exercise,
    );
}

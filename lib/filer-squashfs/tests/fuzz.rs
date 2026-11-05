// This is free and unencumbered software released into the public domain.

//! A dependency-free, structure-aware fuzzer: it mutates real images
//! where their structures are (the superblock, the inode table, and the
//! directory, fragment, and extended attribute tables after it), with
//! values chosen to hit boundaries, and walks each result with
//! `common::exercise`, which checks that every answer is consistent.
//!
//! Each `cargo test` runs a couple of hundred inputs. For longer runs, set
//! `FILER_SQUASHFS_FUZZ_ITERATIONS`; `FILER_SQUASHFS_FUZZ_SEED` picks
//! another sequence, and a failure prints the seed and iteration that
//! reproduce it. `fuzz/` has coverage-guided `cargo-fuzz` targets over the
//! same walk.

mod common;

use common::exercise;
use filer_spec::fuzz::{Fuzz, Mutations};
use std::{ops::Range, sync::Arc};

/// An image to mutate, and where its structures are.
struct Seed {
    image: Arc<Vec<u8>>,
    regions: Vec<Range<u64>>,
}

impl Seed {
    fn new(image: &[u8]) -> Self {
        let u64_at = |at: usize| u64::from_le_bytes(image[at..at + 8].try_into().unwrap());
        let (used, inodes, dirs) = (u64_at(40), u64_at(64), u64_at(72));
        let len = image.len() as u64;
        Self {
            image: Arc::new(image.to_vec()),
            regions: vec![
                // The superblock, the inode table, the tables after it,
                // and anywhere, now and then.
                0..96,
                inodes..dirs,
                dirs..used,
                0..len,
            ],
        }
    }
}

/// Values at the edges of what SquashFS structures hold: metadata and
/// data blocks' sizes and their flags of being stored as they are, inode
/// types, block sizes, and the absent table's and index's marks.
const BYTES: [u8; 12] = [0, 1, 2, 3, 7, 8, 14, 15, 0x7F, 0x80, 0xFE, 0xFF];
const WORDS: [u32; 12] = [
    0,
    1,
    3,
    0x0FFF,
    0x1000,
    0x2000,
    0x8000,
    0xFFFF,
    0x0002_0000,
    0x0100_0000,
    0x7FFF_FFFF,
    0xFFFF_FFFF,
];

fn corpus() -> Vec<Seed> {
    [
        &include_bytes!("fixtures/spec.sqfs")[..],
        include_bytes!("fixtures/spec-uncompressed.sqfs"),
        include_bytes!("fixtures/attrs.sqfs"),
        include_bytes!("fixtures/attrs-uncompressed.sqfs"),
        include_bytes!("fixtures/gzip.sqfs"),
        include_bytes!("fixtures/lzma.sqfs"),
        include_bytes!("fixtures/lzo.sqfs"),
        include_bytes!("fixtures/xz.sqfs"),
        include_bytes!("fixtures/lz4.sqfs"),
        include_bytes!("fixtures/zstd.sqfs"),
        include_bytes!("fixtures/uncompressed.sqfs"),
        include_bytes!("fixtures/small-blocks.sqfs"),
    ]
    .into_iter()
    .map(Seed::new)
    .collect()
}

/// The mutations to make, with those values.
const MUTATIONS: Mutations = Mutations::new(&BYTES, &WORDS);

#[test]
fn survives_mutated_images() {
    let corpus = corpus();
    Fuzz::new("FILER_SQUASHFS_FUZZ", 0x50A5_5EED).run(
        corpus.len(),
        |source, rng| MUTATIONS.mutate(&corpus[source].image, &corpus[source].regions, rng),
        exercise,
    );
}

// This is free and unencumbered software released into the public domain.

//! A dependency-free, structure-aware fuzzer: it mutates real volumes
//! where their structures are (the boot sector, the MFT, and the index
//! records), with values chosen to hit boundaries, and walks each result
//! with `common::exercise`, which checks that every answer is consistent.
//!
//! Each `cargo test` runs a couple of hundred inputs. For longer runs, set
//! `FILER_NTFS_FUZZ_ITERATIONS`; `FILER_NTFS_FUZZ_SEED` picks another
//! sequence, and a failure prints the seed and iteration that reproduce
//! it. `fuzz/` has coverage-guided `cargo-fuzz` targets over the same walk.

mod common;

use common::{exercise, unsparse};
use filer_spec::fuzz::{Fuzz, Mutations};
use std::{ops::Range, sync::Arc};

/// A volume to mutate, and where its structures are.
struct Seed {
    image: Arc<Vec<u8>>,
    regions: Vec<Range<u64>>,
}

impl Seed {
    fn new(image: Vec<u8>) -> Self {
        let u16_at = |at: usize| u64::from(u16::from_le_bytes([image[at], image[at + 1]]));
        let u64_at = |at: usize| u64::from_le_bytes(image[at..at + 8].try_into().unwrap());
        let sector = u16_at(11);
        let cluster = sector * u64::from(image[13]);
        let mft = u64_at(0x30) * cluster;
        let len = image.len() as u64;
        // The boot sector, the MFT's first records, and the index records.
        let mut regions: Vec<Range<u64>> = Vec::new();
        regions.push(0..512);
        regions.push(mft..(mft + 64 * 1024).min(len));
        let mut at = 0;
        while at + 4 <= len && regions.len() < 34 {
            if &image[at as usize..at as usize + 4] == b"INDX" {
                regions.push(at..at + 4096);
            }
            at += 512;
        }
        // And anywhere, now and then.
        regions.push(0..len);
        Self {
            image: Arc::new(image),
            regions,
        }
    }
}

/// Values at the edges of what NTFS structures hold.
const BYTES: [u8; 12] = [
    0, 1, 0x02, 0x0F, 0x10, 0x30, 0x40, 0x7F, 0x80, 0xA0, 0xFE, 0xFF,
];
const WORDS: [u32; 12] = [
    0,
    1,
    5,
    16,
    0x18,
    0x30,
    0x80,
    0x0FFF,
    0xFFFF,
    0xA000_000C,
    0x7FFF_FFFF,
    0xFFFF_FFFF,
];

fn corpus() -> Vec<Seed> {
    [
        &include_bytes!("fixtures/ntfs-4k.sparse")[..],
        include_bytes!("fixtures/ntfs-512.sparse"),
        include_bytes!("fixtures/ntfs-compressed.sparse"),
        include_bytes!("fixtures/ntfs-links.sparse"),
        include_bytes!("fixtures/ntfs-streams.sparse"),
        include_bytes!("fixtures/windows-4k.sparse"),
        include_bytes!("fixtures/windows-512.sparse"),
    ]
    .into_iter()
    .map(|sparse| Seed::new(unsparse(sparse)))
    .collect()
}

/// The mutations to make, with those values.
const MUTATIONS: Mutations = Mutations::new(&BYTES, &WORDS);

#[test]
fn survives_mutated_volumes() {
    let corpus = corpus();
    Fuzz::new("FILER_NTFS_FUZZ", 0x47F5_5EED).run(
        corpus.len(),
        |source, rng| MUTATIONS.mutate(&corpus[source].image, &corpus[source].regions, rng),
        exercise,
    );
}

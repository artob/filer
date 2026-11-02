// This is free and unencumbered software released into the public domain.

//! A dependency-free, structure-aware fuzzer: it mutates real volumes
//! where their structures are (the boot sector, the FATs, and directory
//! slots), with values chosen to hit boundaries, and walks each result
//! with `common::exercise`, which checks that every answer is consistent.
//!
//! Each `cargo test` runs a few hundred inputs. For longer runs, set
//! `FILER_FAT_FUZZ_ITERATIONS`; `FILER_FAT_FUZZ_SEED` picks another
//! sequence, and a failure prints the seed and iteration that reproduce it.
//! `fuzz/` has coverage-guided `cargo-fuzz` targets over the same walk.

mod common;

use common::{Image, Kind, exercise, file, unsparse};
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
        let u32_at =
            |at: usize| u64::from(u32::from_le_bytes(image[at..at + 4].try_into().unwrap()));
        let sector = u16_at(11);
        let reserved = u16_at(14) * sector;
        let fats = u64::from(image[16]);
        let fat_len = match u16_at(22) {
            0 => u32_at(36),
            sectors => sectors,
        } * sector;
        let root = u16_at(17) * 32;
        let data = reserved + fats * fat_len + root.div_ceil(sector) * sector;
        // The boot sector, the FATs, the root, and the first directories.
        let mut regions: Vec<Range<u64>> = Vec::new();
        regions.push(0..512);
        for copy in 0..fats {
            let start = reserved + copy * fat_len;
            regions.push(start..start + fat_len.min(4096));
        }
        if root > 0 {
            regions.push(data - root.div_ceil(sector) * sector..data);
        }
        // Directories are allocated early, so they lie near the start.
        regions.push(data..(data + 64 * 1024).min(image.len() as u64));
        Self {
            image: Arc::new(image),
            regions,
        }
    }
}

/// Values at the edges of what FAT structures hold.
const BYTES: [u8; 12] = [
    0, 1, 0x05, 0x0F, 0x10, 0x20, 0x2E, 0x40, 0x7F, 0x80, 0xE5, 0xFF,
];
const WORDS: [u32; 14] = [
    0,
    1,
    2,
    3,
    0x0FF6,
    0x0FF7,
    0x0FFF,
    0xFFF6,
    0xFFF7,
    0xFFFF,
    0x0FFF_FFF6,
    0x0FFF_FFF7,
    0x0FFF_FFFF,
    0xFFFF_FFFF,
];

fn corpus() -> Vec<Seed> {
    let mut seeds: Vec<Seed> = [
        &include_bytes!("fixtures/fat12.sparse")[..],
        include_bytes!("fixtures/fat16.sparse"),
        include_bytes!("fixtures/fat32.sparse"),
    ]
    .into_iter()
    .map(|sparse| Seed::new(unsparse(sparse)))
    .collect();
    for kind in [Kind::Fat12, Kind::Fat32] {
        let tree = || {
            vec![
                file("A long file name.txt", vec![7; 1500]),
                file("SHORT.TXT", "short"),
                common::dir("Sub", vec![file("nested.bin", vec![9; 3000])]),
            ]
        };
        seeds.push(Seed::new(Image::new(kind, tree()).bytes));
        seeds.push(Seed::new(Image::fragmented(kind, tree()).bytes));
    }
    seeds
}

/// The mutations to make, with those values.
const MUTATIONS: Mutations = Mutations::new(&BYTES, &WORDS).chunk(32);

#[test]
fn survives_mutated_volumes() {
    let corpus = corpus();
    Fuzz::new("FILER_FAT_FUZZ", 0xF17E_5EED)
        .iterations(300)
        .run(
            corpus.len(),
            |source, rng| MUTATIONS.mutate(&corpus[source].image, &corpus[source].regions, rng),
            exercise,
        );
}

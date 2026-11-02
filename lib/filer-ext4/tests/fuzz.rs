// This is free and unencumbered software released into the public domain.

//! A dependency-free, structure-aware fuzzer: it mutates real volumes
//! where their structures are (the superblock, the group descriptors,
//! the inode table, directories, indirect blocks, and attribute blocks),
//! with values chosen to hit boundaries, and walks each result with
//! `common::exercise`, which checks that every answer is consistent.
//!
//! Each `cargo test` runs a couple of hundred inputs. For longer runs, set
//! `FILER_EXT4_FUZZ_ITERATIONS`; `FILER_EXT4_FUZZ_SEED` picks another
//! sequence, and a failure prints the seed and iteration that reproduce
//! it. `fuzz/` has coverage-guided `cargo-fuzz` targets over the same walk.

mod common;

use common::{
    exercise::{Overlay, exercise},
    unsparse,
};
use std::{ops::Range, panic, sync::Arc};

/// SplitMix64: small, fast, and good enough to drive mutations.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }

    fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        items[self.below(items.len() as u64) as usize]
    }
}

/// A volume to mutate, and where its structures are.
struct Seed {
    image: Arc<Vec<u8>>,
    regions: Vec<Range<u64>>,
}

impl Seed {
    fn new(image: Vec<u8>) -> Self {
        let u32_at =
            |at: usize| u64::from(u32::from_le_bytes(image[at..at + 4].try_into().unwrap()));
        let block = 1024 << u32_at(1024 + 0x18);
        let first = u32_at(1024 + 0x14);
        let descriptors = (first + 1) * block;
        let inode_size = u64::from(u16::from_le_bytes([image[1024 + 0x58], image[1024 + 0x59]]));
        let inode_size = if u32_at(1024 + 0x4C) == 0 {
            128
        } else {
            inode_size
        };
        let table = u32_at(descriptors as usize + 8) * block;
        let len = image.len() as u64;
        // The superblock, the descriptors, and the first inodes.
        let mut regions: Vec<Range<u64>> = vec![
            1024..2048,
            descriptors..descriptors + block.min(1024),
            table..(table + 64 * inode_size).min(len),
        ];
        // The root directory's first block, and each attribute block.
        let root = table + inode_size;
        let root_block = u32_at(root as usize + 0x28) * block;
        regions.push(root_block..root_block + block);
        let mut at = block;
        while at + 4 <= len && regions.len() < 40 {
            if image[at as usize..at as usize + 4] == 0xEA02_0000u32.to_le_bytes() {
                regions.push(at..at + block);
            }
            at += block;
        }
        // And anywhere, now and then.
        regions.push(0..len);
        Self {
            image: Arc::new(image),
            regions,
        }
    }
}

/// Values at the edges of what ext2/3/4 structures hold.
const BYTES: [u8; 12] = [0, 1, 2, 7, 0x0C, 0x10, 0x40, 0x7F, 0x80, 0xEA, 0xFE, 0xFF];
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
    0xEA02_0000,
    0x7FFF_FFFF,
    0xFFFF_FFFF,
];

fn mutate(seed: &Seed, rng: &mut Rng) -> Overlay {
    let mut input = Overlay::new(seed.image.clone());
    for _ in 0..1 + rng.below(6) {
        let region = seed.regions[rng.below(seed.regions.len() as u64) as usize].clone();
        let at = region.start + rng.below(region.end - region.start);
        match rng.below(16) {
            0..=3 => {
                let byte = input.byte(at) ^ (1 << rng.below(8));
                input.patch(at, &[byte]);
            },
            4..=6 => input.patch(at, &[rng.pick(&BYTES)]),
            7..=8 => input.patch(at & !1, &(rng.pick(&WORDS) as u16).to_le_bytes()),
            9..=10 => input.patch(at & !3, &rng.pick(&WORDS).to_le_bytes()),
            11..=12 => {
                // Copy eight bytes from elsewhere in the region.
                let from = region.start + (rng.below(region.end - region.start) & !7);
                let bytes: Vec<u8> = (from..from + 8).map(|at| input.byte(at)).collect();
                input.patch(at & !7, &bytes);
            },
            13..=14 => {
                let bytes: Vec<u8> = (0..8).map(|_| rng.next() as u8).collect();
                input.patch(at & !7, &bytes);
            },
            _ => input.len = rng.below(input.len),
        }
    }
    input
}

fn corpus() -> Vec<Seed> {
    [
        &include_bytes!("fixtures/ext2-1k.sparse")[..],
        include_bytes!("fixtures/ext2-4k.sparse"),
        include_bytes!("fixtures/ext2-rev0.sparse"),
        include_bytes!("fixtures/ext2-attrs.sparse"),
        include_bytes!("fixtures/ext2-attrs-128.sparse"),
    ]
    .into_iter()
    .map(|sparse| Seed::new(unsparse(sparse)))
    .collect()
}

fn env(name: &str, default: u64) -> u64 {
    std::env::var(name).map_or(default, |value| value.parse().expect(name))
}

#[test]
fn survives_mutated_volumes() {
    let iterations = env("FILER_EXT4_FUZZ_ITERATIONS", 200);
    let seed = env("FILER_EXT4_FUZZ_SEED", 0xE47_5EED);
    let corpus = corpus();
    let (mut opened, mut entries, mut bytes, mut errors) = (0, 0, 0, 0);
    for iteration in 0..iterations {
        // Each input has its own generator, so that one reproduces alone.
        let mut rng = Rng(seed ^ iteration.wrapping_mul(0x2545_F491_4F6C_DD1D));
        let source = rng.below(corpus.len() as u64) as usize;
        let input = mutate(&corpus[source], &mut rng);
        match panic::catch_unwind(|| exercise(input)) {
            Ok(outcome) => {
                opened += usize::from(outcome.opened);
                entries += outcome.entries;
                bytes += outcome.bytes;
                errors += outcome.errors;
            },
            Err(panic) => {
                eprintln!(
                    "failed: FILER_EXT4_FUZZ_SEED={seed} at iteration {iteration} \
                     (corpus volume {source})"
                );
                panic::resume_unwind(panic);
            },
        }
    }
    // Shown with `--nocapture`: how deep the inputs got.
    println!(
        "{iterations} inputs: {opened} opened, {entries} entries listed, \
         {bytes} bytes read, {errors} errors"
    );
    assert!(opened > 0 && entries > 0 && bytes > 0, "inputs too shallow");
}

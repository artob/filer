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

use common::{
    Image, Kind,
    exercise::{Overlay, exercise},
    file, unsparse,
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
                // Copy a directory slot over another.
                let from = region.start + (rng.below(region.end - region.start) & !31);
                let slot: Vec<u8> = (from..from + 32).map(|at| input.byte(at)).collect();
                input.patch(at & !31, &slot);
            },
            13..=14 => {
                let slot: Vec<u8> = (0..32).map(|_| rng.next() as u8).collect();
                input.patch(at & !31, &slot);
            },
            _ => input.len = rng.below(input.len),
        }
    }
    input
}

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

fn env(name: &str, default: u64) -> u64 {
    std::env::var(name).map_or(default, |value| value.parse().expect(name))
}

#[test]
fn survives_mutated_volumes() {
    let iterations = env("FILER_FAT_FUZZ_ITERATIONS", 300);
    let seed = env("FILER_FAT_FUZZ_SEED", 0xF17E_5EED);
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
                    "failed: FILER_FAT_FUZZ_SEED={seed} at iteration {iteration} \
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

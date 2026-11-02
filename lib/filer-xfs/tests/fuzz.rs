// This is free and unencumbered software released into the public domain.

//! A dependency-free, structure-aware fuzzer: it mutates real volumes
//! where their structures are (the superblock, the inodes, directory
//! blocks, attribute blocks, and B+ tree blocks), with values chosen to
//! hit boundaries, and walks each result with `common::exercise`, which
//! checks that every answer is consistent.
//!
//! Each `cargo test` runs a couple of hundred inputs. For longer runs, set
//! `FILER_XFS_FUZZ_ITERATIONS`; `FILER_XFS_FUZZ_SEED` picks another
//! sequence, and a failure prints the seed and iteration that reproduce
//! it. `fuzz/` has coverage-guided `cargo-fuzz` targets over the same walk.

mod common;

use common::{
    Sparse,
    exercise::{Overlay, exercise},
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
    image: Arc<Sparse>,
    regions: Vec<Range<u64>>,
}

impl Seed {
    fn new(image: Sparse) -> Self {
        let bytes = |at: u64, n: usize| {
            let mut out = vec![0; n];
            image.copy(at, &mut out);
            out
        };
        let be = |at: u64, n: usize| {
            bytes(at, n)
                .iter()
                .fold(0u64, |value, &byte| value << 8 | u64::from(byte))
        };
        let block = be(4, 4);
        let ag_blocks = be(84, 4);
        let inode_size = be(104, 2);
        let (inopblog, agblklog) = (be(123, 1), be(124, 1));
        // The root inode, and the inodes around it.
        let root = be(56, 8);
        let ag = root >> (agblklog + inopblog);
        let ag_block = (root >> inopblog) & ((1 << agblklog) - 1);
        let slot = root & ((1 << inopblog) - 1);
        let inode = (ag * ag_blocks + ag_block) * block + slot * inode_size;
        let len = image.len();
        // The superblock, and the first inodes.
        let mut regions: Vec<Range<u64>> = vec![0..512, inode..(inode + 64 * inode_size).min(len)];
        // Each block of a directory, an attribute fork, or a B+ tree, by
        // its magic number.
        for at in image.sectors().filter(|at| at % block == 0) {
            let magic = be(at, 4);
            let short = be(at + 8, 2);
            let known = matches!(magic, 0x5844_3242 | 0x5844_3244 | 0x424D_4150)
                || matches!(short, 0xD2F1 | 0xD2FF | 0xFEBE | 0xFBEE);
            if known && regions.len() < 64 {
                regions.push(at..at + block);
            }
        }
        // And anywhere, now and then.
        regions.push(0..len);
        Self {
            image: Arc::new(image),
            regions,
        }
    }
}

/// Values at the edges of what XFS structures hold.
const BYTES: [u8; 12] = [0, 1, 2, 3, 7, 0x0F, 0x10, 0x7F, 0x80, 0xD2, 0xFE, 0xFF];
const WORDS: [u32; 12] = [
    0,
    1,
    8,
    0x80,
    0x0FFF,
    0xFBEE,
    0xFEBE,
    0xFFFF,
    0x5844_3244,
    0x424D_4150,
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
            7..=8 => input.patch(at & !1, &(rng.pick(&WORDS) as u16).to_be_bytes()),
            9..=10 => input.patch(at & !3, &rng.pick(&WORDS).to_be_bytes()),
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
        &include_bytes!("fixtures/xfs-512.sparse")[..],
        include_bytes!("fixtures/xfs-1k.sparse"),
        include_bytes!("fixtures/xfs-4k.sparse"),
        include_bytes!("fixtures/xfs-dirs.sparse"),
        include_bytes!("fixtures/xfs-attrs.sparse"),
        include_bytes!("fixtures/xfs-attrs-512.sparse"),
        include_bytes!("fixtures/xfs-ci.sparse"),
    ]
    .into_iter()
    .map(|sparse| Seed::new(Sparse::new(sparse)))
    .collect()
}

fn env(name: &str, default: u64) -> u64 {
    std::env::var(name).map_or(default, |value| value.parse().expect(name))
}

#[test]
fn survives_mutated_volumes() {
    let iterations = env("FILER_XFS_FUZZ_ITERATIONS", 200);
    let seed = env("FILER_XFS_FUZZ_SEED", 0x0F5_5EED);
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
                    "failed: FILER_XFS_FUZZ_SEED={seed} at iteration {iteration} \
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

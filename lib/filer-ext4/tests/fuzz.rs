// This is free and unencumbered software released into the public domain.

//! A dependency-free, structure-aware fuzzer: it mutates real volumes
//! where their structures are (the superblock, the group descriptors,
//! the inode table, directories and their hash indexes, indirect blocks,
//! extent tree nodes, attribute blocks, and the journal's superblock and
//! log of volumes that need recovery),
//! with values chosen to hit boundaries, and walks each result with
//! `common::exercise`, which checks that every answer is consistent.
//!
//! Each `cargo test` runs a couple of hundred inputs. For longer runs, set
//! `FILER_EXT4_FUZZ_ITERATIONS`; `FILER_EXT4_FUZZ_SEED` picks another
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
        // Hash indexes: roots, a `.` entry and information of 8 bytes, and
        // interior nodes, an unused entry spanning the block, then entries.
        let mut at = 0;
        while at + block <= len && regions.len() < 60 {
            let b = &image[at as usize..(at + block) as usize];
            let root = b[4..8] == [12, 0, 1, 2] && b[8] == b'.' && b[0x1D] == 8;
            let node = b[..4] == [0; 4]
                && u64::from(u16::from_le_bytes([b[4], b[5]])) == block.min(0xFFFF)
                && b[6] == 0
                && b[8..10] != [0, 0];
            // And extent tree nodes, of their magic number.
            let extents = b[..2] == 0xF30Au16.to_le_bytes();
            if root || node || extents {
                regions.push(at..at + block);
            }
            at += block;
        }
        // The journal's superblock and log: its blocks of the magic number.
        let mut at = 0;
        while at + 4 <= len && regions.len() < 80 {
            if image[at as usize..at as usize + 4] == 0xC03B_3998u32.to_be_bytes() {
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
const WORDS: [u32; 13] = [
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
    0xC03B_3998,
    0x7FFF_FFFF,
    0xFFFF_FFFF,
];

fn corpus() -> Vec<Seed> {
    [
        &include_bytes!("fixtures/ext2-1k.sparse")[..],
        include_bytes!("fixtures/ext2-4k.sparse"),
        include_bytes!("fixtures/ext2-rev0.sparse"),
        include_bytes!("fixtures/ext2-attrs.sparse"),
        include_bytes!("fixtures/ext2-attrs-128.sparse"),
        include_bytes!("fixtures/ext3-1k.sparse"),
        include_bytes!("fixtures/ext3-dirty-1k.sparse"),
        include_bytes!("fixtures/ext3-dirty-csum.sparse"),
        include_bytes!("fixtures/ext2-htree-tea.sparse"),
        include_bytes!("fixtures/ext3-extents-deep.sparse"),
        include_bytes!("fixtures/ext4-1k.sparse"),
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
    Fuzz::new("FILER_EXT4_FUZZ", 0xE47_5EED).run(
        corpus.len(),
        |source, rng| MUTATIONS.mutate(&corpus[source].image, &corpus[source].regions, rng),
        exercise,
    );
}

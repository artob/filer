// This is free and unencumbered software released into the public domain.

//! A dependency-free, structure-aware fuzzer: it mutates real volumes
//! where their structures are (the superblock, the inodes, directory
//! blocks, attribute blocks, and B+ tree blocks), with values chosen to
//! hit boundaries, and the log, which recovery replays, and walks each
//! result with `common::exercise`, which
//! checks that every answer is consistent. On version 5 volumes, it mostly
//! reseals the checksum of what it mutated, so that the mutation gets past
//! it to what parses it.
//!
//! Each `cargo test` runs a couple of hundred inputs. For longer runs, set
//! `FILER_XFS_FUZZ_ITERATIONS`; `FILER_XFS_FUZZ_SEED` picks another
//! sequence, and a failure prints the seed and iteration that reproduce
//! it. `fuzz/` has coverage-guided `cargo-fuzz` targets over the same walk.

mod common;

use common::{Overlay, Sparse, exercise};
use filer_spec::{
    Image,
    fuzz::{self, Fuzz, Mutations, Order},
};
use std::{ops::Range, sync::Arc};

/// A region of a volume to mutate, and, where it is version 5 metadata,
/// the size of each object in it and where in each its CRC is.
#[derive(Clone)]
struct Region {
    range: Range<u64>,
    crc: Option<(u64, u64)>,
}

impl fuzz::Region for Region {
    fn range(&self) -> Range<u64> {
        self.range.clone()
    }
}

/// A volume to mutate, and where its structures are.
struct Seed {
    image: Arc<Sparse>,
    regions: Vec<Region>,
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
        let len = image.length();
        let v5 = be(100, 2) & 0xF == 5;
        let sealed = |size, crc| v5.then_some((size, crc));
        // The superblock, and the first inodes.
        let mut regions = vec![
            Region {
                range: 0..512,
                crc: sealed(be(102, 2), 224),
            },
            Region {
                range: inode..(inode + 64 * inode_size).min(len),
                crc: sealed(inode_size, 100),
            },
        ];
        // Each block of a directory, an attribute fork, a B+ tree, or a
        // remote value or symlink target, by its magic number; version 5's
        // with where their CRCs are.
        for at in image.sectors().filter(|at| at % block == 0) {
            let crc = match (be(at, 4), be(at + 8, 2)) {
                (0x5844_3242 | 0x5844_3244 | 0x424D_4150, _)
                | (_, 0xD2F1 | 0xD2FF | 0xFEBE | 0xFBEE) => None,
                (0x5844_4233 | 0x5844_4433, _) => Some(4),
                (0x424D_4133, _) => Some(64),
                (0x5841_524D | 0x5853_4C4D, _) | (_, 0x3DF1 | 0x3DFF | 0x3EBE | 0x3BEE) => Some(12),
                _ => continue,
            };
            if regions.len() < 64 {
                regions.push(Region {
                    range: at..at + block,
                    crc: crc.map(|crc| (block, crc)),
                });
            }
        }
        // The log's first records, which recovery reads if they are past
        // its last unmount record.
        let log_start = be(48, 8);
        if log_start != 0 {
            let ag = log_start >> agblklog;
            let log = (ag * ag_blocks + (log_start & ((1 << agblklog) - 1))) * block;
            regions.push(Region {
                range: log..(log + 128 * 1024).min(len),
                crc: None,
            });
        }
        // And anywhere, now and then.
        regions.push(Region {
            range: 0..len,
            crc: None,
        });
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

/// Sets the CRC of the object of `size` bytes from `start` in `input`, at
/// `crc` in it.
fn reseal(input: &mut Overlay, start: u64, size: u64, crc: u64) {
    let mut bytes: Vec<u8> = (start..start + size).map(|at| input.byte(at)).collect();
    bytes[crc as usize..crc as usize + 4].fill(0);
    let sum = filer_api::checksum::crc32c(&bytes);
    input.patch(start + crc, &sum.to_le_bytes());
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
        include_bytes!("fixtures/xfs5-1k.sparse"),
        include_bytes!("fixtures/xfs5-dirs.sparse"),
        include_bytes!("fixtures/xfs5-attrs.sparse"),
        include_bytes!("fixtures/xfs5d-later.sparse"),
        include_bytes!("fixtures/xfs5n-attrs.sparse"),
        include_bytes!("fixtures/xfs-dirty.sparse"),
        include_bytes!("fixtures/xfs5d-dirty.sparse"),
    ]
    .into_iter()
    .map(|sparse| Seed::new(Sparse::new(sparse)))
    .collect()
}

/// The mutations to make, with those values.
const MUTATIONS: Mutations = Mutations::new(&BYTES, &WORDS).order(Order::Big);

#[test]
fn survives_mutated_volumes() {
    let corpus = corpus();
    Fuzz::new("FILER_XFS_FUZZ", 0x0F5_5EED).run(
        corpus.len(),
        |source, rng| {
            let seed = &corpus[source];
            MUTATIONS.mutate_then(&seed.image, &seed.regions, rng, |input, region, at, rng| {
                // Mostly past the checksum.
                if let Some((size, offset)) = region.crc
                    && rng.below(8) != 0
                {
                    let start = region.range.start + (at - region.range.start) / size * size;
                    reseal(input, start, size, offset);
                }
            })
        },
        exercise,
    );
}

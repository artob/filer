// This is free and unencumbered software released into the public domain.

//! A dependency-free, structure-aware fuzzer: it mutates real images
//! where their structures are (the volume descriptors, and the path
//! tables and directories after them), with values chosen to hit
//! boundaries, and walks each result with `common::exercise`, which
//! checks that every answer is consistent.
//!
//! Each `cargo test` runs a couple of hundred inputs. For longer runs, set
//! `FILER_CDFS_FUZZ_ITERATIONS`; `FILER_CDFS_FUZZ_SEED` picks another
//! sequence, and a failure prints the seed and iteration that reproduce
//! it. `fuzz/` has coverage-guided `cargo-fuzz` targets over the same
//! walk.

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
        // Its first file's data, after the descriptors, the path tables,
        // and the directories, which `xorriso` writes first.
        let structures = first_file(image).unwrap_or(image.len() as u64);
        let len = image.len() as u64;
        Self {
            image: Arc::new(image.to_vec()),
            regions: vec![
                // The descriptors, the directories, and anywhere, now and
                // then.
                32_768..36_864,
                36_864..structures,
                0..len,
            ],
        }
    }
}

/// Returns where the first file's data begins in `image`: the lowest
/// extent of a record in the root directory with data, past it.
fn first_file(image: &[u8]) -> Option<u64> {
    let root = u32::from_le_bytes(image.get(32_768 + 158..32_768 + 162)?.try_into().ok()?);
    let sector = image.get(root as usize * 2048..root as usize * 2048 + 2048)?;
    let mut at = 0;
    let mut first = None::<u64>;
    while let Some(&len) = sector.get(at).filter(|&&len| len != 0) {
        let record = sector.get(at..at + usize::from(len))?;
        let extent = u64::from(u32::from_le_bytes(record.get(2..6)?.try_into().ok()?));
        let size = u32::from_le_bytes(record.get(10..14)?.try_into().ok()?);
        if record.get(25)? & 2 == 0 && size > 0 && extent > u64::from(root) {
            first = Some(first.map_or(extent, |first| first.min(extent)));
        }
        at += usize::from(len);
    }
    first.map(|block| block * 2048)
}

/// Values at the edges of what ISO 9660 structures hold: lengths of
/// records and names, flags, signatures' lengths, and blocks.
const BYTES: [u8; 12] = [0, 1, 2, 3, 4, 7, 0x22, 0x80, 0x81, 0xBE, 0xEF, 0xFF];
const WORDS: [u32; 12] = [
    0,
    1,
    16,
    17,
    18,
    0x0800,
    0x0801,
    0xFFFF,
    0x0001_0000,
    0x7FFF_FFFF,
    0xFFFF_FFFE,
    0xFFFF_FFFF,
];

fn corpus() -> Vec<Seed> {
    [
        &include_bytes!("fixtures/spec.iso")[..],
        include_bytes!("fixtures/rock-ridge.iso"),
        include_bytes!("fixtures/rock-ridge-joliet.iso"),
        include_bytes!("fixtures/joliet.iso"),
        include_bytes!("fixtures/plain.iso"),
        include_bytes!("fixtures/iso9660-1999.iso"),
        include_bytes!("fixtures/zisofs.iso"),
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
    Fuzz::new("FILER_CDFS_FUZZ", 0x0C0D_F5ED).run(
        corpus.len(),
        |source, rng| MUTATIONS.mutate(&corpus[source].image, &corpus[source].regions, rng),
        exercise,
    );
}

// This is free and unencumbered software released into the public domain.

//! A dependency-free, structure-aware fuzzer: it mutates archives where
//! their headers and names are, with values chosen to hit boundaries, and
//! walks each result with `common::exercise`.
//!
//! Each `cargo test` runs a few hundred inputs. For longer runs, set
//! `FILER_CPIO_FUZZ_ITERATIONS`; `FILER_CPIO_FUZZ_SEED` picks another
//! sequence, and a failure prints the seed and iteration that reproduce
//! it. `fuzz/` has a coverage-guided `cargo-fuzz` target over the same
//! walk.

mod common;

use common::{FORMATS, Writer, exercise};
use filer_spec::{
    Image,
    fuzz::{Fuzz, Mutations},
};
use std::{ops::Range, sync::Arc};

/// Values at the edges of what cpio headers hold: digits of each radix,
/// the binary magic number's bytes, and NUL.
const BYTES: [u8; 12] = [
    0, b'0', b'1', b'7', b'8', b'9', b'A', b'F', b'G', 0x71, 0xC7, 0xFF,
];
const WORDS: [u32; 9] = [
    0,
    0x71C7,
    0xC771,
    0xFFFF,
    u32::from_le_bytes(*b"0000"),
    u32::from_le_bytes(*b"FFFF"),
    u32::from_le_bytes(*b"7777"),
    u32::from_le_bytes(*b"0707"),
    u32::MAX,
];

const MUTATIONS: Mutations = Mutations::new(&BYTES, &WORDS);

/// An archive to mutate, and where its headers are: from each magic
/// number, the longest header and a name.
struct Seed {
    archive: Arc<Vec<u8>>,
    regions: Vec<Range<u64>>,
}

impl Seed {
    fn new(archive: Vec<u8>) -> Self {
        let len = archive.len() as u64;
        let mut regions = Vec::new();
        for at in 0..archive.len().saturating_sub(1) {
            let magic = archive[at..].starts_with(b"07070")
                || archive[at..].starts_with(&[0xC7, 0x71])
                || archive[at..].starts_with(&[0x71, 0xC7]);
            if magic {
                regions.push(at as u64..(at as u64 + 140).min(len));
            }
        }
        regions.push(0..len);
        Self {
            archive: Arc::new(archive),
            regions,
        }
    }
}

fn corpus() -> Vec<Seed> {
    let data = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data");
    let fixtures = [
        "gnu-newc.cpio",
        "gnu-crc.cpio",
        "gnu-odc.cpio",
        "gnu-bin.cpio",
        "bsd-newc.cpio",
        "bsd-odc.cpio",
        "initramfs.img",
    ];
    let mut seeds: Vec<Seed> = fixtures
        .iter()
        .map(|name| Seed::new(std::fs::read(format!("{data}/{name}")).unwrap()))
        .collect();
    for format in FORMATS {
        let archive = Writer::new(format)
            .dir(b"dir")
            .file(b"dir/file", b"contents")
            .symlink(b"link", b"dir/file")
            .hardlinks(&[b"one", b"two"], b"linked")
            .finish();
        seeds.push(Seed::new(archive));
    }
    seeds
}

#[test]
fn survives_mutated_archives() {
    let corpus = corpus();
    Fuzz::new("FILER_CPIO_FUZZ", 0xC910_5EED)
        .iterations(400)
        .run(
            corpus.len(),
            |source, rng| {
                let seed = &corpus[source];
                let input = MUTATIONS.mutate(&seed.archive, &seed.regions, rng);
                let mut bytes = vec![0; input.length() as usize];
                input.copy(0, &mut bytes);
                bytes
            },
            exercise,
        );
}

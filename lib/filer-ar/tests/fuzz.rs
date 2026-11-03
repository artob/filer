// This is free and unencumbered software released into the public domain.

//! A dependency-free, structure-aware fuzzer: it mutates archives where
//! their headers and name tables are, with values chosen to hit
//! boundaries, and walks each result with `common::exercise`.
//!
//! Each `cargo test` runs a few hundred inputs. For longer runs, set
//! `FILER_AR_FUZZ_ITERATIONS`; `FILER_AR_FUZZ_SEED` picks another
//! sequence, and a failure prints the seed and iteration that reproduce
//! it. `fuzz/` has a coverage-guided `cargo-fuzz` target over the same
//! walk.

mod common;

use common::{DIALECTS, Writer, exercise};
use filer_spec::{
    Image,
    fuzz::{Fuzz, Mutations},
};
use std::{ops::Range, sync::Arc};

/// Values at the edges of what ar headers hold: padding, digits, and the
/// characters that begin special names.
const BYTES: [u8; 12] = [
    0, b' ', b'#', b'/', b'0', b'1', b'9', b'`', b'\n', 0x7F, 0x80, 0xFF,
];
const WORDS: [u32; 8] = [
    0,
    u32::from_le_bytes(*b"    "),
    u32::from_le_bytes(*b"9999"),
    u32::from_le_bytes(*b"#1/9"),
    u32::from_le_bytes(*b"/0  "),
    u32::from_le_bytes(*b"//  "),
    u32::from_le_bytes(*b"/\n/\n"),
    u32::MAX,
];

const MUTATIONS: Mutations = Mutations::new(&BYTES, &WORDS);

/// An archive to mutate, and where its headers are: wherever a header's
/// closing `` `\n `` is, and its first bytes of data, which may be a name.
struct Seed {
    archive: Arc<Vec<u8>>,
    regions: Vec<Range<u64>>,
}

impl Seed {
    fn new(archive: Vec<u8>) -> Self {
        let mut regions = Vec::new();
        for at in 58..archive.len().saturating_sub(1) {
            if &archive[at..at + 2] == b"`\n" {
                let start = (at - 58) as u64;
                regions.push(start..(at as u64 + 34).min(archive.len() as u64));
            }
        }
        regions.push(0..archive.len() as u64);
        Self {
            archive: Arc::new(archive),
            regions,
        }
    }
}

fn corpus() -> Vec<Seed> {
    let data = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data");
    let mut seeds: Vec<Seed> = ["gnu.a", "bsd.a", "coff.lib", "darwin.a", "hello.deb"]
        .iter()
        .map(|name| Seed::new(std::fs::read(format!("{data}/{name}")).unwrap()))
        .collect();
    for dialect in DIALECTS {
        let archive = Writer::new(dialect)
            .file(b"odd", b"odd")
            .file(b"dir/even.o", b"even")
            .file(b"0123456789abcde", b"x")
            .finish();
        seeds.push(Seed::new(archive));
    }
    seeds
}

#[test]
fn survives_mutated_archives() {
    let corpus = corpus();
    Fuzz::new("FILER_AR_FUZZ", 0xA2_5EED).iterations(400).run(
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

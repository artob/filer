// This is free and unencumbered software released into the public domain.

//! A structure-aware fuzzer of the decoder: it mutates streams, where
//! their headers and trailers are more often, with values chosen to hit
//! boundaries, and checks that each decodes the same in pieces of a random
//! size as whole, and as zlib decodes it, through `zlib-rs`: to the same
//! bytes, consuming the same, or not at all.
//!
//! Each `cargo test` runs a few hundred inputs. For longer runs, set
//! `DECODE_ZLIB_FUZZ_ITERATIONS`; `DECODE_ZLIB_FUZZ_SEED` picks another
//! sequence, and a failure prints the seed and iteration that reproduce
//! it. `fuzz/` has a coverage-guided `cargo-fuzz` target over the same
//! check.

mod common;

use common::{agree, zlib, zlib_at};
use filer_spec::{
    Image, Outcome,
    fuzz::{Fuzz, Mutations},
};
use std::sync::Arc;

/// Values at the edges of what zlib headers hold: methods, windows, the
/// dictionary flag, and level hints.
const BYTES: [u8; 12] = [0, 1, 7, 8, 0x20, 0x78, 0x7F, 0x88, 0x9C, 0xC0, 0xDA, 0xFF];
const WORDS: [u32; 6] = [0, 1, 0xFF, 0xFFFF, 0x7FFF_FFFF, u32::MAX];

/// The Adler-32 catches most mutations of the data, so few at once.
const MUTATIONS: Mutations = Mutations::new(&BYTES, &WORDS).aligned(false).most(2);

#[test]
fn decodes_mutations_as_zlib_does() {
    let text: Vec<u8> = (0..2000_u32)
        .flat_map(|n| format!("{n} ").into_bytes())
        .collect();
    let corpus = [
        include_bytes!("data/numbers.zz").to_vec(),
        include_bytes!("data/window.zz").to_vec(),
        zlib_at(b"some data, some data", 6),
        [zlib_at(&text, 0), b"after".to_vec()].concat(),
        zlib_at(b"", 9),
    ]
    .map(|stream| {
        let len = stream.len() as u64;
        let regions = vec![0..2, len.saturating_sub(4)..len, 0..len];
        (Arc::new(stream), regions)
    });
    Fuzz::new("DECODE_ZLIB_FUZZ", 0x1950_5EED)
        .iterations(400)
        .run(
            corpus.len(),
            |source, rng| {
                let (seed, regions) = &corpus[source];
                let input = MUTATIONS.mutate(seed, regions, rng);
                let mut stream = vec![0; input.length() as usize];
                input.copy(0, &mut stream);
                (stream, 1 + rng.below(64) as usize)
            },
            |(stream, piece)| {
                let ours = agree(&stream, piece).ok();
                let theirs = zlib(&stream);
                assert!(
                    ours == theirs,
                    "ours: {:?}; zlib's: {:?}",
                    ours.as_ref().map(|(data, consumed)| (data.len(), consumed)),
                    theirs
                        .as_ref()
                        .map(|(data, consumed)| (data.len(), consumed)),
                );
                Outcome {
                    opened: ours.is_some(),
                    entries: 1,
                    bytes: ours.as_ref().map_or(0, |(data, _)| data.len() as u64),
                    errors: usize::from(ours.is_none()),
                    ..Outcome::default()
                }
            },
        );
}

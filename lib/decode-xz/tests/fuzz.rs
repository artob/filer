// This is free and unencumbered software released into the public domain.

//! A structure-aware fuzzer of the decoder: it mutates xz files, where
//! their headers, indexes, and footers are more often, with values chosen
//! to hit boundaries, and checks that each decodes the same in pieces of a
//! random size as whole, and as `lzma-rust2` decodes it: to the same
//! bytes, or not at all.
//!
//! Each `cargo test` runs a few hundred inputs. For longer runs, set
//! `DECODE_XZ_FUZZ_ITERATIONS`; `DECODE_XZ_FUZZ_SEED` picks another
//! sequence, and a failure prints the seed and iteration that reproduce
//! it. `fuzz/` has coverage-guided `cargo-fuzz` targets over the same
//! checks.

mod common;

use common::{agree, theirs};
use filer_spec::{
    Image, Outcome,
    fuzz::{Fuzz, Mutations},
};
use std::{ops::Range, sync::Arc};

/// Values at the edges of what xz's fields hold: magic bytes, flags,
/// check types, filter IDs, and sizes.
const BYTES: [u8; 14] = [
    0, 1, 2, 3, 4, 0x0A, 0x21, 0x40, 0x80, 0xC0, 0xFD, 0x59, 0x5A, 0xFF,
];
const WORDS: [u32; 6] = [0, 1, 0xFFFF, 0x7FFF_FFFF, 0xFFFF_FFFE, u32::MAX];

/// The checks catch most mutations, so few at once.
const MUTATIONS: Mutations = Mutations::new(&BYTES, &WORDS).aligned(false).most(2);

/// A file to mutate, and where to mutate it more often.
type Seed = (Arc<Vec<u8>>, Vec<Range<u64>>);

#[test]
fn decodes_mutations_as_lzma_rust2_does() {
    let seeds: Vec<Seed> = [
        &include_bytes!("data/numbers.xz")[..],
        include_bytes!("data/checks.xz"),
        include_bytes!("data/blocks.xz"),
    ]
    .into_iter()
    .map(|stream| {
        let len = stream.len() as u64;
        // The stream's header and first block's, and its index and footer.
        let regions = vec![0..32, len.saturating_sub(64)..len, 0..len];
        (Arc::new(stream.to_vec()), regions)
    })
    .collect();
    Fuzz::new("DECODE_XZ_FUZZ", 0x0F2A_5EED)
        .iterations(300)
        .run(
            seeds.len(),
            |source, rng| {
                let (seed, regions) = &seeds[source];
                let input = MUTATIONS.mutate(seed, regions, rng);
                let mut stream = vec![0; input.length() as usize];
                input.copy(0, &mut stream);
                (stream, 1 + rng.below(64) as usize)
            },
            |(stream, piece)| {
                let ours = agree(&stream, piece).ok().map(|(data, _)| data);
                let theirs = theirs(&stream);
                assert!(
                    ours == theirs,
                    "ours: {:?}; lzma-rust2's: {:?}",
                    ours.as_ref().map(Vec::len),
                    theirs.as_ref().map(Vec::len),
                );
                Outcome {
                    opened: ours.is_some(),
                    entries: 1,
                    bytes: ours.as_ref().map_or(0, |data| data.len() as u64),
                    errors: usize::from(ours.is_none()),
                    ..Outcome::default()
                }
            },
        );
}

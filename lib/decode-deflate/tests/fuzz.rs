// This is free and unencumbered software released into the public domain.

//! A structure-aware fuzzer of the decoder: it mutates streams, at their
//! starts, where block headers and code descriptions are, more often, with
//! values chosen to hit boundaries, and checks that each decodes the same
//! in pieces of a random size as whole, and as zlib decodes it, through
//! `zlib-rs`: to the same bytes, consuming the same, or not at all.
//!
//! Each `cargo test` runs a few hundred inputs. For longer runs, set
//! `DECODE_DEFLATE_FUZZ_ITERATIONS`; `DECODE_DEFLATE_FUZZ_SEED` picks
//! another sequence, and a failure prints the seed and iteration that
//! reproduce it. `fuzz/` has coverage-guided `cargo-fuzz` targets over the
//! same checks.

mod common;

use common::{agree, zlib};
use filer_spec::{
    Image, Outcome,
    fuzz::{Fuzz, Mutations},
};
use std::sync::Arc;

/// Values at the edges of what Deflate's fields hold: block types, code
/// counts, and lengths.
const BYTES: [u8; 12] = [0, 1, 2, 3, 4, 5, 6, 7, 0x1F, 0x80, 0xFE, 0xFF];
const WORDS: [u32; 6] = [0, 1, 0xFFFF, 0x8000, 0x7FFF, u32::MAX];

/// Fields are packed bits, and few mutations at once leave more streams
/// decoding far enough to compare.
const MUTATIONS: Mutations = Mutations::new(&BYTES, &WORDS).aligned(false).most(2);

#[test]
fn decodes_mutations_as_zlib_does() {
    let seeds = [
        &include_bytes!("data/text-9.deflate")[..],
        include_bytes!("data/text-fixed.deflate"),
        include_bytes!("data/text-small-window.deflate"),
        include_bytes!("data/lz-9.deflate"),
        include_bytes!("data/lz-rle.deflate"),
        include_bytes!("data/stored.deflate"),
        include_bytes!("data/zeros-9.deflate"),
    ]
    .map(|stream| Arc::new(stream.to_vec()));
    Fuzz::new("DECODE_DEFLATE_FUZZ", 0x1951_5EED)
        .iterations(400)
        .run(
            seeds.len(),
            |source, rng| {
                let seed = &seeds[source];
                let regions = [0..64.min(seed.len() as u64), 0..seed.len() as u64];
                let input = MUTATIONS.mutate(seed, &regions, rng);
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

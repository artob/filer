// This is free and unencumbered software released into the public domain.

//! A structure-aware fuzzer of the decoder: it mutates LZMA and LZMA2
//! streams, at their starts and LZMA2's chunk headers more often, with
//! values chosen to hit boundaries, and checks that each decodes the same
//! in pieces of a random size as whole, and as `lzma-rust2` decodes it:
//! to the same bytes, or not at all; but for LZMA of a known size, which
//! `lzma-rust2` may decode where this crate, as the LZMA SDK, refuses.
//!
//! Each `cargo test` runs a few hundred inputs. For longer runs, set
//! `DECODE_LZMA_FUZZ_ITERATIONS`; `DECODE_LZMA_FUZZ_SEED` picks another
//! sequence, and a failure prints the seed and iteration that reproduce
//! it. `fuzz/` has coverage-guided `cargo-fuzz` targets over the same
//! checks.

mod common;

use common::{agree, lzma, lzma2, noise, options, text, theirs_lzma, theirs_lzma2};
use decode_lzma::Decoder;
use filer_spec::{
    Image, Outcome,
    fuzz::{Fuzz, Mutations},
};
use lzma_rust2::Lzma2Options;
use std::{num::NonZeroU64, ops::Range, sync::Arc};

/// Values at the edges of what LZMA2's control bytes and sizes, and the
/// range coder's bytes, hold.
const BYTES: [u8; 12] = [0, 1, 2, 3, 0x7F, 0x80, 0xA0, 0xC0, 0xE0, 0xFE, 0xFF, 0x5D];
const WORDS: [u32; 5] = [0, 1, 0xFFFF, 0x8000, u32::MAX];

/// Few mutations at once leave more streams decoding far enough to
/// compare.
const MUTATIONS: Mutations = Mutations::new(&BYTES, &WORDS).aligned(false).most(2);

/// A stream to mutate, where to mutate it more often, and what it is.
type Seed = (Arc<Vec<u8>>, Vec<Range<u64>>, Kind);

/// What a seed is.
#[derive(Clone, Copy, Debug)]
enum Kind {
    /// LZMA with these properties, of this size, if known.
    Lzma([u8; 5], Option<u64>),
    /// LZMA2 with this dictionary's property.
    Lzma2(u8),
}

#[test]
fn decodes_mutations_as_lzma_rust2_does() {
    let data = [text(6000, 20), noise(300, 21), text(3000, 22)].concat();
    let size = Some(data.len() as u64);
    let options = options(6, 3, 0, 2, 1 << 16);
    let marked = lzma(&data, &options, true);
    let unmarked = lzma(&data, &options, false);
    let mut chunked = Lzma2Options::with_preset(6);
    chunked.set_chunk_size(NonZeroU64::new(2000));
    let (chunks, property) = lzma2(&data, chunked);
    let lzma2_regions: Vec<Range<u64>> = vec![0..8, 0..chunks.len() as u64];
    let seeds: Vec<Seed> = vec![
        (
            Arc::new(marked.stream.clone()),
            vec![0..16, 0..marked.stream.len() as u64],
            Kind::Lzma(marked.properties, None),
        ),
        (
            Arc::new(unmarked.stream.clone()),
            vec![0..16, 0..unmarked.stream.len() as u64],
            Kind::Lzma(unmarked.properties, size),
        ),
        (Arc::new(chunks), lzma2_regions, Kind::Lzma2(property)),
    ];
    Fuzz::new("DECODE_LZMA_FUZZ", 0x1A2A_5EED)
        .iterations(300)
        .run(
            seeds.len(),
            |source, rng| {
                let (seed, regions, kind) = &seeds[source];
                let input = MUTATIONS.mutate(seed, regions, rng);
                let mut stream = vec![0; input.length() as usize];
                input.copy(0, &mut stream);
                (stream, *kind, 1 + rng.below(64) as usize)
            },
            |(stream, kind, piece)| {
                let (ours, theirs) = match kind {
                    Kind::Lzma(properties, size) => (
                        agree(|| Decoder::lzma(properties, size), &stream, piece).ok(),
                        theirs_lzma(&stream, properties, size),
                    ),
                    Kind::Lzma2(property) => (
                        agree(|| Decoder::lzma2(property), &stream, piece).ok(),
                        theirs_lzma2(&stream, property),
                    ),
                };
                let data = |decoded: &Option<(Vec<u8>, usize)>| {
                    decoded.as_ref().map(|(data, _)| data.clone())
                };
                // Of a known size, `lzma-rust2` ends there without checking
                // that the range coder does, as the LZMA SDK does, and so
                // decodes what this crate refuses.
                let lax = matches!(kind, Kind::Lzma(_, Some(_))) && ours.is_none();
                assert!(
                    lax || data(&ours) == data(&theirs),
                    "{kind:?}: ours: {:?}; lzma-rust2's: {:?}",
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

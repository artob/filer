// This is free and unencumbered software released into the public domain.

//! A structure-aware fuzzer of the decoder: it mutates streams, where
//! members' headers and trailers are more often, with values chosen to hit
//! boundaries, and checks that each decodes the same in pieces of a random
//! size as whole, and as zlib decodes it, through `zlib-rs`: to the same
//! bytes, consuming the same, or not at all.
//!
//! Each `cargo test` runs a few hundred inputs. For longer runs, set
//! `DECODE_GZIP_FUZZ_ITERATIONS`; `DECODE_GZIP_FUZZ_SEED` picks another
//! sequence, and a failure prints the seed and iteration that reproduce
//! it. `fuzz/` has a coverage-guided `cargo-fuzz` target over the same
//! check.

mod common;

use common::{Member, agree, zlib};
use filer_spec::{
    Image, Outcome,
    fuzz::{Fuzz, Mutations},
};
use std::{ops::Range, sync::Arc};

/// Values at the edges of what gzip headers and trailers hold: the magic
/// number's bytes, the method, and each flag.
const BYTES: [u8; 12] = [0, 1, 7, 8, 0x02, 0x04, 0x08, 0x10, 0x1F, 0x8B, 0xE0, 0xFF];
const WORDS: [u32; 7] = [0, 1, 0xFF, 0xFFFF, 0x8B1F, 0x7FFF_FFFF, u32::MAX];

/// Fields lie anywhere after a name; the checksums catch most mutations,
/// so few at once.
const MUTATIONS: Mutations = Mutations::new(&BYTES, &WORDS).aligned(false).most(2);

/// Returns the stream of `members`, each with its data, and `garbage`
/// after, and where its headers and trailers are.
fn seed(members: &[(Member<'_>, &[u8])], garbage: &[u8]) -> (Vec<u8>, Vec<Range<u64>>) {
    let mut stream = Vec::new();
    let mut regions = Vec::new();
    for (member, data) in members {
        let at = stream.len() as u64;
        stream.extend(member.gzip(data));
        regions.push(at..at + member.header().len() as u64);
        regions.push(stream.len() as u64 - 8..stream.len() as u64);
    }
    stream.extend_from_slice(garbage);
    (stream, regions)
}

#[test]
fn decodes_mutations_as_zlib_does() {
    let full = Member {
        extra: Some(b"AB\x02\x00hi"),
        name: Some(b"name.txt"),
        comment: Some(b"comment"),
        header_crc: true,
    };
    let numbers = include_bytes!("data/numbers.gz").to_vec();
    let len = numbers.len() as u64;
    let corpus = [
        (numbers, vec![0..32, len - 30..len]),
        seed(&[(full, b"some data, some data")], b""),
        seed(
            &[
                (Member::default(), b"one"),
                (full, b""),
                (Member::default(), b"two"),
            ],
            b"garbage",
        ),
    ]
    .map(|(stream, mut regions)| {
        regions.push(0..stream.len() as u64);
        (Arc::new(stream), regions)
    });
    Fuzz::new("DECODE_GZIP_FUZZ", 0x1952_5EED)
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

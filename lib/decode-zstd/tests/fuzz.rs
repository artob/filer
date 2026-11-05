// This is free and unencumbered software released into the public domain.

//! A structure-aware fuzzer of the decoder: it mutates streams where their
//! headers and checksums are, with values chosen to hit boundaries, and
//! checks that each decodes the same in pieces of a random size as whole,
//! and as `ruzstd` decodes it: to the same bytes, or not at all, but where
//! it decodes what zstd refuses (`common::agrees_with_ruzstd`).
//!
//! Each `cargo test` runs a few hundred inputs. For longer runs, set
//! `DECODE_ZSTD_FUZZ_ITERATIONS`; `DECODE_ZSTD_FUZZ_SEED` picks another
//! sequence, and a failure prints the seed and iteration that reproduce
//! it. `fuzz/` has coverage-guided `cargo-fuzz` targets over the same
//! checks.

mod common;

use common::{Block, agree, agrees_with_ruzstd, skippable, zstd};
use filer_spec::{
    Image, Outcome,
    fuzz::{Fuzz, Mutations},
};
use std::{ops::Range, sync::Arc};

/// A stream to mutate, and where its headers and checksums are.
struct Seed {
    stream: Arc<Vec<u8>>,
    regions: Vec<Range<u64>>,
}

impl Seed {
    fn new(stream: Vec<u8>, mut regions: Vec<Range<u64>>) -> Self {
        regions.push(0..stream.len() as u64);
        Self {
            stream: Arc::new(stream),
            regions,
        }
    }
}

/// Mutates streams of `corpus` with `mutations`, from `seed`, and checks
/// that each decodes alike in pieces and whole, and as `ruzstd` does.
fn run(seed: u64, corpus: &[Seed], mutations: Mutations) {
    Fuzz::new("DECODE_ZSTD_FUZZ", seed).iterations(400).run(
        corpus.len(),
        |source, rng| {
            let seed = &corpus[source];
            let input = mutations.mutate(&seed.stream, &seed.regions, rng);
            let mut bytes = vec![0; input.length() as usize];
            input.copy(0, &mut bytes);
            (bytes, 1 + rng.below(64) as usize)
        },
        |(stream, piece)| {
            let ours = agree(&stream, piece).ok();
            assert!(
                agrees_with_ruzstd(&stream, ours.as_deref()),
                "ours: {:?}; ruzstd's: {:?}",
                ours.as_ref().map(Vec::len),
                common::ruzstd(&stream).map(|(data, _)| data.len()),
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

/// Values at the edges of what zstd frame and block headers hold: the
/// magic numbers' bytes, each flag, and block types.
const BYTES: [u8; 12] = [0, 1, 2, 4, 6, 0x08, 0x20, 0x28, 0x50, 0xC0, 0xFD, 0xFF];
const WORDS: [u32; 8] = [
    0,
    1,
    0xFFFF,
    0x2_0000 << 3,
    0x184D_2A50,
    0xFD2F_B528,
    0x7FFF_FFFF,
    u32::MAX,
];

/// Headers are packed; the checksums catch most mutations, so few at
/// once.
const MUTATIONS: Mutations = Mutations::new(&BYTES, &WORDS).aligned(false).most(2);

/// Returns `stream`, without its frames' checksums unless `checked`,
/// and where its frame headers, block headers, the starts of its
/// compressed blocks, where their literals and tables are described,
/// and its checksums are.
fn seed(stream: &[u8], checked: bool) -> Seed {
    let (mut out, mut regions) = (Vec::new(), Vec::new());
    let mut at = 0;
    let copy = |out: &mut Vec<u8>, regions: &mut Vec<_>, from: usize, len, region| {
        if region {
            regions.push(out.len() as u64..(out.len() + len) as u64);
        }
        out.extend_from_slice(&stream[from..from + len]);
    };
    while at < stream.len() {
        let magic = u32::from_le_bytes(stream[at..at + 4].try_into().unwrap());
        if magic & !0x0F == 0x184D_2A50 {
            let len = 8 + u32::from_le_bytes(stream[at + 4..at + 8].try_into().unwrap());
            copy(&mut out, &mut regions, at, 8, true);
            copy(&mut out, &mut regions, at + 8, len as usize - 8, false);
            at += len as usize;
            continue;
        }
        let descriptor = stream[at + 4];
        let single = descriptor & 0x20 != 0;
        let header = 5
            + usize::from(!single)
            + [0, 1, 2, 4][usize::from(descriptor & 3)]
            + [usize::from(single), 2, 4, 8][usize::from(descriptor >> 6)];
        let start = out.len();
        copy(&mut out, &mut regions, at, header, true);
        if !checked {
            out[start + 4] &= !0x04;
        }
        at += header;
        loop {
            let block = u32::from_le_bytes([stream[at], stream[at + 1], stream[at + 2], 0]);
            let size = if (block >> 1) & 3 == 1 {
                1
            } else {
                block as usize >> 3
            };
            copy(&mut out, &mut regions, at, 3, true);
            let head = if (block >> 1) & 3 == 2 {
                size.min(16)
            } else {
                0
            };
            copy(&mut out, &mut regions, at + 3, head, true);
            copy(&mut out, &mut regions, at + 3 + head, size - head, false);
            at += 3 + size;
            if block & 1 != 0 {
                break;
            }
        }
        if descriptor & 0x04 != 0 {
            copy(&mut out, &mut regions, at, 4, checked);
            if !checked {
                out.truncate(out.len() - 4);
            }
            at += 4;
        }
    }
    Seed::new(out, regions)
}

/// zstd's own streams, as they are and without checksums, so that what
/// mutations decode to, not just whether, is compared; and blocks of
/// each kind built by hand.
#[test]
fn decodes_mutations_as_ruzstd_does() {
    let built = [
        skippable(b"skipped"),
        zstd(&[Block::Raw(b"raw"), Block::Rle(b'r', 300), Block::Raw(b"")]),
        zstd(&[Block::Rle(0, 0)]),
    ]
    .concat();
    let mut corpus = vec![seed(&built, true)];
    for stream in [
        &include_bytes!("data/numbers.zst")[..],
        include_bytes!("data/raw.zst"),
        include_bytes!("data/literals.zst"),
        include_bytes!("data/sequences.zst"),
    ] {
        corpus.extend([seed(stream, true), seed(stream, false)]);
    }
    run(0x2575_5EED, &corpus, MUTATIONS);
}

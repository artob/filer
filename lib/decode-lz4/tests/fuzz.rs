// This is free and unencumbered software released into the public domain.

//! A structure-aware fuzzer of the decoder: it mutates frames and blocks,
//! where their headers and sizes are more often, with values chosen to hit
//! boundaries, and checks that each frame decodes the same in pieces of a
//! random size as whole, and as `lz4_flex` decodes it, and each block as
//! `lz4_flex` decodes it: to the same bytes, or not at all.
//!
//! Each `cargo test` runs a few hundred inputs. For longer runs, set
//! `DECODE_LZ4_FUZZ_ITERATIONS`; `DECODE_LZ4_FUZZ_SEED` picks another
//! sequence, and a failure prints the seed and iteration that reproduce
//! it. `fuzz/` has a coverage-guided `cargo-fuzz` target over the same
//! checks.

mod common;

use common::{agree, frame, lz4_flex, lz4_flex_block, numbers};
use filer_spec::{
    Image, Outcome,
    fuzz::{Fuzz, Mutations},
};
use std::{io::Write, sync::Arc};

/// Values at the edges of what LZ4 frames and blocks hold: tokens, flags,
/// block sizes, and the magic numbers' bytes.
const BYTES: [u8; 14] = [
    0, 1, 4, 0x0F, 0x10, 0x40, 0x60, 0x70, 0x80, 0xF0, 0x18, 0x22, 0x4D, 0xFF,
];
const WORDS: [u32; 8] = [
    0,
    1,
    0xFF,
    0xFFFF,
    0x1_0000,
    0x8000_0000,
    0x184D_2204,
    u32::MAX,
];

/// The checksums catch most mutations of frames, so few at once.
const MUTATIONS: Mutations = Mutations::new(&BYTES, &WORDS).aligned(false).most(2);

/// Returns the frame of `data` that `lz4_flex` writes.
fn flex_frame(data: &[u8]) -> Vec<u8> {
    let mut encoder = lz4_flex::frame::FrameEncoder::new(Vec::new());
    encoder.write_all(data).unwrap();
    encoder.finish().unwrap()
}

#[test]
fn decodes_mutations_as_lz4_flex_does() {
    let data = numbers().repeat(2);
    let block = lz4_flex::block::compress(&data[..20_000]);
    let corpus = [
        include_bytes!("data/linked.lz4")[..4000].to_vec(),
        frame(0x00, &[(false, &block)], b""),
        frame(0x34, &[(false, &block), (true, b"stored")], &[]),
        flex_frame(&data),
    ]
    .map(|stream| {
        let len = stream.len() as u64;
        (Arc::new(stream), vec![0..len.min(24), 0..len])
    });
    let blocks = [block.clone(), lz4_flex::block::compress(&[7; 300])];
    Fuzz::new("DECODE_LZ4_FUZZ", 0x4C5A_5EED)
        .iterations(400)
        .run(
            corpus.len() + blocks.len(),
            |source, rng| {
                let (seed, regions) = match corpus.get(source) {
                    Some((seed, regions)) => (seed.clone(), regions.clone()),
                    None => {
                        let block = &blocks[source - corpus.len()];
                        (
                            Arc::new(block.clone()),
                            std::iter::once(0..block.len() as u64).collect(),
                        )
                    },
                };
                let input = MUTATIONS.mutate(&seed, &regions, rng);
                let mut stream = vec![0; input.length() as usize];
                input.copy(0, &mut stream);
                (source >= corpus.len(), stream, 1 + rng.below(64) as usize)
            },
            |(is_block, stream, piece)| {
                let (ours, theirs) = if is_block {
                    let mut output = vec![0; 30_000];
                    let ours = decode_lz4::decode_block(&stream, &mut output)
                        .ok()
                        .map(|len| output[..len].to_vec());
                    (ours, lz4_flex_block(&stream, 30_000))
                } else {
                    (agree(&stream, piece).ok(), lz4_flex(&stream))
                };
                // `lz4_flex` takes a stream cut short between blocks to end
                // there, as `lz4` does not: this decoder decodes as much,
                // short of finishing. It takes four bytes alone, whatever
                // they are, to hold nothing; `lz4` refuses four that begin
                // no frame, as this decoder does.
                let cut = || common::unfinished(&stream, stream.len().max(1)).ok();
                let four = stream.len() == 4 && theirs.as_ref().is_some_and(Vec::is_empty);
                assert!(
                    ours == theirs
                        || (!is_block
                            && ours.is_none()
                            && (four || cut().map(|(data, _)| data) == theirs)),
                    "ours: {:?}; lz4_flex's: {:?}",
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

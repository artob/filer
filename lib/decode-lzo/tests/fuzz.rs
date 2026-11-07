// This is free and unencumbered software released into the public domain.

//! A structure-aware fuzzer of the decoder: it mutates lzop files and
//! LZO1X blocks, where their headers and sizes are more often, with values
//! chosen to hit boundaries, and checks that each file decodes the same in
//! pieces of a random size as whole, and each block as `lzokay-native`
//! decodes it: to the same bytes, or not at all.
//!
//! Each `cargo test` runs a few hundred inputs. For longer runs, set
//! `DECODE_LZO_FUZZ_ITERATIONS`; `DECODE_LZO_FUZZ_SEED` picks another
//! sequence, and a failure prints the seed and iteration that reproduce
//! it. `fuzz/` has coverage-guided `cargo-fuzz` targets over the same
//! checks.

mod common;

use common::{
    ADLER32_C, ADLER32_D, CRC32_C, CRC32_D, Header, agree, file, lzokay, names_a_version, numbers,
};
use filer_spec::{
    Image, Outcome,
    fuzz::{Fuzz, Mutations},
};
use std::sync::Arc;

/// Values at the edges of what lzop files and LZO1X blocks hold:
/// instructions, lengths, flags, and the magic number's bytes.
const BYTES: [u8; 14] = [
    0, 1, 3, 0x0F, 0x10, 0x11, 0x12, 0x1F, 0x20, 0x40, 0x89, 0x90, 0xE0, 0xFF,
];
const WORDS: [u32; 8] = [
    0,
    1,
    0xFF,
    0xFFFF,
    0x4_0000,
    0x0400_0000,
    0x0400_0001,
    u32::MAX,
];

/// The checksums catch most mutations of files, so few at once.
const MUTATIONS: Mutations = Mutations::new(&BYTES, &WORDS).aligned(false).most(2);

#[test]
fn decodes_mutations_as_lzokay_does() {
    let data = numbers().repeat(2);
    let checked = Header {
        flags: ADLER32_D | ADLER32_C | CRC32_D | CRC32_C,
        extra: Some(b"extra".to_vec()),
        ..Header::default()
    };
    let unchecked = Header {
        flags: 0,
        ..Header::default()
    };
    let corpus = [
        include_bytes!("data/fast.lzo")[..4000].to_vec(),
        file(&checked, &[(true, &data[..20_000]), (false, b"stored")]),
        file(&unchecked, &[(true, &data[..5000]), (true, &data[..3000])]),
    ]
    .map(|stream| {
        let len = stream.len() as u64;
        (Arc::new(stream), vec![0..len.min(64), 0..len])
    });
    let blocks = [
        lzokay_native::compress(&data[..20_000]).unwrap(),
        lzokay_native::compress(&[7; 300]).unwrap(),
    ];
    Fuzz::new("DECODE_LZO_FUZZ", 0x4C5A_0F15)
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
                let ours = if is_block {
                    let mut output = vec![0; 30_000];
                    let ours = decode_lzo::decode_block(&stream, &mut output)
                        .ok()
                        .map(|len| output[..len].to_vec());
                    let theirs = lzokay(&stream, 30_000);
                    assert!(
                        ours == theirs || names_a_version(&stream),
                        "ours: {:?}; lzokay-native's: {:?}",
                        ours.as_ref().map(Vec::len),
                        theirs.as_ref().map(Vec::len),
                    );
                    ours
                } else {
                    agree(&stream, piece).ok()
                };
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

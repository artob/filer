// This is free and unencumbered software released into the public domain.

//! A dependency-free, structure-aware fuzzer of the decoders: it mutates
//! streams where their headers and trailers are, with values chosen to hit
//! boundaries, and checks that each decodes the same in pieces of a random
//! size as whole, failing only as `Corrupt`, or `Unsupported` for what a
//! decoder declines.
//!
//! Each `cargo test` runs a few hundred inputs per decoder. For longer
//! runs, set `FILER_PACK_FUZZ_ITERATIONS`; `FILER_PACK_FUZZ_SEED` picks
//! another sequence, and a failure prints the seed and iteration that
//! reproduce it. `fuzz/` has a coverage-guided `cargo-fuzz` target per
//! decoder over the same check.

#![cfg(any(feature = "bzip2", feature = "gzip", feature = "xz", feature = "zstd"))]

mod common;

use common::agree;
use filer_api::ErrorKind;
use filer_pack::Decoder;
use filer_spec::{
    Image, Outcome,
    fuzz::{Fuzz, Mutations},
};
use std::{ops::Range, sync::Arc};

/// A stream to mutate, and where its headers and trailers are.
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
/// that each decodes alike with decoders from `new` in pieces and whole,
/// failing only as one of `kinds`.
fn run<D: Decoder>(
    seed: u64,
    corpus: &[Seed],
    mutations: Mutations,
    new: fn() -> D,
    kinds: &[ErrorKind],
) {
    Fuzz::new("FILER_PACK_FUZZ", seed).iterations(400).run(
        corpus.len(),
        |source, rng| {
            let seed = &corpus[source];
            let input = mutations.mutate(&seed.stream, &seed.regions, rng);
            let mut bytes = vec![0; input.length() as usize];
            input.copy(0, &mut bytes);
            (bytes, 1 + rng.below(64) as usize)
        },
        |(stream, piece)| match agree(new, &stream, piece) {
            Ok(data) => Outcome {
                opened: true,
                entries: 1,
                bytes: data.len() as u64,
                ..Outcome::default()
            },
            Err(error) => {
                assert!(kinds.contains(&error.kind()), "{error:?}");
                Outcome {
                    errors: 1,
                    ..Outcome::default()
                }
            },
        },
    );
}

#[cfg(feature = "gzip")]
mod gzip {
    use super::*;
    use common::Member;
    use filer_pack::Gzip;

    /// Values at the edges of what gzip headers and trailers hold: the
    /// magic number's bytes, the method, and each flag.
    const BYTES: [u8; 12] = [0, 1, 7, 8, 0x02, 0x04, 0x08, 0x10, 0x1F, 0x8B, 0xE0, 0xFF];
    const WORDS: [u32; 7] = [0, 1, 0xFF, 0xFFFF, 0x8B1F, 0x7FFF_FFFF, u32::MAX];

    /// Fields lie anywhere after a name; the checksums catch most
    /// mutations, so few at once.
    const MUTATIONS: Mutations = Mutations::new(&BYTES, &WORDS).aligned(false).most(2);

    /// Returns the stream of `members`, each with its data, and `garbage`
    /// after.
    fn seed(members: &[(Member<'_>, &[u8])], garbage: &[u8]) -> Seed {
        let mut stream = Vec::new();
        let mut regions = Vec::new();
        for (member, data) in members {
            let at = stream.len() as u64;
            stream.extend(member.gzip(data));
            regions.push(at..at + member.header().len() as u64);
            regions.push(stream.len() as u64 - 8..stream.len() as u64);
        }
        stream.extend_from_slice(garbage);
        Seed::new(stream, regions)
    }

    #[test]
    fn survives_mutated_gzip_streams() {
        let full = Member {
            extra: Some(b"AB\x02\x00hi"),
            name: Some(b"name.txt"),
            comment: Some(b"comment"),
            header_crc: true,
        };
        let numbers = include_bytes!("data/numbers.gz").to_vec();
        let len = numbers.len() as u64;
        let corpus = [
            Seed::new(numbers, vec![0..32, len - 30..len]),
            seed(&[(full, b"some data, some data")], b""),
            seed(
                &[
                    (Member::default(), b"one"),
                    (full, b""),
                    (Member::default(), b"two"),
                ],
                b"garbage",
            ),
        ];
        run(
            0x6219_5EED,
            &corpus,
            MUTATIONS,
            Gzip::new,
            &[ErrorKind::Corrupt],
        );
    }
}

#[cfg(feature = "zstd")]
mod zstd {
    use super::*;
    use common::{Block, skippable, zstd};
    use filer_pack::Zstd;

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
    fn survives_mutated_zstd_streams() {
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
        let kinds = [ErrorKind::Corrupt, ErrorKind::Unsupported];
        run(0x2575_5EED, &corpus, MUTATIONS, Zstd::new, &kinds);
    }
}

#[cfg(feature = "xz")]
mod xz {
    use super::*;
    use filer_pack::{Lzma, Lzma2, Xz};

    /// Values at the edges of what xz headers, indexes, and LZMA2 chunks
    /// hold: the magic numbers' bytes, check types, filter ids, and chunk
    /// controls.
    const BYTES: [u8; 14] = [
        0, 1, 2, 4, 0x0A, 0x21, 0x37, 0x59, 0x5A, 0x7A, 0x80, 0xC0, 0xE0, 0xFF,
    ];
    const WORDS: [u32; 7] = [0, 1, 0xFFFF, 0x0101, 0x7FFF_FFFF, 0xFFFF_FFFE, u32::MAX];

    /// Fields are packed; the checks catch most mutations, so few at once.
    const MUTATIONS: Mutations = Mutations::new(&BYTES, &WORDS).aligned(false).most(2);

    /// Returns `stream`, with where its stream headers and footers are, as
    /// far as the first and last 64 bytes go, and where it holds `text`.
    fn seed(stream: &[u8], text: &[u8]) -> Seed {
        let len = stream.len() as u64;
        let mut regions = vec![0..len.min(64), len.saturating_sub(64)..len];
        let found = stream
            .windows(text.len().max(1))
            .position(|bytes| bytes == text);
        if let Some(at) = found.filter(|_| !text.is_empty()) {
            regions.push(at as u64..(at + text.len()) as u64);
        }
        Seed::new(stream.to_vec(), regions)
    }

    #[test]
    fn survives_mutated_xz_streams() {
        // Stored text, which may change and still decode where no check
        // covers it, as in the stream with none.
        let corpus = [
            seed(include_bytes!("data/numbers.xz"), b"end"),
            seed(include_bytes!("data/checks.xz"), b"none\n"),
        ];
        let kinds = [ErrorKind::Corrupt, ErrorKind::Unsupported];
        run(0x0C5_5EED, &corpus, MUTATIONS, Xz::new, &kinds);
    }

    #[test]
    fn survives_mutated_raw_streams() {
        let lzma = include_bytes!("data/numbers.lzma");
        let corpus = [seed(&lzma[13..], b"")];
        // As `numbers.lzma`'s header has it: `-6`, with 8 MiB.
        let new = || Lzma::new([0x5D, 0, 0, 0x80, 0], None).unwrap();
        run(0x1A3A_5EED, &corpus, MUTATIONS, new, &[ErrorKind::Corrupt]);
        let corpus = [seed(include_bytes!("data/numbers.lzma2"), b"")];
        let new = || Lzma2::new(22).unwrap();
        run(0x1A3B_5EED, &corpus, MUTATIONS, new, &[ErrorKind::Corrupt]);
    }
}

#[cfg(feature = "bzip2")]
mod bzip2 {
    use super::*;
    use filer_pack::Bzip2;
    use filer_spec::fuzz::Order;

    /// Values at the edges of what bzip2 headers hold: the magic numbers'
    /// bytes, block sizes, and table and selector counts.
    const BYTES: [u8; 12] = [0, 1, 2, 6, 7, 0x17, 0x31, 0x80, b'1', b'9', b'h', 0xFF];
    const WORDS: [u32; 6] = [0, 1, 0xFFFF, 0x3141_5926, 0x1772_4538, u32::MAX];

    /// Fields are of bits, most significant first; the CRCs catch most
    /// mutations, so few at once.
    const MUTATIONS: Mutations = Mutations::new(&BYTES, &WORDS)
        .order(Order::Big)
        .aligned(false)
        .most(2);

    /// Returns `stream`, with where its first block's header and tables
    /// are, as far as its first 64 bytes go, and its end.
    fn seed(stream: &[u8]) -> Seed {
        let len = stream.len() as u64;
        Seed::new(
            stream.to_vec(),
            vec![0..len.min(64), len.saturating_sub(16)..len],
        )
    }

    #[test]
    fn survives_mutated_bzip2_streams() {
        // Only a small stream: a mutated block decodes whole, in pieces,
        // before its CRC fails it.
        let corpus = [seed(include_bytes!("data/numbers.bz2"))];
        let kinds = [ErrorKind::Corrupt, ErrorKind::Unsupported];
        run(0xB212_5EED, &corpus, MUTATIONS, Bzip2::new, &kinds);
    }
}

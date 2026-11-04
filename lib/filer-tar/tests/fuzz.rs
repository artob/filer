// This is free and unencumbered software released into the public domain.

//! A dependency-free, structure-aware fuzzer: it mutates archives where
//! their headers and extended headers' records are, with values chosen to
//! hit boundaries, and walks each result with `filer_spec::exercise`. It
//! mostly reseals the checksum of a header it mutated, so that the
//! mutation gets past it to what parses the header.
//!
//! Each `cargo test` runs a few hundred inputs. For longer runs, set
//! `FILER_TAR_FUZZ_ITERATIONS`; `FILER_TAR_FUZZ_SEED` picks another
//! sequence, and a failure prints the seed and iteration that reproduce
//! it. `fuzz/` has a coverage-guided `cargo-fuzz` target over the same
//! walk.

mod common;

use common::{Writer, seal};
use filer_spec::{
    Checks, Image, Overlay, block_on, exercise,
    fuzz::{self, Fuzz, Mutations},
};
use filer_tar::Tar;
use std::{ops::Range, sync::Arc};

/// Values at the edges of what tar headers hold: octal digits and what
/// ends them, base-256's marker, and the type flags the driver tells apart.
const BYTES: [u8; 16] = [
    0, b' ', b'/', b'0', b'7', b'8', 0x80, 0xFF, b'1', b'2', b'5', b'g', b'x', b'K', b'L', b'S',
];
const WORDS: [u32; 8] = [
    0,
    u32::from_le_bytes(*b"0000"),
    u32::from_le_bytes(*b"7777"),
    u32::from_le_bytes(*b"9999"),
    u32::from_le_bytes(*b"../\0"),
    u32::from_le_bytes(*b"=\n0 "),
    u32::from_le_bytes([0x80, 0, 0, 0xFF]),
    u32::MAX,
];

/// Header fields are text, wherever in the header they are.
const MUTATIONS: Mutations = Mutations::new(&BYTES, &WORDS).aligned(false);

/// A region of an archive to mutate, and whether it is a header, whose
/// checksum to reseal.
struct Region {
    range: Range<u64>,
    header: bool,
}

impl fuzz::Region for Region {
    fn range(&self) -> Range<u64> {
        self.range.clone()
    }
}

/// An archive to mutate, and where its headers are.
struct Seed {
    archive: Arc<Vec<u8>>,
    regions: Vec<Region>,
}

impl Seed {
    /// Finds the headers by walking the archive, as the driver does, and
    /// the records of extended headers after them.
    fn new(archive: Vec<u8>) -> Self {
        let mut regions = Vec::new();
        let mut at = 0;
        while at + 512 <= archive.len() && archive[at..at + 512].iter().any(|&b| b != 0) {
            let header = &archive[at..at + 512];
            let size = std::str::from_utf8(&header[124..136]).unwrap();
            let size = usize::from_str_radix(size.trim_matches(['\0', ' ']), 8).unwrap();
            regions.push(Region {
                range: at as u64..at as u64 + 512,
                header: true,
            });
            let data = at + 512;
            if matches!(header[156], b'x' | b'g' | b'L' | b'K') && size > 0 {
                regions.push(Region {
                    range: data as u64..(data + size) as u64,
                    header: false,
                });
            }
            at = data + size.div_ceil(512) * 512;
        }
        regions.push(Region {
            range: 0..archive.len() as u64,
            header: false,
        });
        Self {
            archive: Arc::new(archive),
            regions,
        }
    }
}

/// Stores the checksum of the header at `start`, as written.
fn reseal(input: &mut Overlay<Vec<u8>>, start: u64) {
    let mut header = [0; 512];
    input.copy(start, &mut header);
    seal(&mut header);
    input.patch(start, &header);
}

fn corpus() -> Vec<Seed> {
    let mut seeds = vec![include_bytes!("data/hello.tar").to_vec()];
    seeds.push(
        Writer::default()
            .dir(b"dir/")
            .file(b"dir/file", b"contents")
            .symlink(b"link", b"dir/file")
            .member(b"hard", b'1', b"dir/file", b"")
            .member(b"fifo", b'6', b"", b"")
            .member(b"char", b'3', b"", b"")
            .finish(),
    );
    // Extended headers: PAX, global PAX, and GNU long names.
    seeds.push(
        Writer::default()
            .pax(&[("path", b"long/name"), ("mtime", b"1.5"), ("size", b"4")])
            .file(b"ignored", b"data")
            .pax(&[("linkpath", b"long/name")])
            .symlink(b"pax-link", b"")
            .member(b"pax_global_header", b'g', b"", b"6 a=b\n")
            .member(b"././@LongLink", b'L', b"", b"gnu/long/name\0")
            .file(b"ignored", b"gnu")
            .member(b"././@LongLink", b'K', b"", b"gnu/long/name\0")
            .symlink(b"gnu-link", b"")
            .finish(),
    );
    seeds.into_iter().map(Seed::new).collect()
}

#[test]
fn survives_mutated_archives() {
    let corpus = corpus();
    Fuzz::new("FILER_TAR_FUZZ", 0x7A2_5EED).iterations(400).run(
        corpus.len(),
        |source, rng| {
            let seed = &corpus[source];
            let input = MUTATIONS.mutate_then(
                &seed.archive,
                &seed.regions,
                rng,
                |input, region, _, rng| {
                    // Mostly past the checksum.
                    if region.header && rng.below(8) != 0 {
                        reseal(input, region.range.start);
                    }
                },
            );
            let mut bytes = vec![0; input.length() as usize];
            input.copy(0, &mut bytes);
            bytes
        },
        |bytes| {
            let checks = Checks::new().kinds(true).xattrs(false).refusals(true);
            exercise(block_on(Tar::open(bytes)), checks)
        },
    );
}

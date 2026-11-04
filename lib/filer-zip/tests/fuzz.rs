// This is free and unencumbered software released into the public domain.

//! A dependency-free, structure-aware fuzzer: it mutates archives where
//! their records are, with values chosen to hit boundaries, and walks each
//! result with `filer_spec::exercise`, through `common::exercise`, which
//! also checks that the zip bomb limits hold. A second test writes archives from hostile name
//! fragments and checks that exactly the unsafe ones are refused.
//!
//! Each `cargo test` runs a few hundred inputs. For longer runs, set
//! `FILER_ZIP_FUZZ_ITERATIONS`; `FILER_ZIP_FUZZ_SEED` picks another
//! sequence, and a failure prints the seed and iteration that reproduce
//! it. `fuzz/` has coverage-guided `cargo-fuzz` targets over the same walk.

mod common;

use common::{Entry, FILE, Writer, exercise};
use filer_api::ErrorKind;
use filer_spec::{
    Image, block_on,
    fuzz::{Fuzz, Mutations},
};
use filer_zip::Zip;
use std::{ops::Range, sync::Arc};

/// An archive to mutate, and where its records are.
struct Seed {
    archive: Arc<Vec<u8>>,
    regions: Vec<Range<u64>>,
}

impl Seed {
    fn new(archive: Vec<u8>) -> Self {
        // Each record's fixed part, found by signature.
        let mut regions = Vec::new();
        for at in 0..archive.len().saturating_sub(4) {
            let len = match &archive[at..at + 4] {
                b"PK\x03\x04" => 30,
                b"PK\x01\x02" => 46,
                b"PK\x05\x06" => 22,
                b"PK\x06\x06" => 56,
                b"PK\x06\x07" => 20,
                _ => continue,
            };
            regions.push(at as u64..(at + len).min(archive.len()) as u64);
        }
        // And anywhere, member data included, now and then.
        regions.push(0..archive.len() as u64);
        Self {
            archive: Arc::new(archive),
            regions,
        }
    }
}

/// Values at the edges of what ZIP records hold, whose fields are packed
/// without alignment; records cross-check, so few mutations at once.
const BYTES: [u8; 10] = [0, 1, 8, 0x2F, 0x5C, 0x7F, 0x80, 0xA1, 0xFE, 0xFF];
const WORDS: [u32; 12] = [
    0,
    1,
    8,
    22,
    46,
    0x0FFF,
    0xFFFF,
    0x1_0000,
    0x10_0000,
    0x7FFF_FFFF,
    0xFFFF_FFFE,
    0xFFFF_FFFF,
];

const MUTATIONS: Mutations = Mutations::new(&BYTES, &WORDS).aligned(false).most(2);

/// Archives of every kind the driver reads.
fn corpus() -> Vec<Seed> {
    let mut seeds = vec![
        include_bytes!("data/hello.zip").to_vec(),
        include_bytes!("data/methods.zip").to_vec(),
    ];
    for (zip64, deflate) in [(false, false), (false, true), (true, true)] {
        let mut writer = Writer {
            zip64,
            deflate,
            ..Writer::default()
        };
        writer
            .dir(b"dir/")
            .file(b"dir/text", b"Some text, some text, some text.\n")
            .symlink(b"link", b"dir/text")
            .symlink(b"up", b"../../outside")
            .fifo(b"fifo")
            .file(b"implied/\xFFforeign", b"foreign")
            .file(b"empty", b"");
        writer
            .file(b"described", b"described")
            .last()
            .data_descriptor = true;
        seeds.push(writer.finish());
    }
    // A member at the edge of the ratio limit's exemption.
    let mut zeros = Entry::new(b"zeros", Some(FILE), &[0; 1 << 20]);
    zeros.method = 8;
    seeds.push(Writer::default().entry(zeros).finish());
    seeds.into_iter().map(Seed::new).collect()
}

#[test]
fn survives_mutated_archives() {
    let corpus = corpus();
    Fuzz::new("FILER_ZIP_FUZZ", 0x21B_5EED).iterations(300).run(
        corpus.len(),
        |source, rng| {
            let seed = &corpus[source];
            let input = MUTATIONS.mutate(&seed.archive, &seed.regions, rng);
            let mut bytes = vec![0; input.length() as usize];
            input.copy(0, &mut bytes);
            bytes
        },
        exercise,
    );
}

/// Fragments that make up hostile names, and whether each alone makes a
/// name unsafe wherever it appears.
const FRAGMENTS: [(&[u8], bool); 12] = [
    (b"a", false),
    (b"b.txt", false),
    (b"/", false),
    (b".", false),
    (b"..", false),
    (b"...", false),
    (b"\\", true),
    (b"\0", true),
    (b"C:", false),
    (b"\xC3\xA9", false),
    (b"\xFF", false),
    (b" ", false),
];

/// Returns whether the driver must refuse `name`, for a directory if
/// `dir`; a directory naming the root itself is harmless.
fn unsafe_name(name: &[u8], dir: bool) -> bool {
    let components: Vec<&[u8]> = name.split(|&b| b == b'/').collect();
    let real = components.iter().any(|c| !matches!(*c, b"" | b"."));
    name.starts_with(b"/")
        || name.contains(&b'\\')
        || name.contains(&0)
        || components.contains(&&b".."[..])
        || matches!(name, [letter, b':', ..] if letter.is_ascii_alphabetic())
        || (!real && !dir)
}

#[test]
fn refuses_exactly_the_unsafe_names() {
    Fuzz::new("FILER_ZIP_FUZZ", 0x21B_5EED).iterations(300).run(
        1,
        |_, rng| {
            let mut name = Vec::new();
            for _ in 0..1 + rng.below(6) {
                let (fragment, _) = rng.pick(&FRAGMENTS);
                name.extend_from_slice(fragment);
            }
            let mut writer = Writer::default();
            let dir = match rng.below(3) {
                0 => writer.file(&name, b"x").last().name.ends_with(b"/"),
                1 => writer.symlink(&name, b"../x").last().name.ends_with(b"/"),
                _ => {
                    writer.dir(&name);
                    true
                },
            };
            (name, dir, writer.finish())
        },
        |(name, dir, archive)| {
            // A trailing `/` makes any member a directory.
            let refuse = unsafe_name(&name, dir);
            match block_on(Zip::open(archive.clone())) {
                Ok(_) => assert!(!refuse, "opened {name:?}"),
                Err(error) => {
                    assert!(refuse, "refused {name:?}: {error:?}");
                    assert_eq!(error.kind(), ErrorKind::InvalidName, "{name:?}");
                },
            }
            exercise(archive)
        },
    );
}

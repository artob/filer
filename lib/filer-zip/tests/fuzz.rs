// This is free and unencumbered software released into the public domain.

//! A dependency-free, structure-aware fuzzer: it mutates archives where
//! their records are, with values chosen to hit boundaries, and walks each
//! result with `common::exercise`, which checks every answer and that the
//! zip bomb limits hold. A second test writes archives from hostile name
//! fragments and checks that exactly the unsafe ones are refused.
//!
//! Each `cargo test` runs a few hundred inputs. For longer runs, set
//! `FILER_ZIP_FUZZ_ITERATIONS`; `FILER_ZIP_FUZZ_SEED` picks another
//! sequence, and a failure prints the seed and iteration that reproduce
//! it. `fuzz/` has coverage-guided `cargo-fuzz` targets over the same walk.

mod common;

use common::{Entry, FILE, Writer, exercise::exercise};
use filer_api::ErrorKind;
use filer_spec::block_on;
use filer_zip::Zip;
use std::{ops::Range, panic};

/// SplitMix64: small, fast, and good enough to drive mutations.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }

    fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        items[self.below(items.len() as u64) as usize]
    }
}

/// An archive to mutate, and where its records are.
struct Seed {
    archive: Vec<u8>,
    regions: Vec<Range<usize>>,
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
            regions.push(at..(at + len).min(archive.len()));
        }
        // And anywhere, now and then.
        regions.push(0..archive.len());
        Self { archive, regions }
    }
}

/// Values at the edges of what ZIP records hold.
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

fn mutate(seed: &Seed, rng: &mut Rng) -> Vec<u8> {
    let mut input = seed.archive.clone();
    for _ in 0..1 + rng.below(2) {
        if input.is_empty() {
            break;
        }
        let region = seed.regions[rng.below(seed.regions.len() as u64) as usize].clone();
        let end = region.end.min(input.len());
        let start = region.start.min(end);
        let at = start + rng.below((end - start) as u64) as usize;
        let mut patch = |at: usize, bytes: &[u8]| {
            for (i, &byte) in bytes.iter().enumerate() {
                if let Some(slot) = input.get_mut(at + i) {
                    *slot = byte;
                }
            }
        };
        match rng.below(15) {
            0..=3 => {
                let bit = 1 << rng.below(8);
                patch(at, &[seed.archive.get(at).copied().unwrap_or(0) ^ bit]);
            },
            4..=5 => patch(at, &[rng.pick(&BYTES)]),
            6..=8 => patch(at, &(rng.pick(&WORDS) as u16).to_le_bytes()),
            9..=11 => patch(at, &rng.pick(&WORDS).to_le_bytes()),
            12 => {
                let bytes: Vec<u8> = (0..8).map(|_| rng.next() as u8).collect();
                patch(at, &bytes);
            },
            13 => input.truncate(rng.below(input.len() as u64) as usize),
            // Leave the headers be, and only corrupt member data.
            _ => {
                let at = rng.below(input.len() as u64) as usize;
                if !seed.regions.iter().any(|r| r.contains(&at) && r.len() < 64) {
                    input[at] ^= 1 << rng.below(8);
                }
            },
        }
    }
    input
}

/// Archives of every kind the driver reads.
fn corpus() -> Vec<Seed> {
    let mut seeds = vec![include_bytes!("data/hello.zip").to_vec()];
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

fn env(name: &str, default: u64) -> u64 {
    std::env::var(name).map_or(default, |value| value.parse().expect(name))
}

#[test]
fn survives_mutated_archives() {
    let iterations = env("FILER_ZIP_FUZZ_ITERATIONS", 300);
    let seed = env("FILER_ZIP_FUZZ_SEED", 0x21B_5EED);
    let corpus = corpus();
    let (mut opened, mut entries, mut bytes, mut errors) = (0, 0, 0, 0);
    for iteration in 0..iterations {
        // Each input has its own generator, so that one reproduces alone.
        let mut rng = Rng(seed ^ iteration.wrapping_mul(0x2545_F491_4F6C_DD1D));
        let source = rng.below(corpus.len() as u64) as usize;
        let input = mutate(&corpus[source], &mut rng);
        match panic::catch_unwind(|| exercise(input)) {
            Ok(outcome) => {
                opened += usize::from(outcome.opened);
                entries += outcome.entries;
                bytes += outcome.bytes;
                errors += outcome.errors;
            },
            Err(panic) => {
                eprintln!(
                    "failed: FILER_ZIP_FUZZ_SEED={seed} at iteration {iteration} \
                     (corpus archive {source})"
                );
                panic::resume_unwind(panic);
            },
        }
    }
    // Shown with `--nocapture`: how deep the inputs got.
    println!(
        "{iterations} inputs: {opened} opened, {entries} entries listed, \
         {bytes} bytes read, {errors} errors"
    );
    assert!(opened > 0 && entries > 0 && bytes > 0, "inputs too shallow");
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
    let iterations = env("FILER_ZIP_FUZZ_ITERATIONS", 300);
    let seed = env("FILER_ZIP_FUZZ_SEED", 0x21B_5EED);
    for iteration in 0..iterations {
        let mut rng = Rng(seed ^ iteration.wrapping_mul(0x2545_F491_4F6C_DD1D));
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
        let archive = writer.finish();
        // A trailing `/` makes any member a directory.
        let refuse = unsafe_name(&name, dir);
        let result = block_on(Zip::open(archive.clone()));
        let what = format!("{name:?} (FILER_ZIP_FUZZ_SEED={seed}, iteration {iteration})");
        match result {
            Ok(_) => assert!(!refuse, "opened {what}"),
            Err(error) => {
                assert!(refuse, "refused {what}: {error:?}");
                assert_eq!(error.kind(), ErrorKind::InvalidName, "{what}");
            },
        }
        exercise(archive);
    }
}

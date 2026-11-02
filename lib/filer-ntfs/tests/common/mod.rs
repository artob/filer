// This is free and unencumbered software released into the public domain.

//! Shared by the tests: expanding fixtures, and what they hold.

// Each test binary compiles this module and uses only part of it.
#![allow(dead_code)]

use filer_ntfs::{Ntfs, Options};
use std::collections::BTreeMap;

/// A volume image as a base image with bytes patched over it.
pub type Overlay = filer_spec::Overlay<Vec<u8>>;

/// Opens `source` and walks it with [`filer_spec::exercise`], with the default
/// checks, allowing dirty volumes: listed kinds come from the index's copy of a
/// file's attributes and stats from its record, which a corrupt volume may make
/// disagree. Shared by `tests/fuzz.rs` and the `cargo-fuzz` targets in `fuzz/`.
pub fn exercise<S: filer_api::ReadAt + 'static>(source: S) -> filer_spec::Outcome {
    filer_spec::exercise(
        filer_spec::block_on(Ntfs::open_with(source, Options::new().allow_dirty(true))),
        filer_spec::Checks::new(),
    )
}

/// Expands a sparse fixture, as `tests/fixtures/make.sh` stores them.
pub fn unsparse(sparse: &[u8]) -> Vec<u8> {
    filer_spec::Sparse::new(sparse).to_vec()
}
pub mod manifest;

const SECTOR: usize = 512;

/// The modification time `tests/fixtures/make.sh` gives files:
/// 2024-01-02T03:04:06Z.
pub const MTIME: i64 = 1_704_164_646;

/// What a path holds: a directory, or a file's contents.
#[derive(Debug, PartialEq, Eq)]
pub enum Expected {
    Dir,
    File(Vec<u8>),
}

/// The pattern `make.sh` writes.
pub fn pattern(size: usize, seed: usize) -> Vec<u8> {
    (0..size).map(|i| ((i * 7 + seed) % 251) as u8).collect()
}

/// The tree `make.sh`'s `fill` writes, after its deletions.
pub fn expected() -> BTreeMap<String, Expected> {
    let mut tree = BTreeMap::new();
    let mut file = |path: &str, contents: Vec<u8>| {
        tree.insert(path.to_owned(), Expected::File(contents));
    };
    file("README.TXT", pattern(9, 1));
    file("notes.txt", pattern(10, 2));
    file("A long file name with spaces.txt", pattern(100, 3));
    file("Ünïcödé ✓.txt", pattern(7, 4));
    file("emoji 😀.bin", pattern(5, 5));
    file("empty", Vec::new());
    file("big.bin", pattern(100_000, 7));
    file("Sub/nested.bin", pattern(5000, 8));
    file("Sub/Deeper/leaf.txt", pattern(5, 9));
    for i in 0..150 {
        file(&format!("many/f{i:03}"), pattern(i, 100 + i));
    }
    for (name, seed) in [("frag.a", 20), ("frag.b", 40)] {
        file(
            name,
            (0..12).flat_map(|i| pattern(4096, seed + i)).collect(),
        );
    }
    let mut sparse = vec![0; 1_000_000];
    sparse.extend(b"end");
    file("sparse.bin", sparse);
    file("small", pattern(4, 10));
    file("Sub/hardlink", pattern(4, 10));
    file("Case.txt", pattern(3, 11));
    file("case.txt", pattern(4, 12));
    file("Long name for DOS.txt", pattern(6, 13));
    for dir in ["Sub", "Sub/Deeper", "many"] {
        tree.insert(dir.to_owned(), Expected::Dir);
    }
    tree
}

/// Bytes from Xorshift32, as `make.sh` writes them.
pub fn noise(size: usize, seed: u32) -> Vec<u8> {
    let mut x = seed;
    (0..size)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            x as u8
        })
        .collect()
}

/// The tree `make.sh`'s `compressed` writes.
pub fn expected_compressed() -> BTreeMap<String, Expected> {
    let mut tree = BTreeMap::new();
    let mut file = |name: &str, contents: Vec<u8>| {
        tree.insert(format!("Compressed/{name}"), Expected::File(contents));
    };
    file("text.txt", pattern(200_000, 1));
    file("noise.bin", noise(100_000, 7));
    file("exact.bin", pattern(65536, 3));
    let mut mixed = noise(30_000, 9);
    mixed.extend(vec![0; 70_000]);
    mixed.extend(pattern(50_000, 4));
    file("mixed.bin", mixed);
    file("small.txt", pattern(5, 2));
    let mut sparse = vec![0; 200_000];
    sparse.extend(b"end");
    file("sparse.bin", sparse);
    tree.insert("Compressed".to_owned(), Expected::Dir);
    tree
}

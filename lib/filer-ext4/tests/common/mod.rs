// This is free and unencumbered software released into the public domain.

//! Shared by the tests: expanding fixtures, and what they hold.

// Each test binary compiles this module and uses only part of it.
#![allow(dead_code)]

use filer_api::FileKind;
use std::collections::BTreeMap;

pub mod exercise;

const SECTOR: usize = 512;

/// The modification time `tests/fixtures/make.sh` gives files:
/// 2024-01-02T03:04:06.123456789Z.
pub const MTIME: i64 = 1_704_164_646;
pub const MTIME_NANOS: u32 = 123_456_789;

/// Expands a sparse image, as `tests/fixtures/make.sh` stores them.
pub fn unsparse(sparse: &[u8]) -> Vec<u8> {
    let rest = sparse
        .strip_prefix(b"FILER-SPARSE-2\n")
        .expect("sparse magic");
    let (size, mut rest) = rest.split_at(8);
    let mut image = vec![0; u64::from_le_bytes(size.try_into().unwrap()) as usize];
    while !rest.is_empty() {
        let first = u32::from_le_bytes(rest[0..4].try_into().unwrap()) as usize;
        let count = u32::from_le_bytes(rest[4..8].try_into().unwrap()) as usize;
        let sector = &rest[8..8 + SECTOR];
        for i in first..first + count {
            image[i * SECTOR..(i + 1) * SECTOR].copy_from_slice(sector);
        }
        rest = &rest[8 + SECTOR..];
    }
    image
}

/// What a path holds.
#[derive(Debug, PartialEq, Eq)]
pub enum Expected {
    Dir,
    File(Vec<u8>),
    Link(Vec<u8>),
    Special(FileKind),
}

/// The pattern `make.sh` writes.
pub fn pattern(size: usize, seed: usize) -> Vec<u8> {
    (0..size).map(|i| ((i * 7 + seed) % 251) as u8).collect()
}

/// The tree `make.sh`'s `fill` writes, after its deletions, but for the
/// file whose name is not UTF-8.
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
    let mut deep = vec![0; 70_000_003];
    deep[..5].copy_from_slice(b"start");
    deep[300_000..300_006].copy_from_slice(b"middle");
    deep[70_000_000..].copy_from_slice(b"end");
    file("deep.bin", deep);
    file("small", pattern(4, 10));
    file("Sub/hardlink", pattern(4, 10));
    file("Case.txt", pattern(3, 11));
    file("case.txt", pattern(4, 12));
    file(&"n".repeat(255), pattern(6, 13));
    for (link, target) in [
        ("link", "README.TXT".to_owned()),
        ("deeplink", "Sub/Deeper/leaf.txt".to_owned()),
        ("dirlink", "Sub".to_owned()),
        ("abs", "/notes.txt".to_owned()),
        ("dangling", "nope".to_owned()),
        ("long", format!("Sub/{}", "x".repeat(100))),
    ] {
        tree.insert(link.to_owned(), Expected::Link(target.into_bytes()));
    }
    for (name, kind) in [
        ("fifo", FileKind::Fifo),
        ("sock", FileKind::Socket),
        ("chr", FileKind::CharDevice),
        ("blk", FileKind::BlockDevice),
    ] {
        tree.insert(format!("special/{name}"), Expected::Special(kind));
    }
    for dir in ["Sub", "Sub/Deeper", "many", "special", "lost+found"] {
        tree.insert(dir.to_owned(), Expected::Dir);
    }
    tree
}

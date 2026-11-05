// This is free and unencumbered software released into the public domain.

//! Shared by the tests: walking images, what the fixtures hold, and
//! images made here, record by record.

// Each test binary compiles this module and uses only part of it.
#![allow(dead_code)]

use filer_api::FileKind;
use filer_cdfs::Cdfs;
use std::collections::BTreeMap;

/// Opens `source` and walks it with [`filer_spec::exercise`]: listed kinds
/// and stats come from the same records, so they must agree; ISO 9660 has
/// no extended attributes Filer reads. Shared by `tests/fuzz.rs` and the
/// `cargo-fuzz` targets in `fuzz/`.
pub fn exercise<S: filer_api::ReadAt + 'static>(source: S) -> filer_spec::Outcome {
    filer_spec::exercise(
        filer_spec::block_on(Cdfs::open(source)),
        filer_spec::Checks::new().kinds(true).xattrs(false),
    )
}

/// The modification time `tests/fixtures/make.sh` gives everything:
/// 2024-01-02T03:04:06Z.
pub const MTIME: i64 = 1_704_164_646;

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

/// The tree `make.sh`'s `fill` writes, as Rock Ridge records it, but for
/// the file whose name is not UTF-8.
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
    file("big.bin", pattern(20_000, 7));
    file("zeros.bin", vec![0; 20_000]);
    file("Sub/nested.bin", pattern(5000, 8));
    file("Sub/Deeper/leaf.txt", pattern(5, 9));
    for i in 0..30 {
        file(&format!("many/f{i:03}"), pattern(i, 100 + i));
    }
    file(DEEP_FILE, pattern(6, 15));
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
    let mut dirs = vec!["Sub".to_owned(), "Sub/Deeper".to_owned(), "many".to_owned()];
    dirs.push("special".to_owned());
    let mut deep = String::new();
    for letter in "abcdefghi".chars() {
        if !deep.is_empty() {
            deep.push('/');
        }
        deep.push(letter);
        dirs.push(deep.clone());
    }
    for dir in dirs {
        tree.insert(dir, Expected::Dir);
    }
    tree
}

/// The file nine directories deep, more than ISO 9660 allows.
pub const DEEP_FILE: &str = "a/b/c/d/e/f/g/h/i/bottom.txt";

/// The date records made here have: 2024-01-02T03:04:06Z.
pub const DATE: [u8; 7] = [124, 1, 2, 3, 4, 6, 0];

/// The flags of a record.
pub const HIDDEN: u8 = 0x01;
pub const DIRECTORY: u8 = 0x02;
pub const ASSOCIATED: u8 = 0x04;
pub const MULTI_EXTENT: u8 = 0x80;

/// Returns a directory record of `name`, for `len` bytes from logical
/// block `extent`, with `flags` and System Use entries `system_use`.
pub fn record(name: &[u8], extent: u32, len: u32, flags: u8, system_use: &[u8]) -> Vec<u8> {
    let pad = usize::from(name.len().is_multiple_of(2));
    let size = 33 + name.len() + pad + system_use.len();
    let mut bytes = vec![0; size];
    bytes[0] = u8::try_from(size).unwrap();
    bytes[2..6].copy_from_slice(&extent.to_le_bytes());
    bytes[6..10].copy_from_slice(&extent.to_be_bytes());
    bytes[10..14].copy_from_slice(&len.to_le_bytes());
    bytes[14..18].copy_from_slice(&len.to_be_bytes());
    bytes[18..25].copy_from_slice(&DATE);
    bytes[25] = flags;
    bytes[28..30].copy_from_slice(&1_u16.to_le_bytes());
    bytes[30..32].copy_from_slice(&1_u16.to_be_bytes());
    bytes[32] = u8::try_from(name.len()).unwrap();
    bytes[33..33 + name.len()].copy_from_slice(name);
    bytes[33 + name.len() + pad..].copy_from_slice(system_use);
    bytes
}

/// Returns a System Use entry of `signature`, with `data`.
pub fn entry(signature: &[u8; 2], data: &[u8]) -> Vec<u8> {
    let mut entry = vec![
        signature[0],
        signature[1],
        u8::try_from(4 + data.len()).unwrap(),
        1,
    ];
    entry.extend_from_slice(data);
    entry
}

/// The `SP` entry that says Rock Ridge's entries are in use, skipping
/// nothing.
pub fn sharing() -> Vec<u8> {
    entry(b"SP", &[0xBE, 0xEF, 0])
}

/// Where the root directory of an image made here is, in blocks, and the
/// first block free for data.
pub const ROOT: u32 = 18;
pub const DATA: u32 = 20;

/// Returns an image of 2048-byte blocks: the primary volume descriptor at
/// block 16, the terminator at 17, and the root directory at 18, of two
/// sectors, holding `.` (with `root_use` as its System Use entries), `..`,
/// and `records`, one sector after another as each fills; and each
/// `(block, bytes)` of `data`.
pub fn image(root_use: &[u8], records: &[Vec<u8>], data: &[(u32, &[u8])]) -> Vec<u8> {
    let blocks = data
        .iter()
        .map(|(block, bytes)| *block as usize + bytes.len().div_ceil(2048))
        .max()
        .unwrap_or(0)
        .max(DATA as usize);
    let mut image = vec![0; blocks * 2048];
    let descriptor = &mut image[16 * 2048..17 * 2048];
    descriptor[0] = 1;
    descriptor[1..6].copy_from_slice(b"CD001");
    descriptor[6] = 1;
    descriptor[80..84].copy_from_slice(&(blocks as u32).to_le_bytes());
    descriptor[120..122].copy_from_slice(&1_u16.to_le_bytes());
    descriptor[124..126].copy_from_slice(&1_u16.to_le_bytes());
    descriptor[128..130].copy_from_slice(&2048_u16.to_le_bytes());
    descriptor[130..132].copy_from_slice(&2048_u16.to_be_bytes());
    let root = record(&[0], ROOT, 4096, DIRECTORY, &[]);
    descriptor[156..156 + root.len()].copy_from_slice(&root);
    descriptor[881] = 1;
    let terminator = &mut image[17 * 2048..18 * 2048];
    terminator[0] = 255;
    terminator[1..6].copy_from_slice(b"CD001");
    terminator[6] = 1;
    let mut all = vec![
        record(&[0], ROOT, 4096, DIRECTORY, root_use),
        record(&[1], ROOT, 4096, DIRECTORY, &[]),
    ];
    all.extend(records.iter().cloned());
    let (mut sector, mut at) = (ROOT as usize * 2048, 0);
    for record in all {
        if at + record.len() > 2048 {
            sector += 2048;
            at = 0;
        }
        assert!(sector < DATA as usize * 2048, "too many records");
        image[sector + at..sector + at + record.len()].copy_from_slice(&record);
        at += record.len();
    }
    for (block, bytes) in data {
        let at = *block as usize * 2048;
        image[at..at + bytes.len()].copy_from_slice(bytes);
    }
    image
}

/// An image as a base image with bytes patched over it, as the
/// `cargo-fuzz` targets lay their input over a template.
pub type Overlay = filer_spec::Overlay<Vec<u8>>;

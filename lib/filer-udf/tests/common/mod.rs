// This is free and unencumbered software released into the public domain.

//! Shared by the tests: walking volumes, what the fixtures hold, and
//! volumes made here, descriptor by descriptor.

// Each test binary compiles this module and uses only part of it.
#![allow(dead_code)]

use filer_api::FileKind;
use filer_udf::Udf;
use std::collections::BTreeMap;

/// Opens `source` and walks it with [`filer_spec::exercise`]: listed kinds
/// and stats come from the same file entries, so they must agree; UDF's
/// named streams are not read. Shared by `tests/fuzz.rs` and the
/// `cargo-fuzz` targets in `fuzz/`.
pub fn exercise<S: filer_api::ReadAt + 'static>(source: S) -> filer_spec::Outcome {
    filer_spec::exercise(
        filer_spec::block_on(Udf::open(source)),
        filer_spec::Checks::new().kinds(true).xattrs(false),
    )
}

/// Returns the volume a sparse fixture holds.
pub fn image(sparse: &[u8]) -> Vec<u8> {
    filer_spec::Sparse::new(sparse).to_vec()
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

/// The tree `make.sh`'s `fill` writes.
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
    file("Sub/nested.bin", pattern(5000, 8));
    file("Sub/Deeper/leaf.txt", pattern(5, 9));
    for i in 0..30 {
        file(&format!("many/f{i:03}"), pattern(i, 100 + i));
    }
    file("a/b/c/d/e/f/g/h/i/bottom.txt", pattern(6, 15));
    file("small", pattern(4, 10));
    file("Sub/hardlink", pattern(4, 10));
    file("Case.txt", pattern(3, 11));
    file("case.txt", pattern(4, 12));
    file(&"n".repeat(254), pattern(6, 13));
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

/// The block size of volumes made here.
pub const BLOCK: usize = 512;

/// Where the partition of a volume made here starts, and its length, in
/// blocks.
pub const START: u32 = 300;
pub const LEN: u32 = 200;

/// Returns a descriptor of a block, with tag `id`, saying it is at
/// `location`, with `fields` at their offsets.
pub fn tagged(id: u16, location: u32, fields: &[(usize, &[u8])]) -> Vec<u8> {
    let mut bytes = vec![0; BLOCK];
    for (at, field) in fields {
        bytes[*at..*at + field.len()].copy_from_slice(field);
    }
    bytes[0..2].copy_from_slice(&id.to_le_bytes());
    bytes[2..4].copy_from_slice(&2_u16.to_le_bytes());
    bytes[12..16].copy_from_slice(&location.to_le_bytes());
    let sum = bytes[..16]
        .iter()
        .enumerate()
        .filter(|&(at, _)| at != 4)
        .fold(0_u8, |sum, (_, &byte)| sum.wrapping_add(byte));
    bytes[4] = sum;
    bytes
}

/// Returns a short allocation descriptor of `len` bytes at `block`, of
/// kind `kind` (0 recorded, 1 and 2 not, 3 continued).
pub fn short_ad(len: u32, block: u32, kind: u32) -> Vec<u8> {
    [(len | kind << 30).to_le_bytes(), block.to_le_bytes()].concat()
}

/// Returns a long allocation descriptor of `len` bytes at `block` of
/// partition `reference`.
pub fn long_ad(len: u32, block: u32, reference: u16) -> Vec<u8> {
    let mut bytes = [len.to_le_bytes(), block.to_le_bytes()].concat();
    bytes.extend(reference.to_le_bytes());
    bytes.extend([0; 6]);
    bytes
}

/// Returns a file entry at partition block `location`, of `file_type`,
/// `len` bytes long, its data allocated by `ads` (0 short, 1 long, 3
/// embedded) as `area` gives it.
pub fn file_entry(location: u32, file_type: u8, len: u64, ads: u16, area: &[u8]) -> Vec<u8> {
    let date = [0x00, 0x10, 0xE8, 0x07, 1, 2, 3, 4, 6, 0, 0, 0];
    tagged(
        261,
        location,
        &[
            (20, &4_u16.to_le_bytes()),
            (27, &[file_type]),
            (34, &ads.to_le_bytes()),
            (56, &len.to_le_bytes()),
            (72, &date),
            (84, &date),
            (96, &date),
            (172, &u32::try_from(area.len()).unwrap().to_le_bytes()),
            (176, area),
        ],
    )
}

/// Returns a file identifier descriptor of `name` (in compressed
/// Unicode, its first byte 8 or 16), with `flags`, for the entry at
/// `block` of partition `reference`.
pub fn identifier(name: &[u8], flags: u8, block: u32, reference: u16) -> Vec<u8> {
    let len = (38 + name.len()).next_multiple_of(4);
    let mut bytes = tagged(
        257,
        0,
        &[
            (16, &1_u16.to_le_bytes()),
            (18, &[flags]),
            (19, &[u8::try_from(name.len()).unwrap()]),
            (20, &long_ad(BLOCK as u32, block, reference)),
            (38, name),
        ],
    );
    bytes.truncate(len);
    bytes
}

/// Returns a volume of `blocks` blocks of 512 bytes, with an anchor at
/// 256, a volume descriptor sequence at 32 of a partition descriptor of
/// partition 0, from block 300 for 200, a logical volume descriptor of
/// the partition maps `maps`, whose file set descriptor is at `file_set`
/// of partition `reference`, and a terminator.
pub fn volume(blocks: usize, maps: &[Vec<u8>], file_set: u32, reference: u16) -> Vec<u8> {
    let mut image = vec![0; blocks * BLOCK];
    let mut sequence = (16 * BLOCK as u32).to_le_bytes().to_vec();
    sequence.extend(32_u32.to_le_bytes());
    put(&mut image, 256, &tagged(2, 256, &[(16, &sequence)]));
    let partition = tagged(
        5,
        32,
        &[
            (22, &0_u16.to_le_bytes()),
            (188, &START.to_le_bytes()),
            (192, &LEN.to_le_bytes()),
        ],
    );
    put(&mut image, 32, &partition);
    let table = maps.concat();
    let logical = tagged(
        6,
        33,
        &[
            (212, &(BLOCK as u32).to_le_bytes()),
            (248, &long_ad(BLOCK as u32, file_set, reference)),
            (264, &u32::try_from(table.len()).unwrap().to_le_bytes()),
            (268, &u32::try_from(maps.len()).unwrap().to_le_bytes()),
            (440, &table),
        ],
    );
    put(&mut image, 33, &logical);
    put(&mut image, 34, &tagged(8, 34, &[]));
    image
}

/// Returns a physical partition map of partition 0.
pub fn physical_map() -> Vec<u8> {
    vec![1, 6, 1, 0, 0, 0]
}

/// Returns a type 2 partition map named `name`, of partition 0, with
/// `rest` after its partition number.
pub fn named_map(name: &[u8], rest: &[u8]) -> Vec<u8> {
    let mut map = vec![0; 64];
    map[0] = 2;
    map[1] = 64;
    map[5..5 + name.len()].copy_from_slice(name);
    map[36..38].copy_from_slice(&1_u16.to_le_bytes());
    map[40..40 + rest.len()].copy_from_slice(rest);
    map
}

/// Returns a file set descriptor at partition block `location`, whose
/// root's entry is at `root` of partition `reference`.
pub fn file_set(location: u32, root: u32, reference: u16) -> Vec<u8> {
    tagged(
        256,
        location,
        &[(400, &long_ad(BLOCK as u32, root, reference))],
    )
}

/// Writes `bytes` at block `block` of `image`.
pub fn put(image: &mut [u8], block: usize, bytes: &[u8]) {
    image[block * BLOCK..block * BLOCK + bytes.len()].copy_from_slice(bytes);
}

/// An image as a base image with bytes patched over it, as the
/// `cargo-fuzz` targets lay their input over a template.
pub type Overlay = filer_spec::Overlay<Vec<u8>>;

// This is free and unencumbered software released into the public domain.

//! Volumes made by real tools: formatted by `mkfs.fat` and filled by
//! `mtools`, as `tests/fixtures/make.sh` does.

mod common;

use common::{MTIME, unsparse};
use filer_api::{Dir, FileKind, MetaMask};
use filer_fat::Fat;
use filer_spec::block_on;
use std::collections::BTreeMap;

type Root = Dir<Fat<Vec<u8>>>;

/// What a path holds: a directory, or a file's contents.
#[derive(Debug, PartialEq, Eq)]
enum Expected {
    Dir,
    File(Vec<u8>),
}

/// The pattern `make.sh` writes.
fn pattern(size: usize, seed: usize) -> Vec<u8> {
    (0..size).map(|i| ((i * 7 + seed) % 251) as u8).collect()
}

/// The tree `make.sh` writes, after its deletions.
fn expected() -> BTreeMap<String, Expected> {
    let mut tree = BTreeMap::new();
    let mut file = |path: &str, size, seed| {
        tree.insert(path.to_owned(), Expected::File(pattern(size, seed)));
    };
    file("README.TXT", 9, 1);
    file("notes.txt", 10, 2);
    file("A long file name with spaces.txt", 100, 3);
    file("Ünïcödé ✓.txt", 7, 4);
    file("empty", 0, 5);
    file("big.bin", 20000, 6);
    file("Sub/nested.bin", 5000, 7);
    file("Sub/Deeper/leaf.txt", 5, 8);
    for i in 0..100 {
        file(&format!("many/f{i:03}"), i * 3, 100 + i);
    }
    file("spacer.bin", 3000, 10);
    file("frag.bin", 4000, 11);
    for dir in ["Sub", "Sub/Deeper", "many"] {
        tree.insert(dir.to_owned(), Expected::Dir);
    }
    tree
}

/// Reads everything below `dir`, recursively.
fn walk(root: &Root, dir: &str, found: &mut BTreeMap<String, Expected>) {
    let handle = block_on(root.open_dir(dir)).unwrap();
    let mut entries = handle.entries().unwrap();
    while let Some(entry) = block_on(entries.next()) {
        let entry = entry.unwrap();
        let name = entry.name.as_name().expect("UTF-8 name").as_str();
        let path = if dir.is_empty() {
            name.to_owned()
        } else {
            format!("{dir}/{name}")
        };
        match entry.kind {
            Some(FileKind::Dir) => {
                walk(root, &path, found);
                found.insert(path, Expected::Dir);
            },
            Some(FileKind::File) => {
                let file = block_on(root.open_file(path.as_str())).unwrap();
                let contents = block_on(file.read_to_end()).unwrap();
                let metadata = block_on(file.metadata(MetaMask::all())).unwrap();
                assert_eq!(metadata.len, Some(contents.len() as u64), "{path}");
                assert_eq!(metadata.modified.unwrap().secs(), MTIME, "{path}");
                found.insert(path, Expected::File(contents));
            },
            kind => panic!("{path}: unexpected {kind:?}"),
        }
    }
}

fn check(sparse: &[u8]) {
    let root = block_on(Fat::open(unsparse(sparse))).unwrap();
    let mut found = BTreeMap::new();
    walk(&root, "", &mut found);
    let expected = expected();
    let names = |tree: &BTreeMap<String, Expected>| tree.keys().cloned().collect::<Vec<_>>();
    assert_eq!(names(&found), names(&expected));
    for (path, contents) in &expected {
        assert!(&found[path] == contents, "{path}");
    }
    // Lookups ignore case, and find long names by their short aliases.
    for path in [
        "readme.txt",
        "NOTES.TXT",
        "sub/DEEPER/Leaf.TXT",
        "ünïcödé ✓.TXT",
    ] {
        assert!(block_on(root.open_file(path)).is_ok(), "{path}");
    }
}

#[test]
fn reads_fat12_made_by_mkfs_fat() {
    check(include_bytes!("fixtures/fat12.sparse"));
}

#[test]
fn reads_fat16_made_by_mkfs_fat() {
    check(include_bytes!("fixtures/fat16.sparse"));
}

#[test]
fn reads_fat32_made_by_mkfs_fat() {
    check(include_bytes!("fixtures/fat32.sparse"));
}

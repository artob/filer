// This is free and unencumbered software released into the public domain.

//! Volumes made by real tools: formatted by `mkntfs` and filled through
//! ntfs-3g, as `tests/fixtures/make.sh` does, in four layouts.

mod common;

use common::{Expected, MTIME, expected, unsparse};
use filer_api::{Dir, FileKind, MetaMask};
use filer_ntfs::Ntfs;
use filer_spec::block_on;
use std::collections::BTreeMap;

type Root = Dir<Ntfs<Vec<u8>>>;

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
        let metadata = block_on(root.metadata(path.as_str(), MetaMask::all())).unwrap();
        assert_eq!(metadata.kind, entry.kind, "{path}: kind");
        assert_eq!(metadata.id, entry.id, "{path}: id");
        match entry.kind {
            Some(FileKind::Dir) => {
                walk(root, &path, found);
                found.insert(path, Expected::Dir);
            },
            Some(FileKind::File) => {
                let file = block_on(root.open_file(path.as_str())).unwrap();
                let contents = block_on(file.read_to_end()).unwrap();
                assert_eq!(metadata.len, Some(contents.len() as u64), "{path}");
                assert_eq!(metadata.modified.unwrap().secs(), MTIME, "{path}");
                found.insert(path, Expected::File(contents));
            },
            kind => panic!("{path}: unexpected {kind:?}"),
        }
    }
}

fn check(sparse: &[u8]) {
    let root = block_on(Ntfs::open(unsparse(sparse))).unwrap();
    let mut found = BTreeMap::new();
    walk(&root, "", &mut found);
    let expected = expected();
    let names = |tree: &BTreeMap<String, Expected>| tree.keys().cloned().collect::<Vec<_>>();
    assert_eq!(names(&found), names(&expected));
    for (path, contents) in &expected {
        assert!(&found[path] == contents, "{path}");
    }

    let id = |path: &str| block_on(root.metadata(path, MetaMask::ID)).unwrap().id;
    // Hard links share an id; names differing in case are two files.
    assert_eq!(id("small"), id("Sub/hardlink"));
    assert_ne!(id("Case.txt"), id("case.txt"));
    // Lookups match exactly first, then ignoring case, and find DOS names.
    let read =
        |path: &str| block_on(block_on(root.open_file(path)).unwrap().read_to_end()).unwrap();
    assert_eq!(read("case.txt"), common::pattern(4, 12));
    assert_eq!(read("CASE.TXT"), common::pattern(3, 11));
    assert_eq!(read("readme.txt"), common::pattern(9, 1));
    assert_eq!(read("sub/DEEPER/Leaf.TXT"), common::pattern(5, 9));
    assert_eq!(read("LONGNA~1.TXT"), common::pattern(6, 13));
    assert_eq!(id("LONGNA~1.TXT"), id("Long name for DOS.txt"));
    // System files are hidden.
    assert!(block_on(root.metadata("$MFT", MetaMask::KIND)).is_err());
}

#[test]
fn reads_4k_clusters() {
    check(include_bytes!("fixtures/ntfs-4k.sparse"));
}

#[test]
fn reads_512_byte_clusters() {
    check(include_bytes!("fixtures/ntfs-512.sparse"));
}

#[test]
fn reads_64k_clusters() {
    check(include_bytes!("fixtures/ntfs-64k.sparse"));
}

#[test]
fn reads_4k_sectors_and_records() {
    check(include_bytes!("fixtures/ntfs-4kn.sparse"));
}

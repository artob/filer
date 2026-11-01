// This is free and unencumbered software released into the public domain.

//! Volumes made by real tools: formatted by `mkntfs` and filled through
//! ntfs-3g, as `tests/fixtures/make.sh` does, in four layouts.

mod common;

use common::{Expected, MTIME, expected, expected_compressed, unsparse};
use filer_api::{Dir, ErrorKind, FileKind, MetaMask, Symlinks};
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

#[test]
fn reads_compressed_files() {
    let root = block_on(Ntfs::open(unsparse(include_bytes!(
        "fixtures/ntfs-compressed.sparse"
    ))))
    .unwrap();
    let mut found = BTreeMap::new();
    walk(&root, "", &mut found);
    let expected = expected_compressed();
    let names = |tree: &BTreeMap<String, Expected>| tree.keys().cloned().collect::<Vec<_>>();
    assert_eq!(names(&found), names(&expected));
    for (path, contents) in &expected {
        assert!(&found[path] == contents, "{path}");
    }
}

#[test]
fn reads_links() {
    let root = block_on(Ntfs::open(unsparse(include_bytes!(
        "fixtures/ntfs-links.sparse"
    ))))
    .unwrap();
    let target = |path: &str| block_on(root.read_link(path)).unwrap().as_bytes().to_vec();
    assert_eq!(target("link"), b"target.txt");
    assert_eq!(target("deep"), b"Sub/nested.txt");
    assert_eq!(target("dirlink"), b"Sub");
    assert_eq!(target("wsl"), b"target.txt");
    assert_eq!(target("dangling"), b"nope");
    // Absolute paths on a drive, from the volume's root.
    assert_eq!(target("abs"), b"/target.txt");
    assert_eq!(target("junction"), b"/Sub");

    // Listings and stats agree that these are symlinks.
    let dir = block_on(root.open_dir("")).unwrap();
    let mut entries = dir.entries().unwrap();
    while let Some(entry) = block_on(entries.next()) {
        let entry = entry.unwrap();
        let name = entry.name.as_name().unwrap().as_str().to_owned();
        let metadata = block_on(root.symlink_metadata(name.as_str(), MetaMask::all())).unwrap();
        assert_eq!(entry.kind, metadata.kind, "{name}");
        let link = [
            "link", "deep", "dirlink", "wsl", "dangling", "abs", "junction",
        ];
        let expected = if link.contains(&name.as_str()) {
            FileKind::Symlink
        } else if name == "Sub" {
            FileKind::Dir
        } else {
            FileKind::File
        };
        assert_eq!(metadata.kind, Some(expected), "{name}");
    }

    let read = |root: &Root, path: &str| {
        let file = block_on(root.open_file(path))?;
        block_on(file.read_to_end())
    };
    assert_eq!(read(&root, "link").unwrap(), b"target");
    assert_eq!(read(&root, "deep").unwrap(), b"nested");
    assert_eq!(read(&root, "wsl").unwrap(), b"target");
    assert_eq!(read(&root, "dirlink/nested.txt").unwrap(), b"nested");
    assert_eq!(
        read(&root, "dangling").unwrap_err().kind(),
        ErrorKind::NotFound
    );
    // Absolute targets escape a directory confined beneath it, and resolve
    // from it as a root.
    assert_eq!(
        read(&root, "abs").unwrap_err().kind(),
        ErrorKind::EscapeBlocked
    );
    let in_root = root.clone().with_resolve(Symlinks::InRoot);
    assert_eq!(read(&in_root, "abs").unwrap(), b"target");
    assert_eq!(read(&in_root, "junction/nested.txt").unwrap(), b"nested");
    // Another tag's data is not the file's contents.
    assert_eq!(
        read(&root, "cloud.bin").unwrap_err().kind(),
        ErrorKind::Unsupported
    );
    assert_eq!(
        block_on(root.read_link("target.txt")).unwrap_err().kind(),
        ErrorKind::NotASymlink
    );
}

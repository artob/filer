// This is free and unencumbered software released into the public domain.

//! Volumes Windows itself formatted and filled, with the manifest Windows
//! wrote of each, as `tests/fixtures/make-windows.sh` makes them.
//!
//! Filer's manifest must equal Windows', line by line in listing order:
//! both list a directory in its index's collation order. Access times are
//! not compared, as Windows' own reading updates them, nor what is in
//! `System Volume Information`, which Windows keeps from administrators
//! too, and changes after writing its manifest.

mod common;

use common::{manifest::manifest, unsparse};
use filer_api::{Dir, ErrorKind, FileAttrs, MetaMask};
use filer_ntfs::Ntfs;
use filer_spec::block_on;

type Root = Dir<Ntfs<Vec<u8>>>;

/// Files Filer refuses to read, by design: encrypted (EFS), and
/// compressed by WOF. Windows' manifest has their contents and streams.
const REFUSED: [&str; 2] = ["secret.txt", "wof.bin"];

/// Windows' own directory.
const SYSTEM: &str = "System Volume Information";

/// Normalizes a manifest line for comparing: no access time, lists sorted
/// by bytes (Windows sorts by culture), and what differs by design.
fn normal(line: &str) -> String {
    let mut fields: Vec<String> = line.split('\t').map(str::to_owned).collect();
    assert_eq!(fields.len(), 9, "{line}");
    fields[5] = "-".to_owned();
    if fields[0] == SYSTEM {
        fields[3] = "-".to_owned();
        fields[4] = "-".to_owned();
    }
    if fields[0] == "wof.bin" {
        // WOF marks the file sparse on disk, and hides it from Windows.
        fields[7] = fields[7].replace(",sparse", "");
    }
    for list in [7, 8] {
        let mut items: Vec<&str> = fields[list].split(',').collect();
        items.sort_unstable();
        fields[list] = items.join(",");
    }
    if REFUSED.contains(&fields[0].as_str()) {
        fields[6] = "!error".to_owned();
        fields[8] = "!error".to_owned();
    }
    fields.join("\t")
}

fn check(sparse: &[u8], expected: &str) -> Root {
    let root = block_on(Ntfs::open(unsparse(sparse))).unwrap();
    let found = block_on(manifest(&root)).unwrap();
    let found: Vec<String> = found
        .iter()
        .filter(|line| !line.starts_with(&format!("{SYSTEM}/")))
        .map(|line| normal(line))
        .collect();
    let expected: Vec<String> = expected.lines().map(normal).collect();
    for (found, expected) in found.iter().zip(&expected) {
        assert_eq!(found, expected);
    }
    assert_eq!(found.len(), expected.len());
    root
}

/// What every layout holds, beyond what the manifest shows.
fn check_basic(root: &Root) {
    let read =
        |path: &str| block_on(block_on(root.open_file(path)).unwrap().read_to_end()).unwrap();
    // Lookups ignore case, and find the DOS name Windows made.
    assert_eq!(read("readme.txt"), common::pattern(9, 1));
    assert_eq!(read("LONGNA~1.TXT"), common::pattern(6, 13));
    let id = |path: &str| block_on(root.metadata(path, MetaMask::ID)).unwrap().id;
    assert_eq!(id("small"), id("Sub/hardlink"));
    assert_eq!(id("LONGNA~1.TXT"), id("Long name for DOS.txt"));
}

#[test]
fn reads_4k_clusters() {
    let root = check(
        include_bytes!("fixtures/windows-4k.sparse"),
        include_str!("fixtures/windows-4k.manifest"),
    );
    check_basic(&root);
    // Encrypted and WOF-compressed files have metadata, but no contents
    // Filer can read.
    for path in REFUSED {
        let error = block_on(root.open_file(path)).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Unsupported, "{path}");
    }
    let secret = block_on(root.metadata("secret.txt", MetaMask::all())).unwrap();
    assert!(secret.attrs.unwrap().contains(FileAttrs::ENCRYPTED));
}

#[test]
fn reads_512_byte_clusters() {
    let root = check(
        include_bytes!("fixtures/windows-512.sparse"),
        include_str!("fixtures/windows-512.manifest"),
    );
    check_basic(&root);
}

#[test]
fn reads_64k_clusters() {
    let root = check(
        include_bytes!("fixtures/windows-64k.sparse"),
        include_str!("fixtures/windows-64k.manifest"),
    );
    check_basic(&root);
}

#[test]
fn reads_128k_clusters() {
    let root = check(
        include_bytes!("fixtures/windows-128k.sparse"),
        include_str!("fixtures/windows-128k.manifest"),
    );
    check_basic(&root);
}

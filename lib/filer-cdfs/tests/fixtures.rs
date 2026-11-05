// This is free and unencumbered software released into the public domain.

//! Images made by `xorriso`, as `tests/fixtures/make.sh` does: of each
//! extension, holding the tree `common` expects.

mod common;

use common::{DEEP_FILE, Expected, MTIME, expected, pattern};
use core::time::Duration;
use filer_api::{
    BufResult, Dir, Driver, EntryName, ErrorKind, FileAttrs, FileKind, MetaMask, Metadata, Op,
    Output, Resolve, Symlinks, Target,
};
use filer_cdfs::{Cdfs, CdfsError};
use filer_spec::block_on;
use std::collections::BTreeMap;

type Root = Dir<Cdfs<&'static [u8]>>;

/// Reads everything below `dir`, recursively, checking that listings and
/// stats agree. Entries whose names are not UTF-8 are stated, and
/// collected in `foreign`.
fn walk(
    root: &Root,
    dir: &str,
    found: &mut BTreeMap<String, Expected>,
    foreign: &mut Vec<(EntryName, Metadata)>,
) {
    let handle = block_on(root.open_dir(dir)).unwrap();
    let mut entries = handle.entries().unwrap();
    while let Some(entry) = block_on(entries.next()) {
        let entry = entry.unwrap();
        let Some(name) = entry.name.as_name() else {
            foreign.push((entry.name.clone(), stat(&handle, entry.name)));
            continue;
        };
        let path = match dir {
            "" => name.as_str().to_owned(),
            dir => format!("{dir}/{name}"),
        };
        let metadata = block_on(root.symlink_metadata(path.as_str(), MetaMask::all())).unwrap();
        assert_eq!(entry.kind, metadata.kind, "{path}");
        assert_eq!(metadata.id, entry.id, "{path}: id");
        let modified = metadata.modified.unwrap();
        assert_eq!(modified.secs(), MTIME, "{path}");
        assert_eq!(
            modified.granularity(),
            Some(Duration::from_secs(1)),
            "{path}"
        );
        match metadata.kind.unwrap() {
            FileKind::Dir => {
                walk(root, &path, found, foreign);
                found.insert(path, Expected::Dir);
            },
            FileKind::File => {
                let file = block_on(root.open_file(path.as_str())).unwrap();
                let contents = block_on(file.read_to_end()).unwrap();
                assert_eq!(metadata.len, Some(contents.len() as u64), "{path}");
                found.insert(path, Expected::File(contents));
            },
            FileKind::Symlink => {
                let target = block_on(root.read_link(path.as_str())).unwrap();
                let target = target.as_bytes().to_vec();
                assert_eq!(metadata.len, Some(target.len() as u64), "{path}");
                found.insert(path, Expected::Link(target));
            },
            kind => {
                let error = block_on(root.open_file(path.as_str())).unwrap_err();
                assert_eq!(error.kind(), ErrorKind::NotRegularFile, "{path}");
                found.insert(path, Expected::Special(kind));
            },
        }
    }
}

/// Reads the metadata of the entry `name` of `dir`, which has no path.
fn stat(dir: &Root, name: EntryName) -> Metadata {
    let op = Op::Stat {
        target: Target::Entry {
            at: dir.handle().clone(),
            name,
            follow: false,
            resolve: Resolve::default(),
        },
        mask: MetaMask::all(),
    };
    match block_on(dir.driver().submit(op)).result.unwrap() {
        Output::Metadata(metadata) => metadata,
        output => panic!("stat returned {output:?}"),
    }
}

/// Walks the image in `image`, returning what it holds and its entries
/// whose names are not UTF-8.
fn tree(image: &'static [u8]) -> (Root, BTreeMap<String, Expected>, Vec<(EntryName, Metadata)>) {
    let root = block_on(Cdfs::open(image)).unwrap();
    let mut found = BTreeMap::new();
    let mut foreign = Vec::new();
    walk(&root, "", &mut found, &mut foreign);
    (root, found, foreign)
}

fn read(root: &Root, path: &str) -> filer_api::Result<Vec<u8>> {
    let file = block_on(root.open_file(path))?;
    block_on(file.read_to_end())
}

/// Checks an image of the tree `fill` wrote, with Rock Ridge's entries,
/// with `serials` if they number files, as Rock Ridge 1.12 does.
fn check_rock_ridge(image: &'static [u8], serials: bool) {
    let (root, found, foreign) = tree(image);
    let expected = expected();
    let names = |tree: &BTreeMap<String, Expected>| tree.keys().cloned().collect::<Vec<_>>();
    assert_eq!(names(&found), names(&expected));
    for (path, contents) in &expected {
        assert!(&found[path] == contents, "{path}");
    }
    // The name that is not UTF-8.
    let [(name, metadata)] = &foreign[..] else {
        panic!("foreign names: {foreign:?}");
    };
    let EntryName::Foreign(name) = name else {
        panic!("{name} is UTF-8");
    };
    assert_eq!(name.as_bytes(), b"caf\xE9.txt");
    assert_eq!(metadata.len, Some(3));

    let metadata = |path: &str| block_on(root.symlink_metadata(path, MetaMask::all())).unwrap();
    // Hard links share an id, if numbered; names differing in case are
    // two files.
    assert_eq!(metadata("small").id == metadata("Sub/hardlink").id, serials);
    assert_ne!(metadata("Case.txt").id, metadata("case.txt").id);
    assert_ne!(metadata("Sub").id, metadata("Sub/Deeper").id);
    // Lookups match bytes exactly.
    assert_eq!(read(&root, "case.txt").unwrap(), pattern(4, 12));
    assert_eq!(
        read(&root, "readme.txt").unwrap_err().kind(),
        ErrorKind::NotFound
    );
    // Symlinks resolve, confined.
    assert_eq!(read(&root, "link").unwrap(), pattern(9, 1));
    assert_eq!(read(&root, "dirlink/nested.bin").unwrap(), pattern(5000, 8));
    assert_eq!(
        read(&root, "abs").unwrap_err().kind(),
        ErrorKind::EscapeBlocked
    );
    let in_root = root.clone().with_resolve(Symlinks::InRoot);
    assert_eq!(read(&in_root, "abs").unwrap(), pattern(10, 2));
    assert_eq!(
        read(&root, "dangling").unwrap_err().kind(),
        ErrorKind::NotFound
    );
    // Directories are not files, and files not directories.
    assert_eq!(
        read(&root, "Sub").unwrap_err().kind(),
        ErrorKind::IsADirectory
    );
    assert_eq!(
        block_on(root.open_dir("README.TXT")).unwrap_err().kind(),
        ErrorKind::NotADirectory
    );
}

/// Rock Ridge 1.12 alone; 1.10 with Joliet too, and a directory
/// relocated for its depth, shown where it belongs; and 1.12 with every
/// file zisofs shrinks compressed.
#[test]
fn reads_rock_ridge() {
    let mut images = vec![
        (&include_bytes!("fixtures/rock-ridge.iso")[..], true),
        (include_bytes!("fixtures/rock-ridge-joliet.iso"), false),
    ];
    if cfg!(feature = "zisofs") {
        images.push((include_bytes!("fixtures/zisofs.iso"), true));
    }
    for (image, serials) in images {
        check_rock_ridge(image, serials);
    }
}

/// Without the `zisofs` feature, what it compresses does not open, but
/// lists, with its size uncompressed; what it leaves reads.
#[cfg(not(feature = "zisofs"))]
#[test]
fn refuses_compressed_files_without_the_feature() {
    let root = block_on(Cdfs::open(&include_bytes!("fixtures/zisofs.iso")[..])).unwrap();
    let error = read(&root, "big.bin").unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Unsupported);
    let metadata = block_on(root.symlink_metadata("big.bin", MetaMask::all())).unwrap();
    assert_eq!(metadata.len, Some(20_000));
    assert_eq!(read(&root, "small").unwrap(), pattern(4, 10));
}

/// Joliet's UTF-16 names, of up to 103 characters, where Rock Ridge's
/// are not: no symlinks or special files, and names as `xorriso` makes
/// them of what UTF-16 holds.
#[test]
fn reads_joliet() {
    let (root, found, foreign) = tree(include_bytes!("fixtures/joliet.iso"));
    assert!(foreign.is_empty(), "{foreign:?}");
    for (path, contents) in [
        ("README.TXT", pattern(9, 1)),
        ("A long file name with spaces.txt", pattern(100, 3)),
        ("Ünïcödé ✓.txt", pattern(7, 4)),
        ("Case.txt", pattern(3, 11)),
        ("case.txt", pattern(4, 12)),
        ("many/f029", pattern(29, 129)),
        (DEEP_FILE, pattern(6, 15)),
        (&"n".repeat(103), pattern(6, 13)),
    ] {
        assert_eq!(found[path], Expected::File(contents), "{path}");
    }
    assert!(
        found
            .values()
            .all(|what| matches!(what, Expected::File(_) | Expected::Dir))
    );
    // All but the six symlinks and the four special files, and the name
    // that is not UTF-8 too, as `caf_.txt`.
    assert_eq!(found.len(), expected().len() - 9, "{:?}", found.keys());
    assert!(found.contains_key("caf_.txt"));
    let metadata = block_on(root.symlink_metadata("README.TXT", MetaMask::all())).unwrap();
    assert_eq!((metadata.accessed, metadata.created), (None, None));
}

/// The primary tree's names alone: upper case, in eight characters and
/// three, shown without their version.
#[test]
fn reads_plain_names() {
    let (_, found, foreign) = tree(include_bytes!("fixtures/plain.iso"));
    assert!(foreign.is_empty(), "{foreign:?}");
    for (path, contents) in [
        ("README.TXT", pattern(9, 1)),
        ("NOTES.TXT", pattern(10, 2)),
        ("BIG.BIN", pattern(20_000, 7)),
        ("EMPTY", Vec::new()),
        ("MANY/F029", pattern(29, 129)),
        ("A/B/C/D/E/F/G/H/I/BOTTOM.TXT", pattern(6, 15)),
    ] {
        assert_eq!(found[path], Expected::File(contents), "{path}");
    }
    assert!(found.keys().all(|path| !path.contains(';')));
}

/// ISO 9660:1999's names: any bytes, up to 207.
#[test]
fn reads_iso9660_1999_names() {
    let (_, found, foreign) = tree(include_bytes!("fixtures/iso9660-1999.iso"));
    for (path, contents) in [
        ("A long file name with spaces.txt", pattern(100, 3)),
        ("emoji 😀.bin", pattern(5, 5)),
        ("Case.txt", pattern(3, 11)),
        ("case.txt", pattern(4, 12)),
        (DEEP_FILE, pattern(6, 15)),
        (&"n".repeat(207), pattern(6, 13)),
    ] {
        assert_eq!(found[path], Expected::File(contents), "{path}");
    }
    let [(EntryName::Foreign(name), _)] = &foreign[..] else {
        panic!("foreign names: {foreign:?}");
    };
    assert_eq!(name.as_bytes(), b"caf\xE9.txt");
}

/// What zisofs compresses reads whole, and is marked so: what it shrinks,
/// as `xorriso` leaves files it does not.
#[test]
fn marks_compressed_files() {
    for (image, compressed) in [
        (&include_bytes!("fixtures/zisofs.iso")[..], true),
        (include_bytes!("fixtures/rock-ridge.iso"), false),
    ] {
        let root = block_on(Cdfs::open(image)).unwrap();
        for (path, shrinks) in [("big.bin", true), ("zeros.bin", true), ("small", false)] {
            let metadata = block_on(root.symlink_metadata(path, MetaMask::all())).unwrap();
            let attrs = metadata.attrs.unwrap();
            assert_eq!(
                attrs.contains(FileAttrs::COMPRESSED),
                compressed && shrinks,
                "{path}"
            );
            assert!(!attrs.contains(FileAttrs::HIDDEN), "{path}");
        }
    }
}

/// Reads of every size, from anywhere: across blocks, past the end, and
/// out of order.
#[test]
fn reads_at_any_offset() {
    let files = [
        ("big.bin", pattern(20_000, 7)),
        ("zeros.bin", vec![0; 20_000]),
        ("Sub/nested.bin", pattern(5000, 8)),
    ];
    let mut images = vec![&include_bytes!("fixtures/rock-ridge.iso")[..]];
    if cfg!(feature = "zisofs") {
        images.push(include_bytes!("fixtures/zisofs.iso"));
    }
    for image in images {
        let root = block_on(Cdfs::open(image)).unwrap();
        for (path, contents) in &files {
            let file = block_on(root.open_file(*path)).unwrap();
            let len = contents.len() as u64;
            for off in [len - 1, 0, 2047, 2048, 4999, 1, len, len + 9, 10_000] {
                for size in [1, 2, 2049, 40_000] {
                    let BufResult(got, buf) = block_on(file.read_at(Vec::with_capacity(size), off));
                    let got = got.unwrap();
                    let start = (off.min(len)) as usize;
                    let end = (off.saturating_add(size as u64).min(len)) as usize;
                    assert_eq!(got, end - start, "{path} at {off}");
                    assert!(buf[..] == contents[start..end], "{path} at {off}");
                }
            }
        }
    }
}

/// What is not an ISO 9660 image this driver reads.
#[test]
fn refuses_what_it_does_not_read() {
    let image = include_bytes!("fixtures/rock-ridge.iso");
    let open = |bytes: Vec<u8>| block_on(Cdfs::open(bytes)).unwrap_err();
    let source = |error: &filer_api::Error| {
        *core::error::Error::source(error)
            .unwrap()
            .downcast_ref::<CdfsError>()
            .unwrap()
    };
    let patched = |at: usize, bytes: &[u8]| {
        let mut patched = image.to_vec();
        patched[at..at + bytes.len()].copy_from_slice(bytes);
        patched
    };
    for (bytes, expected) in [
        (vec![0; 40_000], CdfsError::NotIso9660),
        (vec![0; 100], CdfsError::NotIso9660),
        (patched(32768, b"\x02"), CdfsError::NoPrimary),
        (
            patched(32768 + 128, &1000_u16.to_le_bytes()),
            CdfsError::BadBlockSize(1000),
        ),
        // The root's record, no directory.
        (patched(32768 + 156 + 25, &[0]), CdfsError::BadRoot),
    ] {
        let error = open(bytes);
        assert_eq!(error.kind(), ErrorKind::Corrupt);
        assert_eq!(source(&error), expected);
    }
}

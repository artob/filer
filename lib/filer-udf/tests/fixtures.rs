// This is free and unencumbered software released into the public domain.

//! Volumes made by `mkudffs` and filled by the Linux kernel, as
//! `tests/fixtures/make.sh` does: of each revision and layout, holding the
//! tree `common` expects.

mod common;

use common::{Expected, MTIME, expected, image, pattern};
use core::time::Duration;
use filer_api::{
    BufResult, Dir, Driver, EntryName, ErrorKind, FileKind, MetaMask, Metadata, Op, Output,
    Resolve, Symlinks, Target,
};
use filer_spec::block_on;
use filer_udf::{Udf, UdfError};
use std::collections::BTreeMap;

type Root = Dir<Udf<Vec<u8>>>;

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
        assert_eq!((modified.secs(), modified.nanos()), (MTIME, 0), "{path}");
        assert_eq!(
            modified.granularity(),
            Some(Duration::from_micros(1)),
            "{path}"
        );
        assert_eq!(metadata.accessed.unwrap().secs(), MTIME, "{path}");
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

/// Walks the volume in `image`, returning what it holds and its entries
/// whose names are not UTF-8.
fn tree(image: Vec<u8>) -> (Root, BTreeMap<String, Expected>, Vec<(EntryName, Metadata)>) {
    let root = block_on(Udf::open(image)).unwrap();
    let mut found = BTreeMap::new();
    let mut foreign = Vec::new();
    walk(&root, "", &mut found, &mut foreign);
    (root, found, foreign)
}

fn read(root: &Root, path: &str) -> filer_api::Result<Vec<u8>> {
    let file = block_on(root.open_file(path))?;
    block_on(file.read_to_end())
}

/// Checks a volume of the tree `fill` wrote.
fn check(image: Vec<u8>) {
    let (root, found, foreign) = tree(image);
    assert!(foreign.is_empty(), "{foreign:?}");
    let expected = expected();
    let names = |tree: &BTreeMap<String, Expected>| tree.keys().cloned().collect::<Vec<_>>();
    assert_eq!(names(&found), names(&expected));
    for (path, contents) in &expected {
        assert!(&found[path] == contents, "{path}");
    }
    let metadata = |path: &str| block_on(root.symlink_metadata(path, MetaMask::all())).unwrap();
    // Hard links share an id; names differing in case are two files.
    assert_eq!(metadata("small").id, metadata("Sub/hardlink").id);
    assert_ne!(metadata("Case.txt").id, metadata("case.txt").id);
    // The deleted file is gone.
    assert_eq!(
        read(&root, "gone.txt").unwrap_err().kind(),
        ErrorKind::NotFound
    );
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

/// UDF 1.02, of file entries and short allocation descriptors; and 2.01,
/// of extended file entries and long ones; small files embedded.
#[test]
fn reads_each_revision() {
    for sparse in [
        &include_bytes!("fixtures/udf-1.02.sparse")[..],
        include_bytes!("fixtures/udf-2.01.sparse"),
    ] {
        check(image(sparse));
    }
}

/// A BD-R's volume of UDF 2.50, its partition virtual: empty, as Linux
/// does not write it, its root found through its virtual allocation
/// table.
#[test]
fn reads_a_virtual_partition() {
    let root = block_on(Udf::open(image(include_bytes!("fixtures/udf-2.50.sparse")))).unwrap();
    assert!(block_on(root.names()).unwrap().is_empty());
    let metadata = block_on(root.symlink_metadata("", MetaMask::all())).unwrap();
    assert_eq!(metadata.kind, Some(FileKind::Dir));
}

/// Reads of every size, from anywhere: across blocks and extents, past
/// the end, and out of order.
#[test]
fn reads_at_any_offset() {
    let files = [
        ("big.bin", pattern(20_000, 7)),
        ("Sub/nested.bin", pattern(5000, 8)),
        ("README.TXT", pattern(9, 1)),
    ];
    for sparse in [
        &include_bytes!("fixtures/udf-1.02.sparse")[..],
        include_bytes!("fixtures/udf-2.01.sparse"),
    ] {
        let root = block_on(Udf::open(image(sparse))).unwrap();
        for (path, contents) in &files {
            let file = block_on(root.open_file(*path)).unwrap();
            let len = contents.len() as u64;
            for off in [len - 1, 0, 511, 512, 4999, 1, len, len + 9, 10_000] {
                for size in [1, 2, 513, 40_000] {
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

/// What is not a UDF volume this driver reads.
#[test]
fn refuses_what_it_does_not_read() {
    let image = image(include_bytes!("fixtures/udf-2.01.sparse"));
    let open = |bytes: Vec<u8>| block_on(Udf::open(bytes)).unwrap_err();
    let source = |error: &filer_api::Error| {
        *core::error::Error::source(error)
            .unwrap()
            .downcast_ref::<UdfError>()
            .unwrap()
    };
    let mut no_anchors = image.to_vec();
    for block in [256, image.len() / 512 - 257, image.len() / 512 - 1] {
        no_anchors[block * 512] = 0;
    }
    for (bytes, expected) in [
        (vec![0; 400_000], UdfError::NoAnchor),
        (vec![0; 100], UdfError::NoAnchor),
        (no_anchors, UdfError::NoAnchor),
    ] {
        let error = open(bytes);
        assert_eq!(error.kind(), ErrorKind::Corrupt);
        assert_eq!(source(&error), expected);
    }
}

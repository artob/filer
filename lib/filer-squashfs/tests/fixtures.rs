// This is free and unencumbered software released into the public domain.

//! Images made by `mksquashfs`, as `tests/fixtures/make.sh` does: of each
//! compressor, block size, and layout, holding the tree `common` expects;
//! of extended attributes; and malformed ones.

mod common;

use common::{Expected, MTIME, deep, expected, noise, pattern};
use core::time::Duration;
use filer_api::{
    BufResult, Dir, Driver, EntryName, ErrorKind, FileKind, MetaMask, Metadata, Op, Output,
    Resolve, Symlinks, Target,
};
use filer_spec::block_on;
use filer_squashfs::{Squashfs, SquashfsError};
use std::collections::BTreeMap;

type Root = Dir<Squashfs<&'static [u8]>>;

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
        // Only the modification time is recorded, in seconds.
        let modified = metadata.modified.unwrap();
        assert_eq!(modified.secs(), MTIME, "{path}");
        assert_eq!(
            modified.granularity(),
            Some(Duration::from_secs(1)),
            "{path}"
        );
        assert_eq!(
            (metadata.changed, metadata.accessed, metadata.created),
            (None, None, None),
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

/// Checks an image of the tree `fill` wrote.
fn check(image: &'static [u8]) {
    let root = block_on(Squashfs::open(image)).unwrap();
    let mut found = BTreeMap::new();
    let mut foreign = Vec::new();
    walk(&root, "", &mut found, &mut foreign);
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
    // Hard links share an id; names differing in case are two files.
    assert_eq!(metadata("small").id, metadata("Sub/hardlink").id);
    assert_ne!(metadata("Case.txt").id, metadata("case.txt").id);
    // Lookups match bytes exactly.
    let read = |root: &Root, path: &str| {
        let file = block_on(root.open_file(path))?;
        block_on(file.read_to_end())
    };
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

/// Each compressor, with blocks of 128 KiB.
#[test]
fn reads_each_compressor() {
    for image in [
        &include_bytes!("fixtures/gzip.sqfs")[..],
        include_bytes!("fixtures/lzma.sqfs"),
        include_bytes!("fixtures/lzo.sqfs"),
        include_bytes!("fixtures/lzo-1.sqfs"),
        include_bytes!("fixtures/xz.sqfs"),
        include_bytes!("fixtures/xz-bcj.sqfs"),
        include_bytes!("fixtures/lz4.sqfs"),
        include_bytes!("fixtures/lz4-hc.sqfs"),
        include_bytes!("fixtures/zstd.sqfs"),
    ] {
        check(image);
    }
}

/// Blocks of 4 KiB and 1 MiB; nothing compressed; no fragments; and every
/// tail in a fragment.
#[test]
fn reads_each_layout() {
    for image in [
        &include_bytes!("fixtures/small-blocks.sqfs")[..],
        include_bytes!("fixtures/large-blocks.sqfs"),
        include_bytes!("fixtures/uncompressed.sqfs"),
        include_bytes!("fixtures/no-fragments.sqfs"),
        include_bytes!("fixtures/all-fragments.sqfs"),
    ] {
        check(image);
    }
}

/// Reads of every size, from anywhere: across blocks, into tails, past
/// the end, and far into a sparse file, out of order.
#[test]
fn reads_at_any_offset() {
    let files = [
        ("big.bin", pattern(100_000, 7)),
        ("random.bin", noise(135_000)),
        (
            "frag.a",
            (0..12).flat_map(|i| pattern(4096, 20 + i)).collect(),
        ),
        ("deep.bin", deep()),
    ];
    for image in [
        &include_bytes!("fixtures/small-blocks.sqfs")[..],
        include_bytes!("fixtures/gzip.sqfs"),
        include_bytes!("fixtures/all-fragments.sqfs"),
    ] {
        let root = block_on(Squashfs::open(image)).unwrap();
        for (path, contents) in &files {
            let file = block_on(root.open_file(*path)).unwrap();
            let len = contents.len() as u64;
            let mut offsets = vec![0, 1, 4095, 4096, 131_071, 131_072, len - 1, len, len + 9];
            offsets.extend(
                [69_999_998, 300_000, 3]
                    .into_iter()
                    .filter(|&off| off < len),
            );
            for off in offsets {
                for size in [1, 2, 4097, 140_000] {
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

/// Attributes of each namespace, many to a file, large, and sharing a
/// value, compressed and not.
#[test]
fn reads_extended_attributes() {
    reads_attributes_of(include_bytes!("fixtures/attrs.sqfs"));
    reads_attributes_of(include_bytes!("fixtures/attrs-uncompressed.sqfs"));
}

fn reads_attributes_of(image: &'static [u8]) {
    let root = block_on(Squashfs::open(image)).unwrap();
    let pairs = |path: &str| {
        let file = block_on(root.open_file(path)).unwrap();
        let mut pairs = Vec::new();
        for name in block_on(file.xattr_names()).unwrap() {
            let value = block_on(file.xattr(name.clone())).unwrap().unwrap();
            pairs.push((name.to_string(), value));
        }
        pairs.sort();
        pairs
    };
    assert_eq!(pairs("plain"), []);
    assert_eq!(
        pairs("one"),
        [
            ("security.label".to_owned(), b"label".to_vec()),
            ("trusted.secret".to_owned(), b"hidden".to_vec()),
            ("user.comment".to_owned(), b"a comment".to_vec()),
            ("user.empty".to_owned(), Vec::new()),
        ]
    );
    let many = pairs("many");
    assert_eq!(many.len(), 200);
    for (i, (name, value)) in many.iter().enumerate() {
        assert_eq!(name, &format!("user.attribute.{i:03}"));
        assert_eq!(value, &pattern(40, i));
    }
    for (name, value) in [("shared.a", b"shared.a"), ("shared.b", b"shared.b")] {
        assert_eq!(
            pairs(name),
            [
                ("user.own".to_owned(), value.to_vec()),
                ("user.same".to_owned(), pattern(1000, 1)),
            ]
        );
    }
    assert_eq!(
        pairs("large"),
        [("user.large".to_owned(), pattern(20_000, 2))]
    );
    let dir = block_on(root.open_dir("dir")).unwrap();
    assert_eq!(block_on(dir.xattr("user.tag")).unwrap().unwrap(), b"dir");
}

/// What is not a SquashFS image this driver reads.
#[test]
fn refuses_what_it_does_not_read() {
    let image = include_bytes!("fixtures/gzip.sqfs");
    let open = |bytes: Vec<u8>| block_on(Squashfs::open(bytes)).unwrap_err();
    let source = |error: &filer_api::Error| {
        *core::error::Error::source(error)
            .unwrap()
            .downcast_ref::<SquashfsError>()
            .unwrap()
    };
    let patched = |at: usize, bytes: &[u8]| {
        let mut patched = image.to_vec();
        patched[at..at + bytes.len()].copy_from_slice(bytes);
        patched
    };
    let cases = [
        (patched(0, b"sqsh"), ErrorKind::Corrupt),
        (patched(28, &[3, 0, 1, 0]), ErrorKind::Unsupported),
        (patched(20, &[7, 0]), ErrorKind::Unsupported),
        (patched(12, &[0, 0, 4, 0]), ErrorKind::Corrupt),
        (image[..image.len() / 2].to_vec(), ErrorKind::Corrupt),
        (image[..50].to_vec(), ErrorKind::Corrupt),
    ];
    let expected = [
        SquashfsError::BadMagic(0x6873_7173),
        SquashfsError::UnsupportedVersion { major: 3, minor: 1 },
        SquashfsError::UnsupportedCompressor(7),
        SquashfsError::BadBlockSize {
            size: 0x4_0000,
            log: 17,
        },
        // `mksquashfs` pads images to 4 KiB, past what they use.
        SquashfsError::Truncated {
            image: u64::from_le_bytes(image[40..48].try_into().unwrap()),
            source: image.len() as u64 / 2,
        },
        SquashfsError::ShortSuperblock,
    ];
    for ((bytes, kind), expected) in cases.into_iter().zip(expected) {
        let error = open(bytes);
        assert_eq!((error.kind(), source(&error)), (kind, expected));
    }
}

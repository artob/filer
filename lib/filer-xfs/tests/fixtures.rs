// This is free and unencumbered software released into the public domain.

//! Volumes made by real tools: formatted by `mkfs.xfs` and filled through
//! the Linux kernel, or by `mkfs.xfs -p`, as `tests/fixtures/make.sh` does.

mod common;

use common::{Expected, MTIME, MTIME_NANOS, Sparse, expected, pattern};
use core::time::Duration;
use filer_api::{
    BufResult, Dir, Driver, EntryName, ErrorKind, FileAttrs, FileKind, MetaMask, Metadata, Op,
    Output, Resolve, Symlinks, Target, XattrEntry, XattrNs,
};
use filer_spec::block_on;
use filer_xfs::Xfs;
use std::collections::BTreeMap;

type Root = Dir<Xfs<Sparse>>;

/// Opens the volume in a fixture.
fn open(sparse: &[u8]) -> Root {
    block_on(Xfs::open(Sparse::new(sparse))).unwrap()
}

/// Reads everything below `dir`, recursively, checking that listings and
/// stats agree. Entries whose names are not UTF-8 are stated, and
/// collected in `foreign`.
fn walk(
    root: &Root,
    dir: &str,
    v5: bool,
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
        // Volumes without the `FTYPE` feature list no kinds.
        assert!(
            entry.kind.is_none() || entry.kind == metadata.kind,
            "{path}"
        );
        assert_eq!(metadata.id, entry.id, "{path}: id");
        let kind = metadata.kind.unwrap();
        if matches!(kind, FileKind::File | FileKind::Symlink) {
            check_time(&metadata, &path, v5);
        }
        match kind {
            FileKind::Dir => {
                walk(root, &path, v5, found, foreign);
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

/// Checks the modification time `make.sh` gives, in nanoseconds; only
/// version 5's inodes record a creation time.
fn check_time(metadata: &Metadata, path: &str, v5: bool) {
    let modified = metadata.modified.unwrap();
    assert_eq!(modified.secs(), MTIME, "{path}");
    assert_eq!(
        modified.granularity(),
        Some(Duration::from_nanos(1)),
        "{path}"
    );
    assert_eq!(modified.nanos(), MTIME_NANOS, "{path}");
    assert_eq!(metadata.created.is_some(), v5, "{path}");
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

/// Checks a fixture `fill` wrote, on a version 5 volume if `v5`.
fn check(sparse: &[u8], v5: bool) {
    let root = open(sparse);
    let mut found = BTreeMap::new();
    let mut foreign = Vec::new();
    walk(&root, "", v5, &mut found, &mut foreign);
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
    assert_eq!(read(&root, "deeplink").unwrap(), pattern(5, 9));
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
    assert_eq!(
        block_on(root.read_link("README.TXT")).unwrap_err().kind(),
        ErrorKind::NotASymlink
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

#[test]
fn reads_512_byte_blocks() {
    check(include_bytes!("fixtures/xfs-512.sparse"), false);
}

/// With 512-byte inodes, and no kinds in directory entries.
#[test]
fn reads_1k_blocks() {
    check(include_bytes!("fixtures/xfs-1k.sparse"), false);
}

/// With 1 KiB inodes and 8 KiB directory blocks.
#[test]
fn reads_2k_blocks() {
    check(include_bytes!("fixtures/xfs-2k.sparse"), false);
}

#[test]
fn reads_4k_blocks() {
    check(include_bytes!("fixtures/xfs-4k.sparse"), false);
}

/// With 2 KiB inodes and 64 KiB directory blocks.
#[test]
fn reads_large_inodes_and_directory_blocks() {
    check(include_bytes!("fixtures/xfs-4k-big.sparse"), false);
}

/// Filled by `mkfs.xfs -p`, as x86 Linux cannot mount version 4 volumes
/// of blocks larger than its pages; its files have no holes.
#[test]
fn reads_64k_blocks() {
    check(include_bytes!("fixtures/xfs-64k.sparse"), false);
}

/// Version 5, the smallest blocks it allows: inodes, `BMA3` B+ trees, and
/// directories, self-describing.
#[test]
fn reads_1k_blocks_v5() {
    check(include_bytes!("fixtures/xfs5-1k.sparse"), true);
}

#[test]
fn reads_4k_blocks_v5() {
    check(include_bytes!("fixtures/xfs5-4k.sparse"), true);
}

/// Filled through Linux, which mounts version 5 volumes of blocks larger
/// than its pages.
#[test]
fn reads_64k_blocks_v5() {
    check(include_bytes!("fixtures/xfs5-64k.sparse"), true);
}

/// As `mkfs.xfs` makes volumes by default: with sparse inode chunks,
/// reverse maps, reflinks, `BIGTIME`, and `NREXT64`.
#[test]
fn reads_default_volumes() {
    check(include_bytes!("fixtures/xfs5d-4k.sparse"), true);
    check_dirs(include_bytes!("fixtures/xfs5d-dirs.sparse"));
    check_attributes(include_bytes!("fixtures/xfs5d-attrs.sparse"));
}

/// What later features change: times outside the legacy range, blocks
/// that reflinked copies share, and a file of many extents.
#[test]
fn reads_what_later_features_change() {
    let root = open(include_bytes!("fixtures/xfs5d-later.sparse"));
    let read = |path: &str| {
        let file = block_on(root.open_file(path)).unwrap();
        block_on(file.read_to_end()).unwrap()
    };
    for (path, secs, nanos) in [
        ("y1901", -2_147_483_648, 0),
        ("y1960", -315_619_200, 999_999_999),
        ("y2038", 2_147_483_648, 1),
        ("y2400", 13_569_465_600, 7),
    ] {
        let metadata = block_on(root.metadata(path, MetaMask::all())).unwrap();
        let modified = metadata.modified.unwrap();
        assert_eq!((modified.secs(), modified.nanos()), (secs, nanos), "{path}");
        assert_eq!(read(path), path.as_bytes());
    }
    let original = pattern(60_000, 3);
    assert_eq!(read("original"), original);
    assert_eq!(read("copy"), original);
    let mut changed = original;
    changed[20_000..25_000].copy_from_slice(&pattern(5000, 9));
    assert_eq!(read("changed"), changed);
    let extents = read("extents");
    assert_eq!(extents.len(), 598 * 4096 + 16);
    for i in 0..=598 {
        let block = &extents[i * 4096..(i * 4096 + 16).min(extents.len())];
        if i % 2 == 0 {
            assert_eq!(block, pattern(16, i % 251), "block {i}");
        } else {
            assert!(block.iter().all(|&b| b == 0), "block {i}");
        }
    }
}

/// A volume whose UUID changed after it was made: its metadata carries
/// `sb_meta_uuid`, the old one.
#[test]
fn reads_metadata_of_another_uuid() {
    let root = open(include_bytes!("fixtures/xfs5-uuid.sparse"));
    let file = block_on(root.open_file("a/b/c.txt")).unwrap();
    assert_eq!(block_on(file.read_to_end()).unwrap(), b"contents");
    let dir = block_on(root.open_dir("many")).unwrap();
    assert_eq!(list(&dir, 1000).len(), 600);
}

/// Changing one byte of any version 5 metadata block makes what reads it
/// `Corrupt`, by its checksum: the superblock, inodes, and blocks of each
/// kind, found by their magic numbers. But a directory's leaves and nodes
/// only speed lookups up: when one is corrupt, lookups scan instead.
#[test]
fn refuses_corrupt_checksums() {
    use common::exercise::{Overlay, exercise};
    use std::sync::Arc;

    // Magic numbers, at the start of a block or at 8, which a corrupt
    // block of makes some read fail, or none; each in the volume that has
    // it, and walks fastest.
    type Magics<'a> = &'a [&'a [u8]];
    let cases: [(&[u8], Magics<'_>, Magics<'_>); 3] = [
        (
            include_bytes!("fixtures/xfs5-4k.sparse"),
            &[b"IN", b"XDB3", b"XSLM"],
            &[],
        ),
        (
            include_bytes!("fixtures/xfs5-dirs.sparse"),
            &[b"XDD3"],
            &[&[0x3D, 0xF1], &[0x3D, 0xFF], &[0x3E, 0xBE]],
        ),
        (
            include_bytes!("fixtures/xfs5-attrs.sparse"),
            &[b"XARM", b"BMA3", &[0x3B, 0xEE], &[0x3E, 0xBE]],
            &[],
        ),
    ];
    for (sparse, fails, passes) in cases {
        let image = Arc::new(Sparse::new(sparse));
        // The superblock's sector, past what is parsed.
        let mut overlay = Overlay::new(image.clone());
        overlay.patch(500, &[1]);
        let error = block_on(Xfs::open(overlay)).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Corrupt);
        // The first block of each kind, a byte in its first sector past
        // any header.
        let first = |magic: &[u8]| {
            let at = if magic.len() == 2 && magic != b"IN" {
                8
            } else {
                0
            };
            image.sectors().find(|&offset| {
                let mut found = vec![0; magic.len()];
                image.copy(offset + at, &mut found);
                found == magic
            })
        };
        let corrupt = |offset: u64| {
            let mut overlay = Overlay::new(image.clone());
            overlay.patch(offset + 255, &[!image.byte(offset + 255)]);
            exercise(overlay).errors
        };
        for &magic in fails {
            let name = magic.escape_ascii();
            let offset = first(magic).unwrap_or_else(|| panic!("{name}: none"));
            assert!(corrupt(offset) > 0, "{name}: corrupt, but read");
        }
        // Each of which a few lookups show, as a walk would scan for each
        // of thousands.
        for &magic in passes {
            let offset = first(magic).unwrap();
            let mut overlay = Overlay::new(image.clone());
            overlay.patch(offset + 255, &[!image.byte(offset + 255)]);
            let root = block_on(Xfs::open(overlay)).unwrap();
            for path in ["leaf/f00399", "node/f00000", "node/f00999", "btree/f01999"] {
                let found = block_on(root.symlink_metadata(path, MetaMask::all()));
                assert!(found.is_ok(), "{}: {path}", magic.escape_ascii());
            }
        }
    }
}

/// Lists directory `dir` in batches of at most `max` entries.
fn list(dir: &Root, max: usize) -> Vec<String> {
    let mut names = Vec::new();
    let mut cookie = None;
    loop {
        let op = Op::ReadDir {
            dir: dir.handle().clone(),
            cookie,
            max,
        };
        let Output::Entries { entries, next } = block_on(dir.driver().submit(op)).result.unwrap()
        else {
            panic!("not entries");
        };
        assert!(entries.len() <= max);
        names.extend(entries.into_iter().map(|entry| entry.name.to_string()));
        match next {
            Some(next) => cookie = Some(next),
            None => break names,
        }
    }
}

#[test]
fn lists_in_batches() {
    let root = open(include_bytes!("fixtures/xfs-512.sparse"));
    let dir = block_on(root.open_dir("many")).unwrap();
    let mut names = list(&dir, 7);
    names.sort();
    let expected: Vec<_> = (0..150).map(|i| format!("f{i:03}")).collect();
    assert_eq!(names, expected);
}

/// Each form of directory, as `make.sh` checks they are: short form,
/// block, leaf, node, and node mapped by a B+ tree.
#[test]
fn reads_each_form_of_directory() {
    check_dirs(include_bytes!("fixtures/xfs-dirs.sparse"));
}

#[test]
fn reads_each_form_of_directory_v5() {
    check_dirs(include_bytes!("fixtures/xfs5-dirs.sparse"));
}

/// Checks a fixture `dirs` wrote.
fn check_dirs(sparse: &[u8]) {
    let root = open(sparse);
    let target = block_on(root.symlink_metadata("target", MetaMask::all()))
        .unwrap()
        .id;
    for (form, count) in [
        ("short", 3),
        ("block", 40),
        ("leaf", 400),
        ("node", 1000),
        ("btree", 2000),
    ] {
        let dir = block_on(root.open_dir(form)).unwrap();
        let mut names = list(&dir, 7);
        names.sort();
        let expected: Vec<_> = (0..count).map(|i| format!("f{i:05}")).collect();
        assert_eq!(names, expected, "{form}");
        // Every entry listed is found, by hash where there are leaves, and
        // is a link to `target`.
        for name in &names {
            let path = format!("{form}/{name}");
            let metadata = block_on(root.symlink_metadata(path.as_str(), MetaMask::all())).unwrap();
            assert_eq!(metadata.id, target, "{path}");
        }
        for i in [0, count / 2, count - 1] {
            let path = format!("{form}/f{i:05}");
            let file = block_on(root.open_file(path.as_str())).unwrap();
            assert_eq!(block_on(file.read_to_end()).unwrap(), b"target", "{path}");
        }
        let path = format!("{form}/f{count:05}");
        assert_eq!(
            block_on(root.open_file(path.as_str())).unwrap_err().kind(),
            ErrorKind::NotFound,
            "{path}"
        );
    }
}

/// The POSIX ACL `make.sh` sets, as Linux's `getxattr` returns it.
fn acl() -> Vec<u8> {
    let mut acl = 2u32.to_le_bytes().to_vec();
    for (tag, perm, id) in [
        (1u16, 6u16, u32::MAX),
        (2, 4, 1000),
        (4, 4, u32::MAX),
        (0x10, 4, u32::MAX),
        (0x20, 0, u32::MAX),
    ] {
        acl.extend(tag.to_le_bytes());
        acl.extend(perm.to_le_bytes());
        acl.extend(id.to_le_bytes());
    }
    acl
}

/// Checks a fixture `attrs` wrote, whose forks `make.sh` checks are of
/// the forms the files are named for.
fn check_attributes(sparse: &[u8]) {
    let root = open(sparse);
    let names = |path: &str| {
        let file = block_on(root.open_file(path)).unwrap();
        let mut names: Vec<String> = block_on(file.xattr_names())
            .unwrap()
            .iter()
            .map(ToString::to_string)
            .collect();
        names.sort();
        names
    };
    let value = |path: &str, name: &str| {
        let file = block_on(root.open_file(path)).unwrap();
        block_on(file.xattr(name)).unwrap()
    };
    assert!(names("plain.txt").is_empty());
    // Short form.
    assert_eq!(names("small.txt"), ["user.empty", "user.small"]);
    assert_eq!(value("small.txt", "user.small").as_deref(), Some(&b"x"[..]));
    assert_eq!(value("small.txt", "user.empty").as_deref(), Some(&b""[..]));
    assert_eq!(value("small.txt", "user.nope"), None);
    assert_eq!(value("small.txt", "trusted.small"), None);
    // In a leaf, and remote.
    assert_eq!(value("big.txt", "user.big"), Some(pattern(900, 1)));
    assert_eq!(value("remote.txt", "user.remote"), Some(pattern(10_000, 2)));
    let leaf: Vec<String> = (0..16).map(|i| format!("user.a{i:02}")).collect();
    assert_eq!(names("leaf.txt"), leaf);
    for i in [0, 7, 15] {
        let name = format!("user.a{i:02}");
        assert_eq!(value("leaf.txt", &name), Some(pattern(30, i)), "{name}");
    }
    // Nodes over leaves, and those mapped by a B+ tree.
    for (path, prefix, count) in [("node.txt", 'n', 400), ("btree.txt", 'b', 1200)] {
        let pad = prefix.to_string().repeat(40);
        let mut expected: Vec<String> = (0..count)
            .map(|i| format!("user.{prefix}{i:03}{pad}"))
            .collect();
        expected.sort();
        assert_eq!(names(path), expected, "{path}");
        for i in [0, count / 2, count - 1] {
            let name = format!("user.{prefix}{i:03}{pad}");
            assert_eq!(value(path, &name), Some(pattern(20, i)), "{path}: {name}");
        }
    }
    assert_eq!(
        value("trusted.txt", "trusted.secret").as_deref(),
        Some(&b"trusted"[..])
    );
    assert_eq!(
        value("security.txt", "security.label").as_deref(),
        Some(&b"label"[..])
    );
    // ACLs, converted, and as they are.
    assert_eq!(
        names("acl.txt"),
        ["system.posix_acl_access", "trusted.SGI_ACL_FILE"]
    );
    assert_eq!(value("acl.txt", "system.posix_acl_access"), Some(acl()));
    let raw = value("acl.txt", "trusted.SGI_ACL_FILE").unwrap();
    assert_eq!(raw[..4], 5u32.to_be_bytes());
    let dir = block_on(root.open_dir("acldir")).unwrap();
    assert_eq!(
        block_on(dir.xattr("system.posix_acl_default")).unwrap(),
        Some(acl())
    );
    let dir = block_on(root.open_dir("tagged")).unwrap();
    assert_eq!(
        block_on(dir.xattr("user.tag")).unwrap().as_deref(),
        Some(&b"dir"[..])
    );
    // A name that is not UTF-8, found by the entry listed.
    let file = block_on(root.open_file("foreign.txt")).unwrap();
    let listed = block_on(file.xattr_names()).unwrap();
    let [XattrEntry::Foreign { ns, name }] = &listed[..] else {
        panic!("{listed:?}");
    };
    assert_eq!((*ns, name.as_bytes()), (XattrNs::User, &b"caf\xE9"[..]));
    assert_eq!(
        block_on(file.xattr(listed[0].clone())).unwrap().as_deref(),
        Some(&b"foreign"[..])
    );
    // A short buffer gets the start, and the whole length.
    let file = block_on(root.open_file("remote.txt")).unwrap();
    let BufResult(len, buf) = block_on(file.xattr_into("user.remote", Vec::with_capacity(10)));
    assert_eq!(len.unwrap(), Some(10_000));
    assert_eq!(buf, pattern(10, 2));

    // Flags.
    let attrs = |path: &str| {
        block_on(root.metadata(path, MetaMask::ATTRS))
            .unwrap()
            .attrs
            .unwrap()
    };
    assert_eq!(attrs("plain.txt"), FileAttrs::empty());
    assert_eq!(attrs("immutable"), FileAttrs::IMMUTABLE);
    assert_eq!(attrs("append"), FileAttrs::APPEND_ONLY);
    assert_eq!(attrs("both"), FileAttrs::IMMUTABLE | FileAttrs::APPEND_ONLY);
    // No dump and no access time updates have no `FileAttrs`.
    assert_eq!(attrs("nodump"), FileAttrs::empty());
    assert_eq!(attrs("noatime"), FileAttrs::empty());
}

#[test]
fn reads_attributes_and_flags() {
    check_attributes(include_bytes!("fixtures/xfs-attrs.sparse"));
}

/// With 512-byte blocks, remote values span many.
#[test]
fn reads_attributes_in_small_blocks() {
    check_attributes(include_bytes!("fixtures/xfs-attrs-512.sparse"));
}

/// Leaves, nodes, and remote values' blocks, self-describing.
#[test]
fn reads_attributes_and_flags_v5() {
    check_attributes(include_bytes!("fixtures/xfs5-attrs.sparse"));
}

#[test]
fn reads_attributes_in_small_blocks_v5() {
    check_attributes(include_bytes!("fixtures/xfs5-attrs-1k.sparse"));
}

/// On an `ASCII_CI` volume, names are found in any case, as Linux finds
/// them, which `make.sh` checks: their bytes folded, Latin-1's letters
/// too, but not UTF-8's.
#[test]
fn reads_ascii_ci_volumes() {
    let root = open(include_bytes!("fixtures/xfs-ci.sparse"));
    let read = |path: &str| {
        let file = block_on(root.open_file(path))?;
        block_on(file.read_to_end())
    };
    for path in ["Hello.TXT", "hello.txt", "HELLO.TXT"] {
        assert_eq!(read(path).unwrap(), b"hello", "{path}");
    }
    assert_eq!(read("Ünïcödé").unwrap(), b"unicode");
    assert_eq!(read("ünïcödé").unwrap_err().kind(), ErrorKind::NotFound);
    // Listed as stored.
    let dir = block_on(root.open_dir("")).unwrap();
    let mut names = list(&dir, 100);
    names.sort();
    assert_eq!(names, ["Hello.TXT", "Large", "caf\\xC9", "Ünïcödé"]);
    // In a directory of nodes over leaves, by hash.
    let dir = block_on(root.open_dir("large")).unwrap();
    let names = list(&dir, 100);
    assert_eq!(names.len(), 2000);
    for name in &names {
        let path = format!("LARGE/{}", name.to_lowercase());
        assert_eq!(read(&path).unwrap(), b"hello", "{path}");
    }
    assert_eq!(
        read("large/file2000").unwrap_err().kind(),
        ErrorKind::NotFound
    );
}

// This is free and unencumbered software released into the public domain.

//! Volumes made by real tools: formatted by `mkfs.xfs` and filled through
//! the Linux kernel, or by `mkfs.xfs -p`, as `tests/fixtures/make.sh` does.

mod common;

use common::{Expected, MTIME, MTIME_NANOS, expected, pattern, unsparse};
use core::time::Duration;
use filer_api::{
    BufResult, Dir, Driver, EntryName, ErrorKind, FileAttrs, FileKind, MetaMask, Metadata, Op,
    Output, Resolve, Symlinks, Target, XattrEntry, XattrNs,
};
use filer_spec::block_on;
use filer_xfs::Xfs;
use std::collections::BTreeMap;

type Root = Dir<Xfs<Vec<u8>>>;

/// Opens the volume in a fixture.
fn open(sparse: &[u8]) -> Root {
    block_on(Xfs::open(unsparse(sparse))).unwrap()
}

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
        // Volumes without the `FTYPE` feature list no kinds.
        assert!(
            entry.kind.is_none() || entry.kind == metadata.kind,
            "{path}"
        );
        assert_eq!(metadata.id, entry.id, "{path}: id");
        let kind = metadata.kind.unwrap();
        if matches!(kind, FileKind::File | FileKind::Symlink) {
            check_time(&metadata, &path);
        }
        match kind {
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

/// Checks the modification time `make.sh` gives, in nanoseconds; version
/// 4 inodes record no creation time.
fn check_time(metadata: &Metadata, path: &str) {
    let modified = metadata.modified.unwrap();
    assert_eq!(modified.secs(), MTIME, "{path}");
    assert_eq!(
        modified.granularity(),
        Some(Duration::from_nanos(1)),
        "{path}"
    );
    assert_eq!(modified.nanos(), MTIME_NANOS, "{path}");
    assert_eq!(metadata.created, None, "{path}");
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

/// Checks a fixture `fill` wrote.
fn check(sparse: &[u8]) {
    let root = open(sparse);
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
    check(include_bytes!("fixtures/xfs-512.sparse"));
}

/// With 512-byte inodes, and no kinds in directory entries.
#[test]
fn reads_1k_blocks() {
    check(include_bytes!("fixtures/xfs-1k.sparse"));
}

/// With 1 KiB inodes and 8 KiB directory blocks.
#[test]
fn reads_2k_blocks() {
    check(include_bytes!("fixtures/xfs-2k.sparse"));
}

#[test]
fn reads_4k_blocks() {
    check(include_bytes!("fixtures/xfs-4k.sparse"));
}

/// With 2 KiB inodes and 64 KiB directory blocks.
#[test]
fn reads_large_inodes_and_directory_blocks() {
    check(include_bytes!("fixtures/xfs-4k-big.sparse"));
}

/// Filled by `mkfs.xfs -p`, as x86 Linux cannot mount version 4 volumes
/// of blocks larger than its pages; its files have no holes.
#[test]
fn reads_64k_blocks() {
    check(include_bytes!("fixtures/xfs-64k.sparse"));
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
    let root = open(include_bytes!("fixtures/xfs-dirs.sparse"));
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
        // Every entry is a link to `target`.
        for i in [0, count / 2, count - 1] {
            let path = format!("{form}/f{i:05}");
            let metadata = block_on(root.symlink_metadata(path.as_str(), MetaMask::all())).unwrap();
            assert_eq!(metadata.id, target, "{path}");
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
        assert_eq!(value("leaf.txt", &name), Some(pattern(10, i)), "{name}");
    }
    // Nodes over leaves, and those mapped by a B+ tree.
    for (path, prefix) in [("node.txt", 'n'), ("btree.txt", 'b')] {
        let pad = prefix.to_string().repeat(40);
        let expected: Vec<String> = (0..400)
            .map(|i| format!("user.{prefix}{i:03}{pad}"))
            .collect();
        assert_eq!(names(path), expected, "{path}");
        for i in [0, 199, 399] {
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

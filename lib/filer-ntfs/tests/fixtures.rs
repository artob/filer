// This is free and unencumbered software released into the public domain.

//! Volumes made by real tools: formatted by `mkntfs` and filled through
//! ntfs-3g, as `tests/fixtures/make.sh` does, in four layouts.

mod common;

use common::{Expected, MTIME, expected, expected_compressed, unsparse};
use filer_api::{BufResult, Dir, ErrorKind, FileAttrs, FileKind, MetaMask, Symlinks};
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

    // A named data stream is an attribute, matched as names are.
    let readme = block_on(root.open_file("README.TXT")).unwrap();
    let names = block_on(readme.xattr_names()).unwrap();
    assert_eq!(names, ["user.extra".try_into().unwrap()]);
    for name in ["user.extra", "user.EXTRA"] {
        let value = block_on(readme.xattr(name)).unwrap();
        assert_eq!(value.as_deref(), Some(&b"stream data"[..]), "{name}");
    }
    for name in ["user.nope", "trusted.extra"] {
        assert_eq!(block_on(readme.xattr(name)).unwrap(), None, "{name}");
    }
    let notes = block_on(root.open_file("notes.txt")).unwrap();
    assert_eq!(block_on(notes.xattr_names()).unwrap(), []);
    // Flags: ntfs-3g marks new files for archiving, and `mkntfs` the root
    // hidden and system, as Windows does.
    let attrs = |path: &str| {
        block_on(root.metadata(path, MetaMask::ATTRS))
            .unwrap()
            .attrs
            .unwrap()
    };
    assert_eq!(attrs("README.TXT"), FileAttrs::ARCHIVE);
    assert_eq!(attrs("sparse.bin"), FileAttrs::ARCHIVE | FileAttrs::SPARSE);
    assert_eq!(
        attrs(""),
        FileAttrs::HIDDEN | FileAttrs::SYSTEM | FileAttrs::ARCHIVE
    );
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
    for path in ["Compressed", "Compressed/text.txt", "Compressed/noise.bin"] {
        let metadata = block_on(root.metadata(path, MetaMask::ATTRS)).unwrap();
        let attrs = metadata.attrs.unwrap();
        assert!(attrs.contains(FileAttrs::COMPRESSED), "{path}: {attrs:?}");
    }
}

#[test]
fn reads_streams_and_attributes() {
    let root = block_on(Ntfs::open(unsparse(include_bytes!(
        "fixtures/ntfs-streams.sparse"
    ))))
    .unwrap();
    let file = block_on(root.open_file("streams.txt")).unwrap();
    assert_eq!(block_on(file.read_to_end()).unwrap(), b"main");
    let mut names: Vec<String> = block_on(file.xattr_names())
        .unwrap()
        .iter()
        .map(ToString::to_string)
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            "user.MixedCase",
            "user.big",
            "user.empty",
            "user.small",
            "user.Ünï ✓"
        ]
    );
    let value = |name: &str| block_on(file.xattr(name)).unwrap();
    assert_eq!(value("user.small").as_deref(), Some(&b"x"[..]));
    assert_eq!(value("user.big"), Some(common::pattern(60_000, 1)));
    assert_eq!(value("user.empty").as_deref(), Some(&b""[..]));
    assert_eq!(value("user.ÜNÏ ✓").as_deref(), Some(&b"unicode"[..]));
    assert_eq!(value("user.mixedcase").as_deref(), Some(&b"m"[..]));
    // A short buffer gets the start of a non-resident stream.
    let BufResult(len, buf) = block_on(file.xattr_into("user.big", Vec::with_capacity(10)));
    assert_eq!(len.unwrap(), Some(60_000));
    assert_eq!(buf, common::pattern(10, 1));

    // More streams than one record holds.
    let many = block_on(root.open_file("many.bin")).unwrap();
    assert_eq!(block_on(many.xattr_names()).unwrap().len(), 100);
    for i in [0, 37, 99] {
        let value = block_on(many.xattr(format!("user.s{i:03}").as_str())).unwrap();
        assert_eq!(value, Some(common::pattern(300, i)), "s{i:03}");
    }

    let dir = block_on(root.open_dir("Tagged")).unwrap();
    assert_eq!(
        block_on(dir.xattr("user.tag")).unwrap().as_deref(),
        Some(&b"dir"[..])
    );

    let attrs = |path: &str| {
        block_on(root.metadata(path, MetaMask::ATTRS))
            .unwrap()
            .attrs
            .unwrap()
    };
    assert_eq!(attrs("hidden.txt"), FileAttrs::HIDDEN);
    assert_eq!(attrs("system.txt"), FileAttrs::SYSTEM);
    assert_eq!(attrs("archive.txt"), FileAttrs::ARCHIVE);
    assert_eq!(attrs("offline.txt"), FileAttrs::OFFLINE);
    // Read-only is a permission, and not content indexed no flag.
    assert_eq!(attrs("readonly.txt"), FileAttrs::empty());
    assert_eq!(
        attrs("all.txt"),
        FileAttrs::HIDDEN | FileAttrs::SYSTEM | FileAttrs::ARCHIVE | FileAttrs::OFFLINE
    );
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
    // Interix symlinks.
    assert_eq!(target("ilink"), b"target.txt");
    assert_eq!(target("idirlink"), b"Sub");

    // Listings and stats agree on each kind, but for system files, which
    // only their contents tell from Interix special files.
    let dir = block_on(root.open_dir("")).unwrap();
    let mut entries = dir.entries().unwrap();
    while let Some(entry) = block_on(entries.next()) {
        let entry = entry.unwrap();
        let name = entry.name.as_name().unwrap().as_str().to_owned();
        let metadata = block_on(root.symlink_metadata(name.as_str(), MetaMask::all())).unwrap();
        let system = metadata.attrs.unwrap().contains(FileAttrs::SYSTEM);
        if system {
            assert_eq!(entry.kind, None, "{name}");
        } else {
            assert_eq!(entry.kind, metadata.kind, "{name}");
        }
        let link = [
            "link", "deep", "dirlink", "wsl", "dangling", "abs", "junction", "ilink", "idirlink",
        ];
        let expected = if link.contains(&name.as_str()) {
            FileKind::Symlink
        } else {
            match name.as_str() {
                "Sub" => FileKind::Dir,
                "fifo" => FileKind::Fifo,
                "sock" => FileKind::Socket,
                "ichr" => FileKind::CharDevice,
                "iblk" => FileKind::BlockDevice,
                _ => FileKind::File,
            }
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
    assert_eq!(read(&root, "ilink").unwrap(), b"target");
    assert_eq!(read(&root, "idirlink/nested.txt").unwrap(), b"nested");
    // Other system files are regular files.
    assert_eq!(
        read(&root, "system.bin").unwrap(),
        b"IntxFOO\0, a plain system file"
    );
    // Special files are not regular files, and what a symlink is, with
    // its target's length.
    for special in ["fifo", "sock", "ichr", "iblk"] {
        let error = read(&root, special).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::NotRegularFile, "{special}");
    }
    let lens = |path: &str| {
        block_on(root.symlink_metadata(path, MetaMask::LEN))
            .unwrap()
            .len
    };
    assert_eq!(
        ["ilink", "fifo", "sock", "ichr"].map(lens),
        [Some(10), Some(0), Some(1), Some(24)]
    );
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

// This is free and unencumbered software released into the public domain.

//! Volumes of other projects, which `tests/external.sh` fetches, checks
//! by their SHA-256 sums, and unpacks into `$FILER_NTFS_EXTERNAL`, as they
//! are not kept here; ignored unless run so. Each is walked as the fuzzer
//! walks its inputs, and the artifacts their makers describe are checked:
//! times that differ between a file's record and its index entry, an index
//! entry left in an MFT record's slack, slack filled with a pattern, and
//! clusters of 2 MiB.

mod common;

use common::exercise;
use filer_api::{FileAttrs, FileKind, MetaMask};
use filer_ntfs::Ntfs;
use filer_spec::block_on;
use filer_system::{Dir, ReadAt, ambient_authority};
use std::path::PathBuf;

/// Every volume, by its file's name in `volumes`: those of ntfs-samples,
/// each the NTFS partition of a disk Windows 10 formatted.
const VOLUMES: &[&str] = &[
    // Clusters of 2 MiB, the largest NTFS has.
    "ntfs-2m.vol",
    "ntfs-lastaccess.vol",
    // Filled with `PTRN` before formatting.
    "ntfs-ptrn.vol",
    // Compressed, and filled with `PTRN` before formatting.
    "ntfs-ramslack.vol",
    "ntfs-si-vs-fn.vol",
    // With shadow copies, which are not the volume's files.
    "ntfs.vol",
];

/// Returns where `tests/external.sh` put the volumes.
fn volumes() -> PathBuf {
    let dir = std::env::var_os("FILER_NTFS_EXTERNAL")
        .expect("FILER_NTFS_EXTERNAL: run tests/external.sh");
    PathBuf::from(dir).join("volumes")
}

/// Opens volume `name`, as a file of the native driver.
fn source(name: &str) -> impl ReadAt + 'static {
    let dir = filer_system::open_ambient(volumes(), ambient_authority()).unwrap();
    block_on(dir.open_file(name)).unwrap()
}

/// Opens volume `name`'s root.
fn open(name: &str) -> Dir<Ntfs<impl ReadAt + 'static>> {
    block_on(Ntfs::open(source(name))).unwrap()
}

/// Returns when the file at `path` of `root` was last accessed, in seconds.
fn accessed<S: ReadAt + 'static>(root: &Dir<Ntfs<S>>, path: &str) -> i64 {
    let metadata = block_on(root.metadata(path, MetaMask::all())).unwrap();
    metadata.accessed.unwrap().secs()
}

/// Returns the names `dir` of `root` lists.
fn names<S: ReadAt + 'static>(root: &Dir<Ntfs<S>>, dir: &str) -> Vec<String> {
    let handle = block_on(root.open_dir(dir)).unwrap();
    let mut entries = handle.entries().unwrap();
    let mut names = Vec::new();
    while let Some(entry) = block_on(entries.next()) {
        names.push(entry.unwrap().name.to_string());
    }
    names
}

/// Returns the paths and contents of the files under `dir` of `root`.
fn files<S: ReadAt + 'static>(root: &Dir<Ntfs<S>>, dir: &str) -> Vec<(String, Vec<u8>)> {
    let mut found = Vec::new();
    for name in names(root, dir) {
        let path = if dir.is_empty() {
            name
        } else {
            format!("{dir}/{name}")
        };
        let metadata = block_on(root.metadata(path.as_str(), MetaMask::all())).unwrap();
        match metadata.kind {
            Some(FileKind::Dir) => found.extend(files(root, &path)),
            Some(FileKind::File) => {
                let file = block_on(root.open_file(path.as_str())).unwrap();
                found.push((path, block_on(file.read_to_end()).unwrap()));
            },
            _ => {},
        }
    }
    found
}

#[test]
#[ignore = "needs tests/external.sh"]
fn walks_other_projects_volumes() {
    for name in VOLUMES {
        let outcome = exercise(source(name));
        assert!(outcome.opened, "{name}");
        assert_eq!(outcome.errors, 0, "{name}: {outcome:?}");
    }
}

/// Times are those of a file's record, `$STANDARD_INFORMATION`, as
/// Windows shows them, not those its directory's index keeps.
#[test]
#[ignore = "needs tests/external.sh"]
fn reads_times_from_records() {
    // 2019-03-03 12:37:55 UTC, where the index has 12:35:17.
    assert_eq!(
        accessed(&open("ntfs-lastaccess.vol"), "test/1.txt"),
        1_551_616_675
    );
    // 2015-11-03, where `$FILE_NAME` has 2014-12-12 and the index
    // 2016-09-24.
    let root = open("ntfs-si-vs-fn.vol");
    let metadata = block_on(root.metadata("test/test.txt", MetaMask::all())).unwrap();
    for time in [metadata.modified, metadata.accessed, metadata.created] {
        assert_eq!(time.unwrap().secs(), 1_446_554_039);
    }
    // 2020-07-25 12:49:21 and 12:35:24 UTC, where the files' records,
    // outside the `$FILE_NAME` attributes, keep other times too.
    let root = open("ntfs.vol");
    assert_eq!(accessed(&root, "test_dir_2/file_2_1.txt"), 1_595_681_361);
    assert_eq!(accessed(&root, "test_dir/file_1.txt"), 1_595_680_524);
}

/// An index entry left in the slack of the MFT is no entry.
#[test]
#[ignore = "needs tests/external.sh"]
fn lists_no_entries_from_slack() {
    let names = names(&open("ntfs.vol"), "test_dir");
    assert_eq!(names.len(), 12, "{names:?}");
    assert!(!names.iter().any(|name| name == "file_hidden.txt"));
}

/// No file reads past its end into slack, which holds the pattern the
/// disks were filled with before they were formatted.
#[test]
#[ignore = "needs tests/external.sh"]
fn reads_no_slack() {
    for name in ["ntfs-lastaccess.vol", "ntfs-ptrn.vol", "ntfs-ramslack.vol"] {
        let found = files(&open(name), "");
        assert!(found.len() >= 3, "{name}: {}", found.len());
        for (path, contents) in found {
            assert!(
                !contents.windows(4).any(|window| window == b"PTRN"),
                "{name}: {path}"
            );
        }
    }
    let root = open("ntfs-ramslack.vol");
    let metadata = block_on(root.metadata("1.txt", MetaMask::all())).unwrap();
    assert!(metadata.attrs.unwrap().contains(FileAttrs::COMPRESSED));
    assert_eq!(metadata.len, Some(6460));
}

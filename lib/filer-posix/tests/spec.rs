// This is free and unencumbered software released into the public domain.

//! The Filer specification, run against the POSIX driver in temporary
//! directories.

#![cfg(all(unix, feature = "std"))]

use filer_api::ambient_authority;
use filer_posix::Posix;
use filer_spec::{Profile, TreeEntry};
use std::{
    ffi::OsStr,
    fs,
    os::unix::{ffi::OsStrExt, fs::symlink},
    path::PathBuf,
    process::Command,
};

#[test]
fn passes_the_specification() {
    // Linux file systems accept any bytes in names; APFS rejects invalid
    // UTF-8.
    let profile = Profile::default().with_foreign_names(cfg!(target_os = "linux"));
    let mut roots: Vec<PathBuf> = Vec::new();
    let report = filer_spec::run_blocking(profile, |tree| {
        let root = std::env::temp_dir().join(format!(
            "filer-posix-spec-{}-{}",
            std::process::id(),
            roots.len()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir(&root).unwrap();
        for entry in tree.entries() {
            match entry {
                TreeEntry::Dir(path) => fs::create_dir(root.join(path.as_str())).unwrap(),
                TreeEntry::File(path, contents) => {
                    fs::write(root.join(path.as_str()), contents).unwrap();
                },
                TreeEntry::Symlink(path, target) => {
                    symlink(target, root.join(path.as_str())).unwrap();
                },
                TreeEntry::Fifo(path) => {
                    let status = Command::new("mkfifo")
                        .arg(root.join(path.as_str()))
                        .status()
                        .unwrap();
                    assert!(status.success());
                },
                TreeEntry::ForeignFile {
                    dir,
                    name,
                    contents,
                } => {
                    let path = root.join(dir.as_str()).join(OsStr::from_bytes(name));
                    fs::write(path, contents).unwrap();
                },
                entry => panic!("unknown entry: {entry:?}"),
            }
        }
        let dir = Posix::blocking()
            .open_ambient(&root, ambient_authority())
            .unwrap();
        roots.push(root);
        dir
    });
    for root in roots {
        let _ = fs::remove_dir_all(root);
    }
    println!("{report}");
    report.assert_passed();
}

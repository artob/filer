// This is free and unencumbered software released into the public domain.

//! The Filer specification, run against the Windows driver in temporary
//! directories.

#![cfg(all(windows, feature = "std"))]

use filer_api::ambient_authority;
use filer_spec::{Profile, TreeEntry};
use filer_windows::Windows;
use std::{
    fs,
    os::windows::fs::symlink_file,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

/// Runs the specification against drivers from `driver`, in temporary
/// directories.
fn run(driver: fn() -> Windows) -> filer_spec::Report {
    // Windows has no FIFOs in the file system, nor extended attributes as
    // Filer reads them; foreign names here are WTF-8 (SPEC-06).
    let profile = Profile::default()
        .with_fifos(false)
        .with_foreign_names(false)
        .with_xattrs(false);
    let mut roots: Vec<PathBuf> = Vec::new();
    let report = filer_spec::run_blocking(profile, |tree| {
        // Tests run in parallel, so each tree gets its own number.
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "filer-windows-spec-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir(&root).unwrap();
        for entry in tree.entries() {
            match entry {
                TreeEntry::Dir(path) => fs::create_dir(root.join(path.as_str())).unwrap(),
                TreeEntry::File(path, contents) => {
                    fs::write(root.join(path.as_str()), contents).unwrap();
                },
                // Every link in the tree is to a file, or to nothing.
                TreeEntry::Symlink(path, target) => {
                    symlink_file(target, root.join(path.as_str())).unwrap();
                },
                entry => panic!("unknown entry: {entry:?}"),
            }
        }
        let dir = driver().open_ambient(&root, ambient_authority()).unwrap();
        roots.push(root);
        dir
    });
    for root in roots {
        let _ = fs::remove_dir_all(root);
    }
    report
}

#[test]
fn passes_the_specification() {
    let report = run(Windows::blocking);
    println!("{report}");
    report.assert_passed();
}

#[test]
fn passes_the_specification_on_a_thread_pool() {
    run(|| Windows::thread_pool(filer_api::DefaultPool::new())).assert_passed();
}

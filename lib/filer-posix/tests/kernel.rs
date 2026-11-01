// This is free and unencumbered software released into the public domain.

//! Kernel-confined resolution with `openat2`, on Linux.

#![cfg(all(target_os = "linux", feature = "std"))]

use filer_api::{ErrorKind, ambient_authority};
use filer_posix::Posix;
use filer_spec::block_on;
use std::{fs, os::unix::fs::symlink};

#[test]
fn confines_with_openat2_and_walks_for_precise_errors() {
    let root = std::env::temp_dir().join(format!("filer-posix-kernel-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("a")).unwrap();
    fs::write(root.join("a/file"), "inside").unwrap();
    symlink("file", root.join("a/link")).unwrap();
    symlink("../../outside", root.join("a/escape")).unwrap();

    let kernel = Posix::blocking()
        .open_ambient(&root, ambient_authority())
        .unwrap();
    let file = block_on(kernel.open_file("a/link")).unwrap();
    assert_eq!(block_on(file.read_to_end()).unwrap(), b"inside");
    // `openat2` refuses the escape with `EXDEV`, which could also mean a
    // mount crossing; the userspace walk names it precisely.
    let error = block_on(kernel.open_file("a/escape")).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::EscapeBlocked);
    let fallback = error.fallback().expect("openat2 was tried first");
    assert_eq!(fallback.to_string(), "openat2 -> userspace walk");
    // Errors the kernel reports precisely involve no fallback.
    let error = block_on(kernel.open_file("a/nope")).unwrap_err();
    assert_eq!(
        (error.kind(), error.fallback()),
        (ErrorKind::NotFound, None)
    );

    let walk = Posix::blocking()
        .with_userspace_resolution()
        .open_ambient(&root, ambient_authority())
        .unwrap();
    let error = block_on(walk.open_file("a/escape")).unwrap_err();
    assert_eq!(
        (error.kind(), error.fallback()),
        (ErrorKind::EscapeBlocked, None)
    );
    fs::remove_dir_all(&root).unwrap();
}

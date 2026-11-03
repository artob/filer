// This is free and unencumbered software released into the public domain.

//! Read-only Minix file system driver for the Filer API.
//!
//! This crate reserves the name for a driver over Minix file systems,
//! versions 1, 2, and 3, read from any [`filer_api::ReadAt`] source, and
//! does not export any items yet. Writing is out of scope for now.
//!
//! What a reader must handle: the superblock at 1 KiB, whose magic number
//! gives the version and the length of names (14 or 30 bytes in versions 1
//! and 2, 60 in version 3); the inode and zone bitmaps; the inode table, of
//! 32-byte inodes with 16-bit zone numbers in version 1 and of 64-byte ones
//! with 32-bit numbers in versions 2 and 3; seven direct zones, then an
//! indirect and a doubly indirect one, and a triply indirect one in
//! versions 2 and 3; version 3's block sizes beyond 1 KiB; directories of
//! fixed-size entries; and the superblock's state, refusing a volume not
//! cleanly unmounted unless asked. The smallest format here, it makes a
//! fair first driver.
//!
//! `LEGAL.md` finds the format clear: a 1987 design with no patents, whose
//! sources (the textbook, MINIX 3, Linux) are read for facts, never copied.
//!
//! **Development.** Fixtures are made with `mkfs.minix`, in `util-linux`;
//! the differential test compares with the Linux kernel's `minix` driver.
//!
//! Volumes are untrusted input: like `filer-fat`, the driver will yield
//! errors, never a panic, a hang, or unbounded memory use, and every codec
//! and checksum will be pure Rust. It needs no OS and builds with
//! `#![no_std]`; the `std` feature, enabled by default, forwards to
//! `filer-api` and `filer-fs`. `all` currently enables nothing.

#![no_std]
#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
// No input may make this crate panic (DESIGN.md D47).
#![deny(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::let_underscore_must_use,
    clippy::panic,
    clippy::string_slice,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable,
    clippy::unwrap_used,
    unused_must_use
)]
// Tests may panic: that is how they fail.
#![cfg_attr(
    test,
    allow(
        clippy::arithmetic_side_effects,
        clippy::as_conversions,
        clippy::expect_used,
        clippy::indexing_slicing,
        clippy::let_underscore_must_use,
        clippy::panic,
        clippy::unreachable,
        clippy::unwrap_used
    )
)]

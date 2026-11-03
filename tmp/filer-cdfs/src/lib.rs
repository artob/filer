// This is free and unencumbered software released into the public domain.

//! Read-only ISO 9660 (CD-ROM) file system driver for the Filer API.
//!
//! This crate reserves the name for a driver over ISO 9660 (ECMA-119)
//! images, the CD-ROM file system that Windows calls CDFS, read from any
//! [`filer_api::ReadAt`] source, and does not export any items yet.
//!
//! **Read-only, permanently.** An ISO 9660 image is mastered whole, never
//! changed in place. This driver will never write one.
//!
//! What a reader must handle: the volume descriptor set from sector 16
//! (each with `CD001` at offset 1), up to its terminator; the primary
//! volume descriptor, its path table, and directory records, whose numbers
//! are stored both little- and big-endian; logical blocks of 2048 bytes;
//! files of several extents, beyond 4 GiB, and interleaved files; the base
//! standard's names (`NAME.EXT;1`, the version stripped); Joliet, a
//! supplementary volume descriptor with UCS-2 names; Rock Ridge, entries of
//! the System Use Sharing Protocol in each record, with POSIX names, modes,
//! links, symlinks, device numbers, and times, and relocated deep
//! directories; zisofs, transparently compressed files; and the enhanced
//! volume descriptor of ISO 9660:1999. The El Torito boot catalog is no
//! file system, and would be ignored. Most DVDs, and many hybrid images,
//! carry UDF too: that is `filer-udf`.
//!
//! **Development.** Fixtures are made with `xorriso`, in the package of
//! that name on Debian and Ubuntu; the differential test compares with the
//! Linux kernel's `isofs` driver.
//!
//! Images are untrusted input: like `filer-fat`, the driver will yield
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

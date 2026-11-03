// This is free and unencumbered software released into the public domain.

//! Read-only ReFS file system driver for the Filer API.
//!
//! This crate reserves the name for a driver over ReFS (Resilient File
//! System) volumes, the copy-on-write file system of Windows Server and of
//! Dev Drive, read from any [`filer_api::ReadAt`] source, and does not
//! export any items yet.
//!
//! **Read-only, permanently, and best effort.** Microsoft's ReFS patents
//! run into the 2030s, so this driver will never write a volume, and its
//! reader carries acknowledged patent exposure (`LEGAL.md`). Microsoft
//! publishes no specification: the format is known only from academic and
//! forensic analyses, and decompiling `refs.sys` is ruled out. The last
//! public structural map covers version 3.4, and later versions (to 3.14)
//! changed it again; versions 1 (Windows Server 2012) and 3 (Windows
//! Server 2016 and later) differ throughout, so a reader starts from the
//! boot sector, which names the file system (`ReFS`, at offset 3) and its
//! version. What it must handle in version 3: the superblock and its
//! checkpoints, the newest valid one the root; B+ trees, among them the
//! object table, which maps object ids to their trees, and one per
//! directory, whose rows hold its files' names and attributes; container
//! tables, which translate cluster numbers; clusters of 4 or 64 KiB;
//! checksums on metadata (CRC-32C or CRC-64), and on data in integrity
//! streams; and named streams and reparse points, as in `filer-ntfs`,
//! sharing its names and times through `filer-fs`.
//!
//! **Development.** The Linux kernel has no ReFS driver, so fixtures are
//! volumes Windows itself formats and fills (a Dev Drive, or
//! `Format-Volume -FileSystem ReFS` where the edition allows it), compared
//! with Windows' own manifest of each, as `filer-ntfs`'s `tests/windows.rs`
//! does.
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

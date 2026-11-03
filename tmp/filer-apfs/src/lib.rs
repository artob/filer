// This is free and unencumbered software released into the public domain.

//! Read-only APFS file system driver for the Filer API.
//!
//! This crate reserves the name for a driver over APFS (Apple File System)
//! containers, read from any [`filer_api::ReadAt`] source, and does not
//! export any items yet.
//!
//! **Read-only, permanently.** Apple's APFS patents run to about 2036, and
//! its reference grants no patent license, so this driver will never write
//! a volume (`LEGAL.md`).
//!
//! Apple documents the format in its *Apple File System Reference*. What a
//! reader must handle: the container superblock (`NXSB`) and the checkpoint
//! areas, the newest valid checkpoint the root; Fletcher-64 checksums on
//! every object; object maps, B-trees that map virtual object ids to
//! physical blocks; the superblocks (`APSB`) of a container's volumes, of
//! which a driver opens one; each volume's file system tree, of inode,
//! directory entry, extent, and extended attribute records, its names
//! hashed, and normalization-insensitive if the volume says so; files
//! compressed through the `com.apple.decmpfs` attribute, with zlib, LZVN,
//! or LZFSE; clones and snapshots; and the incompatible feature flags,
//! refusing any unknown one. Encrypted volumes would be
//! [`Unsupported`](filer_api::ErrorKind::Unsupported). The reference is
//! incomplete around sealed system volumes, Fusion drives, and some
//! compression methods, so those come last, if at all.
//!
//! **Development.** Fixtures are disk images made on macOS with `hdiutil
//! create -fs APFS`. The Linux kernel has no APFS driver, so the
//! differential test compares with macOS itself, mounting each image.
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

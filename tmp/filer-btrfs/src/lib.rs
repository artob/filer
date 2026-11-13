// This is free and unencumbered software released into the public domain.

//! Read-only btrfs file system driver for the Filer API.
//!
//! This crate reserves the name for a driver over btrfs volumes, read from
//! any [`filer_api::ReadAt`] source, and does not export any items yet.
//! Writing is out of scope for now.
//!
//! What a reader must handle: the superblock at 64 KiB, with its magic
//! `_BHRfS_M` and its copies at 64 MiB and 256 GiB; the chunk tree,
//! bootstrapped from the superblock's system chunks, which maps logical
//! addresses to devices; B-trees of nodes and of leaves holding items keyed
//! by object id, type, and offset: the root tree, a file system tree for
//! each subvolume and snapshot, and the extent and checksum trees;
//! checksums on every tree block and on data (CRC-32C, xxHash64, SHA-256,
//! or BLAKE2b); inode, directory item and index (names hashed with
//! CRC-32C), and extended attribute items; inline, regular, and
//! preallocated extents, compressed with zlib, LZO, or zstd; subvolumes
//! seen as directories; and the incompatible feature flags, refusing any
//! unknown one. RAID profiles span several devices; one source only covers
//! a volume of one.
//!
//! **Development.** Fixtures are made with `mkfs.btrfs --rootdir`, in the
//! `btrfs-progs` package on Debian and Ubuntu; the differential test
//! compares with the Linux kernel's `btrfs` driver.
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

// The driver is to read through `filer-fs` (D58); until it does, this
// keeps Cargo from warning that the dependency goes unused.
use filer_fs as _;

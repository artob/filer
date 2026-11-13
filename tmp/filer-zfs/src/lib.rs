// This is free and unencumbered software released into the public domain.

//! Read-only ZFS file system driver for the Filer API.
//!
//! This crate reserves the name for a driver over ZFS (OpenZFS) pools, read
//! from any [`filer_api::ReadAt`] source, and does not export any items
//! yet. Writing is out of scope for now.
//!
//! What a reader must handle: the four vdev labels, two at each end of a
//! device, each with the pool's configuration as an XDR-encoded list of
//! name-value pairs and a ring of uberblocks, the newest valid one the
//! root; block pointers, each with its checksum (Fletcher-4 or SHA-256,
//! among others), compression (LZ4, gzip, ZLE, LZJB, or zstd), and up to
//! three copies, plus gang and embedded blocks; object sets of dnodes; the
//! meta object set, with its datasets, snapshots, and clones; ZAP objects,
//! micro and fat, for directories and properties; the ZFS POSIX layer's
//! znodes and system attributes; mirror and RAID-Z layouts; and the pool's
//! feature flags, refusing any unknown one a reader must understand.
//! Natively encrypted datasets would be
//! [`Unsupported`](filer_api::ErrorKind::Unsupported). A pool may span
//! several devices; one source only covers a pool of one.
//!
//! **Development.** Fixtures are pools made on files with `zpool create`,
//! in the `zfsutils-linux` package on Ubuntu, whose kernel ships the ZFS
//! module; the differential test compares with OpenZFS itself. The driver
//! works from Sun's 2006 on-disk specification, OpenZFS's documentation,
//! and black-box output, never from OpenZFS's code (CDDL). Newer features
//! from companies that may hold patents, such as native encryption,
//! RAID-Z expansion, and fast dedup, need review first (`LEGAL.md`).
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

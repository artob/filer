// This is free and unencumbered software released into the public domain.

//! Read-only UFS (BSD FFS) file system driver for the Filer API.
//!
//! This crate reserves the name for a driver over UFS, the Unix File System
//! (BSD's Fast File System) of FreeBSD, NetBSD, OpenBSD, and Solaris, read
//! from any [`filer_api::ReadAt`] source, and does not export any items
//! yet. Writing is out of scope for now.
//!
//! What a reader must handle: UFS1 and UFS2, told apart by their
//! superblocks' magic numbers, the superblock searched for at its usual
//! offsets (64 KiB for UFS2, 8 KiB for UFS1) and in either byte order;
//! cylinder groups, with their inodes and maps of blocks; blocks split into
//! fragments, a file's last block perhaps a run of them; direct and
//! indirect blocks, 64-bit in UFS2; directories of variable-length records
//! in 512-byte chunks; short symlinks stored in the inode; UFS2's extended
//! attributes; and a journal of soft updates (SU+J) left needing recovery,
//! refused or replayed in memory. Each system's variant differs in details,
//! as the Linux kernel's `ufstype` mount option lists; FreeBSD's UFS2
//! first.
//!
//! `LEGAL.md` finds the format clear: Sun's UFS logging patents, the only
//! ones, expired in 2015, and the BSD code is read for facts, never copied.
//!
//! **Development.** Fixtures are made with `mkfs.ufs`, in the `ufsutils`
//! package on Debian and Ubuntu, or on FreeBSD itself; the differential
//! test compares with the Linux kernel's `ufs` driver.
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

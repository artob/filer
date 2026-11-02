// This is free and unencumbered software released into the public domain.

//! Read-only XFS file system driver for the Filer API.
//!
//! This crate will provide a driver over XFS volumes, read from any
//! [`filer_api::ReadAt`] source, reporting each inconsistency it finds as
//! an [`XfsError`]. It parses superblocks, inodes, and file maps so far. Writing is out of scope
//! for now (see `DESIGN.md` §17.16).
//!
//! **Clean room.** The mature implementations (the Linux kernel's `xfs`,
//! and xfsprogs' `libxfs`) are GPL, so this public-domain crate is written
//! from descriptions of the format only: the *XFS Algorithms & Data
//! Structures* book, and what `xfs_db` shows of real volumes. Contributors
//! must not consult any implementation's source while working on it
//! (`DESIGN.md` D52).
//!
//! What a reader must handle: the big-endian on-disk format; the
//! superblock and its feature flags, in `sb_versionnum` and
//! `sb_features2` (version 4) and in four sets of their own (version 5),
//! where a volume with an unknown incompatible feature must be refused;
//! allocation groups, whose geometry packs inode and block numbers;
//! inodes, in the v5 format with CRC32C metadata checksums, verified; data
//! forks as local, extent, or B+ tree formats; directories in their five
//! forms (short form, block, leaf, node, and B+ tree); symlinks local or
//! remote; extended attributes in the same forms; shared (reflinked)
//! extents, which a reader just follows; and a log that needs recovery,
//! which is either refused or replayed in memory, never read as if clean.
//!
//! **Development.** Fixtures are made with `mkfs.xfs` and inspected with
//! `xfs_db`, both in the `xfsprogs` package on Debian and Ubuntu; the
//! differential test compares with the Linux kernel's `xfs` driver, a
//! module in Ubuntu's standard `linux-modules-$(uname -r)` package.
//!
//! Volumes are untrusted input: like `filer-fat`, the driver will yield
//! errors, never a panic, a hang, or unbounded memory use, and every codec
//! and checksum will be pure Rust. It needs no OS and builds with
//! `#![no_std]`; the `std` feature, enabled by default, forwards to
//! `filer-api`. `all` currently enables nothing.

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
        clippy::expect_used,
        clippy::indexing_slicing,
        clippy::panic,
        clippy::unwrap_used
    )
)]

extern crate alloc;

mod xfs_error;
pub use xfs_error::*;

mod be;
mod file_map;
mod inode;
mod io;
mod superblock;

// This is free and unencumbered software released into the public domain.

//! Read-only ext2, ext3, and ext4 file system driver for the Filer API.
//!
//! This crate will provide a driver over ext2, ext3, and ext4 volumes,
//! read from any [`filer_api::ReadAt`] source, reporting each
//! inconsistency it finds as an [`Ext4Error`]. It parses superblocks so
//! far. Writing is out of scope for now (see `DESIGN.md` §17.15).
//!
//! **Clean room.** The mature implementations (the Linux kernel's `ext4`
//! and `jbd2`, e2fsprogs, GRUB) are GPL, so this public-domain crate is
//! written from descriptions of the format only: the kernel's prose
//! documentation (*ext4 Data Structures and Algorithms*) and Dave
//! Poirier's *The Second Extended File System*. Contributors must not
//! consult any implementation's source while working on it (`DESIGN.md`
//! D52).
//!
//! What a reader must handle: the superblock at byte 1024 and its
//! compatible, read-only-compatible, and incompatible feature flags (a
//! volume with an unknown incompatible feature must be refused, not
//! guessed at); block groups and their descriptors (32 or 64 bytes);
//! inodes mapping data by indirect blocks (ext2/3) or extent trees
//! (ext4), and inline data; linear and hashed (htree) directories, the
//! latter readable linearly; fast symlinks stored in the inode; devices,
//! FIFOs, and sockets as special files; nanosecond times with the extra
//! epoch bits; and `metadata_csum` (CRC32C) checksums, verified. A volume
//! whose journal needs recovery is either refused or replayed in memory,
//! never read as if clean. Case-folded and encrypted directories need
//! their own care.
//!
//! **Development.** Fixtures are made with `mke2fs` and inspected with
//! `debugfs`, both in the `e2fsprogs` package on Debian and Ubuntu; the
//! differential test compares with the Linux kernel's `ext4` driver, which
//! Ubuntu's kernel has built in.
//!
//! Volumes are untrusted input: like `filer-fat`, the driver will yield
//! errors, never a panic, a hang, or unbounded memory use, and every codec
//! and checksum will be pure Rust. It needs no OS and builds with
//! `#![no_std]`; the `std` feature, enabled by default, forwards to
//! `filer-api`. `all` currently enables nothing.

#![no_std]
#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
// The driver comes in a later step; its parts are tested meanwhile.
#![allow(dead_code)]
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

mod ext4_error;
pub use ext4_error::*;

mod block_map;
mod group;
mod inode;
mod io;
mod le;
mod superblock;

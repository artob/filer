// This is free and unencumbered software released into the public domain.

//! Read-only ext2, ext3, and ext4 file system driver for the Filer API.
//!
//! This crate reserves the name for a driver over ext2, ext3, and ext4
//! volumes, read from any [`filer_api::ReadAt`] source, and does not
//! export any items yet. Writing is out of scope for now.
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
//! their own care. See `DESIGN.md` §17.12.
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

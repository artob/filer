// This is free and unencumbered software released into the public domain.

//! Read-only XFS file system driver for the Filer API.
//!
//! This crate reserves the name for a driver over XFS volumes, read from
//! any [`filer_api::ReadAt`] source, and does not export any items yet.
//! Writing is out of scope for now.
//!
//! What a reader must handle: the big-endian on-disk format; the
//! superblock and its feature flags (a volume with an unknown incompatible
//! feature must be refused); allocation groups; inodes, in the v5 format
//! with CRC32C metadata checksums, verified; data forks as local, extent,
//! or B+ tree formats; directories in their five forms (short form,
//! block, leaf, node, and B+ tree); symlinks local or remote; shared
//! (reflinked) extents, which a reader just follows; and a log that needs
//! recovery, which is either refused or replayed in memory, never read as
//! if clean. See `DESIGN.md` §17.12.
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

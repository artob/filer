// This is free and unencumbered software released into the public domain.

//! Read-only SquashFS file system driver for the Filer API.
//!
//! This crate reserves the name for a driver over SquashFS images, the
//! compressed file system of live CDs, snaps, AppImages, and firmware, read
//! from any [`filer_api::ReadAt`] source, and does not export any items
//! yet.
//!
//! **Read-only, permanently.** SquashFS is read-only by design: an image is
//! built whole, by `mksquashfs`, and never changed in place. This driver
//! will never write one.
//!
//! What a reader must handle, in version 4.0: the superblock, with its
//! magic `hsqs`, its compressor, and its block size (4 KiB to 1 MiB);
//! metadata blocks of up to 8 KiB, each behind a two-byte header giving its
//! length and whether it is compressed; the inode, directory, fragment,
//! export, id, and extended attribute tables; basic and extended inodes of
//! every kind; directory headers and entries; data blocks, sparse blocks,
//! and tail ends packed into shared fragments; and the compressors gzip,
//! LZMA, LZO, XZ, LZ4, and zstd. LZO's reference implementation is GPL, so
//! its decoder must come from the algorithm's description or an MIT one
//! such as lzokay, never from minilzo (`LEGAL.md`). Images of earlier
//! versions, which the Linux kernel no longer reads either, would be
//! [`Unsupported`](filer_api::ErrorKind::Unsupported).
//!
//! **Development.** Fixtures are made with `mksquashfs`, in the
//! `squashfs-tools` package on Debian and Ubuntu, with each compressor; the
//! differential test compares with the Linux kernel's `squashfs` driver,
//! and with `unsquashfs -lln` for LZMA, which the Linux kernel lacks.
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

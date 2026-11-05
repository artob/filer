// This is free and unencumbered software released into the public domain.

//! Read-only SquashFS file system driver for the Filer API.
//!
//! [`Squashfs`] implements [`filer_api::Driver`] over SquashFS images, the
//! compressed file system of live CDs, snaps, AppImages, and firmware,
//! read from any [`filer_api::ReadAt`] source, reporting each
//! inconsistency it finds as a [`SquashfsError`]. It reads version 4.0,
//! as `mksquashfs` makes it and the Linux kernel reads it, with every
//! compressor `mksquashfs` offers, each a feature, all in `all`:
//!
//! | Compressor | Feature | Decoder |
//! | --- | --- | --- |
//! | gzip (zlib streams) | `gzip` | `decode-zlib` |
//! | LZMA | `lzma` | `decode-lzma` |
//! | LZO | `lzo` | `decode-lzo` |
//! | xz | `xz` | `decode-xz` |
//! | LZ4 | `lz4` | `decode-lz4` |
//! | zstd | `zstd` | `decode-zstd` |
//!
//! An image compressed otherwise, or by a compressor whose feature is
//! disabled, or of an earlier version, which the Linux kernel no longer
//! reads either, is [`Unsupported`](filer_api::ErrorKind::Unsupported).
//!
//! **Read-only, permanently.** SquashFS is read-only by design: an image is
//! built whole, by `mksquashfs`, and never changed in place. This driver
//! will never write one.
//!
//! **Clean room.** The mature implementations (the Linux kernel's
//! `squashfs`, squashfs-tools) are GPL, so this public-domain crate is
//! written from descriptions of the format only: the kernel's prose
//! documentation and the published description of the binary format,
//! read for facts, checked against images `mksquashfs` makes. Contributors
//! must not consult any implementation's source while working on it
//! (`DESIGN.md` D52).
//!
//! What a reader must handle: the superblock, with its magic `hsqs`, its
//! compressor, and its block size (4 KiB to 1 MiB); metadata blocks of up
//! to 8 KiB, each behind a two-byte header giving its length and whether
//! it is compressed; the inode, directory, fragment, and extended
//! attribute tables; basic and extended inodes of every kind; directory
//! headers and entries; data blocks, sparse blocks, and tail ends packed
//! into shared fragments; and extended attributes whose values are
//! shared. The export and id tables serve NFS and owners, which Filer
//! does not report, and are not read.
//!
//! **Development.** `tests/fixtures/make.sh` makes the fixtures with
//! `mksquashfs`, in the `squashfs-tools` package on Debian and Ubuntu, with
//! each compressor; `tests/differential.sh` compares with the Linux
//! kernel's `squashfs` driver, and with what `unsquashfs` extracts for
//! LZMA, which the Linux kernel lacks.
//!
//! Images are untrusted input: like `filer-fat`, the driver yields
//! errors, never a panic, a hang, or unbounded memory use, and every codec
//! is pure Rust, written clean room. It needs no OS and builds with
//! `#![no_std]`; the `std` feature, enabled by default, forwards to
//! `filer-api`, `filer-fs`, and the decoders.

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

mod codec;
mod dir;
mod file;
mod inode;
mod meta;

mod squashfs;
pub use squashfs::*;

mod squashfs_error;
pub use squashfs_error::*;

mod superblock;
mod volume;
mod xattr;

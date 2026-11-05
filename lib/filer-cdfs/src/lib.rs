// This is free and unencumbered software released into the public domain.

//! Read-only ISO 9660 (CD-ROM) file system driver for the Filer API.
//!
//! [`Cdfs`] implements [`filer_api::Driver`] over ISO 9660 (ECMA-119)
//! images, the CD-ROM file system that Windows calls CDFS, of installers,
//! live systems, and archived discs, read from any
//! [`filer_api::ReadAt`] source, reporting each inconsistency it finds as
//! a [`CdfsError`]. It reads the extensions images carry, as the Linux
//! kernel does:
//!
//! | Extension | What it adds |
//! | --- | --- |
//! | Rock Ridge (SUSP) | POSIX names, kinds, symlinks, times, and serial numbers; deep directories relocated |
//! | Joliet | UTF-16 names, of up to 64 characters, or 103 |
//! | ISO 9660:1999 | names of any bytes, up to 207 |
//! | zisofs (`zisofs` feature) | files compressed block by block, as zlib streams, over `decode-zlib` |
//!
//! Files of several extents, beyond 4 GiB, read as one. Interleaved files,
//! and Rock Ridge's sparse ones, which no current tool writes, are
//! [`Unsupported`](filer_api::ErrorKind::Unsupported), as is a file zisofs
//! compresses while the `zisofs` feature is off. The El Torito boot
//! catalog is no file system, and is ignored. Most DVDs, and many hybrid
//! images, carry UDF too: that is `filer-udf`.
//!
//! **Read-only, permanently.** An ISO 9660 image is mastered whole, never
//! changed in place. This driver will never write one.
//!
//! **Sources.** ECMA-119, the System Use Sharing Protocol and Rock Ridge
//! (IEEE P1281 and P1282, as drafted), Joliet, and zisofs's published
//! description are free, and unpatented (`LEGAL.md`); this public-domain
//! crate is written from them, checked against images `xorriso` makes.
//!
//! What a reader must handle: the volume descriptor set from sector 16
//! (each with `CD001` at offset 1), up to its terminator; the primary
//! volume descriptor, and supplementary ones, of Joliet's names or of
//! ISO 9660:1999's; directory records, whose numbers are stored both
//! little- and big-endian, which end in the sector they begin in; logical
//! blocks of 512 to 2048 bytes; extended attribute records before files'
//! data; the base standard's names (`NAME.EXT;1`, the version stripped);
//! System Use entries, continued in other blocks; and Rock Ridge's `NM`,
//! `PX`, `SL`, `TF`, `CL`, `RE`, `SF`, and `ZF` entries. The path tables
//! repeat what directories hold, and are not read.
//!
//! **Development.** `tests/fixtures/make.sh` makes the fixtures with
//! `xorriso` and `genisoimage`, in the packages of those names on Debian
//! and Ubuntu; `tests/differential.sh` compares with the Linux kernel's
//! `isofs` driver.
//!
//! Images are untrusted input: like `filer-fat`, the driver yields
//! errors, never a panic, a hang, or unbounded memory use, and every codec
//! is pure Rust, written clean room. It needs no OS and builds with
//! `#![no_std]`; the `std` feature, enabled by default, forwards to
//! `filer-api`, `filer-fs`, `filer-msft`, and `decode-zlib`.

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

extern crate alloc;

mod cdfs;
pub use cdfs::*;

mod cdfs_error;
pub use cdfs_error::*;

mod descriptor;
mod dir;
mod file;
mod node;
mod read;
mod record;
mod susp;
mod time;
mod volume;

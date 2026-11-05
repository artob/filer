// This is free and unencumbered software released into the public domain.

//! Read-only UDF file system driver for the Filer API.
//!
//! [`Udf`] implements [`filer_api::Driver`] over UDF (Universal Disk
//! Format: ECMA-167, as OSTA's UDF specifications profile it) volumes,
//! such as DVD and Blu-ray discs' images, and many hybrid images', read
//! from any [`filer_api::ReadAt`] source, reporting each inconsistency it
//! finds as a [`UdfError`]. It reads revisions 1.02 to 2.60, as the Linux
//! kernel does: physical, sparable, virtual (VAT), and metadata
//! partitions, of blocks of 512 bytes to 4 KiB. Many discs carry ISO 9660
//! too, as a bridge: that is `filer-cdfs`.
//!
//! **Sources.** ECMA-167 and OSTA's UDF 2.60 specification are free, and
//! any patents on them have expired (`LEGAL.md`); this public-domain
//! crate is written from them, checked against volumes `mkudffs` makes,
//! filled through the Linux kernel's `udf` driver.
//!
//! What a reader must handle: the anchor volume descriptor pointer, at
//! sector 256, or at the volume's end; the volume descriptor sequences,
//! main and reserve, and their partition and logical volume descriptors;
//! partition maps; the file set descriptor; file entries and extended
//! file entries, with data embedded, or in short, long, or extended
//! allocation descriptors, continued in allocation extent descriptors;
//! file identifier descriptors; names in OSTA's compressed Unicode; and
//! symlinks' path components. Descriptors' CRCs are not checked: their
//! tags' checksums, identifiers, and the bounds of every field are.
//!
//! **Development.** `tests/fixtures/make.sh` makes the fixtures with
//! `mkudffs`, in the `udftools` package on Debian and Ubuntu, filling
//! them through the Linux kernel's `udf` driver; `tests/differential.sh`
//! compares with that driver.
//!
//! Volumes are untrusted input: like `filer-fat`, the driver yields
//! errors, never a panic, a hang, or unbounded memory use. It needs no OS
//! and builds with `#![no_std]`; the `std` feature, enabled by default,
//! forwards to `filer-api`, `filer-fs`, and `filer-msft`.

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

mod udf;
pub use udf::*;

mod udf_error;
pub use udf_error::*;

mod dir;
mod entry;
mod file;
mod fs;
mod tag;
mod text;
mod volume;

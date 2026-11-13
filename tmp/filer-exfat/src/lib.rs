// This is free and unencumbered software released into the public domain.

//! Read-only exFAT file system driver for the Filer API.
//!
//! This crate reserves the name for a driver over exFAT volumes, such as SD
//! cards and USB drives, read from any [`filer_api::ReadAt`] source, and
//! does not export any items yet. Writing is out of scope for now.
//!
//! **Not to be released before mid-2028.** Microsoft's exFAT patents run
//! to 2028, and its grant through the Open Invention Network reaches
//! neither a public-domain library nor any system but Linux. A reader may
//! be developed, but must not be published before the last of them
//! expires (`LEGAL.md`). This crate only reserves the name.
//!
//! Unlike FAT's, the format is fully specified: Microsoft published it in
//! 2019. What a reader must handle: the main and backup boot regions, each
//! of 12 sectors: the boot sector (`EXFAT   ` at offset 3), the extended
//! boot sectors, the OEM parameters, and a sector of checksums over the
//! rest; the FAT and the cluster heap; the allocation bitmap; the up-case
//! table, by which the volume compares names without case; directory entry
//! sets of 32-byte entries, a file entry, a stream extension, and file name
//! entries, guarded by the set's checksum; files whose clusters are
//! contiguous (`NoFatChain`), whose FAT entries mean nothing; names of up
//! to 255 UTF-16 code units; and times in local time, in 10 ms steps, with
//! an offset from UTC.
//!
//! **Development.** Fixtures are made with `mkfs.exfat`, in the `exfatprogs`
//! package on Debian and Ubuntu; the differential test compares with the
//! Linux kernel's `exfat` driver. It shares little on disk with `filer-fat`
//! (D46), but both read FAT's local times from `filer-msft`. See
//! `DESIGN.md` §17.12.
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

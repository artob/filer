// This is free and unencumbered software released into the public domain.

//! Read-only NTFS file system driver for the Filer API.
//!
//! [`Ntfs`] implements [`filer_api::Driver`] over NTFS volumes, read from
//! any [`filer_api::ReadAt`] source, reporting each inconsistency it finds
//! as an [`NtfsError`]. Writing is out of scope for now (see `DESIGN.md`
//! §17.14).
//!
//! **Clean room.** The mature NTFS implementations are GPL, so this
//! public-domain crate is written from public descriptions of the format
//! only: Microsoft's open specifications [MS-FSCC] and [MS-XCA], and the
//! Linux-NTFS project's *NTFS Documentation*. Contributors must not
//! consult GPL implementations (ntfs-3g, the Linux kernel's `ntfs3` and
//! `ntfs`) while working on it (`DESIGN.md` D52).
//!
//! What a reader must handle: the boot sector and the master file table
//! (MFT), whose records carry update sequence arrays (fixups) to check
//! and apply; resident and non-resident attributes, the latter as data
//! runs; directories as B+ tree indexes (`$INDEX_ROOT` and
//! `$INDEX_ALLOCATION`); names in UTF-16, with case-insensitive matching
//! by the volume's own `$UpCase` table, and the DOS (8.3) namespace
//! alongside the Win32 one; hard links; sparse and LZNT1-compressed
//! data, decompressed in pure Rust; alternate data streams, read as
//! extended attributes, and file attributes, read as flags; reparse
//! points (symbolic links and junctions, mapped to symlinks where they
//! can be confined); and a volume marked dirty, whose `$LogFile` has not
//! been replayed.
//!
//! **Development.** `tests/fixtures/make.sh` makes the fixtures with
//! `mkntfs` and fills them through the FUSE driver `ntfs-3g`, both in the
//! `ntfs-3g` package on Debian and Ubuntu, and needs `python3` and `sudo`.
//! `tests/differential.sh` compares with the Linux kernel's `ntfs3`
//! driver, a module that Ubuntu ships in `linux-modules-extra-$(uname -r)`
//! (kept current by `linux-generic`), and, for streams and flags, with
//! `ntfs-3g`.
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

mod ntfs_error;
pub use ntfs_error::*;

mod file_name;
mod fixup;
mod geometry;
mod index;
mod lznt1;
mod mft;

mod ntfs;
pub use ntfs::*;

mod record;
mod reparse;
mod runs;
mod stream;
mod time;
mod upcase;
mod volume;
mod wtf8;

mod options;
pub use options::*;

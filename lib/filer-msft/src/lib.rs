// This is free and unencumbered software released into the public domain.

//! Conventions of Microsoft's on-disk formats for Filer API drivers.
//!
//! Microsoft's file systems and archive formats share conventions, and so
//! do the drivers that read them: file systems (FAT and NTFS, and later
//! exFAT, ReFS, and ISO 9660's Joliet names), archives (ZIP, 7z, and RAR),
//! and the Windows driver, which reads the same values from Windows. This
//! crate is where they share them (`DESIGN.md` §17.17, D60):
//!
//! - [`nt`] converts NT times, in 100 ns steps since 1601, and the file
//!   attributes that are flags;
//! - [`wtf8`] encodes UTF-16 names as WTF-8, unpaired surrogates and all,
//!   and back;
//! - [`reparse`] parses reparse points, and reads links' targets as Filer
//!   sees them;
//! - [`dos`] converts DOS dates and times, local and in 2-second steps,
//!   at a given [`UtcOffset`](dos::UtcOffset).
//!
//! It holds the conventions of Microsoft's formats and nothing of
//! Microsoft's: it is written from published facts (`LEGAL.md`), and is
//! neither from nor endorsed by Microsoft. "msft" is how others name the
//! company, by its ticker.
//!
//! Like the drivers, it treats volumes and archives as untrusted input:
//! nothing in it may panic, hang, or allocate without bound on any input,
//! which its root denies by lint. It needs no OS, builds with
//! `#![no_std]`, and is pure Rust; the `std` feature, enabled by default,
//! forwards to `filer-api`. `all` currently enables nothing.

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

pub mod dos;
pub mod nt;
pub mod reparse;
pub mod wtf8;

// This is free and unencumbered software released into the public domain.

//! Conventions of Microsoft's on-disk formats for Filer API drivers.
//!
//! Microsoft's file systems and archive formats share conventions: NT
//! times, in 100 ns steps since 1601; attribute flags; names in UTF-16;
//! reparse points; and FAT's local dates and times, in 2-second steps.
//! This crate is where the drivers reading them are to share those,
//! whether file systems (`filer-fat`, `filer-ntfs`, and later exFAT,
//! ReFS, and ISO 9660's Joliet names), archives (`filer-zip`, and later
//! 7z and RAR), or the Windows driver, and does not export any items yet.
//! What moves in: `nt`, `wtf8`, and `reparse` from `filer-fs`, and FAT's
//! local times from `filer-fat`, as `dos`. See `DESIGN.md` §17.17 and D60.
//!
//! It holds the conventions of Microsoft's formats and nothing of
//! Microsoft's: it is written from published facts (`LEGAL.md`), and is
//! neither from nor endorsed by Microsoft. "msft" is how others name the
//! company, by its ticker.
//!
//! Like the drivers, it will treat volumes and archives as untrusted
//! input: nothing in it may panic, hang, or allocate without bound on any
//! input, which its root denies by lint. It needs no OS and builds with
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
        clippy::as_conversions,
        clippy::expect_used,
        clippy::indexing_slicing,
        clippy::let_underscore_must_use,
        clippy::panic,
        clippy::unreachable,
        clippy::unwrap_used
    )
)]

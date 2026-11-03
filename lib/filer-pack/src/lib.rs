// This is free and unencumbered software released into the public domain.

//! Shared machinery of archive drivers for the Filer API.
//!
//! This crate is where the archive drivers (`filer-tar`, `filer-zip`,
//! `filer-ar`, `filer-cpio`, and those to come) are to share their code,
//! as the file system drivers do in `filer-fs`, and does not export any
//! items yet. What moves in, in order: all of [`filer_api::archive`], the
//! index and the driver every format feeds; the limit checks and the read
//! helper each parser repeats; one checksum-verifying decoder; a cache of
//! decoded streams, and a [`filer_api::ReadAt`] source over one, for
//! compressed tarballs and solid archives; and pure-Rust decoders, one
//! feature per algorithm. See `DESIGN.md` §17.7 and D59.
//!
//! **Read-only, permanently.** Filer reads archives as directories; it
//! never creates or modifies them, and never will: nothing here will
//! compress or write.
//!
//! Like the drivers, it will treat archives as untrusted input: nothing in
//! it may panic, hang, or allocate without bound on any input, which its
//! root denies by lint. It needs no OS and builds with `#![no_std]`; the
//! `std` feature, enabled by default, forwards to `filer-api`. `all`
//! currently enables nothing.

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

// This is free and unencumbered software released into the public domain.

//! TAR archive driver for the Filer API.
//!
//! [`Tar`] serves the contents of a TAR archive, read-only, as a
//! [`Dir`](filer_api::Dir): open the archive from any
//! [`ReadAt`](filer_api::ReadAt) source, such as a `File` from another
//! driver or bytes in memory, then use it like any other directory. Paths
//! stay confined to the archive, and a hostile archive fails to open rather
//! than exposing anything outside it (see [`Tar`] and [`Limits`]).
//!
//! **Read-only, permanently.** Filer reads archives as directories; it
//! never creates or modifies them, and never will: writing archives is out
//! of scope for this project. Use a dedicated archiving library for that.
//!
//! So far it reads uncompressed archives with random access. Compressed
//! archives (gzip, zstd, xz) and forward-only sources are planned (see
//! `DESIGN.md` §17.7).
//!
//! It needs no OS and builds with `#![no_std]`; the `std` feature, enabled
//! by default, forwards to `filer-api`. `all` currently enables nothing.

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

mod header;

mod index;

/// Re-exported from [`filer_pack`], where all archive drivers share it.
pub use filer_pack::Limits;

mod tar;
pub use tar::*;

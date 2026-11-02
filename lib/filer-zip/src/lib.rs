// This is free and unencumbered software released into the public domain.

//! ZIP archive driver for the Filer API.
//!
//! [`Zip`] serves the contents of a ZIP archive, read-only, as a
//! [`Dir`](filer_api::Dir): open the archive from any
//! [`ReadAt`](filer_api::ReadAt) source with random access, such as a
//! `File` from another driver or bytes in memory, then use it like any
//! other directory. Paths stay confined to the archive ("zip slip" cannot
//! happen), and a hostile archive fails to open rather than exposing
//! anything outside it (see [`Zip`] and [`Limits`]).
//!
//! **Read-only, permanently.** Filer reads archives as directories; it
//! never creates or modifies them, and never will: writing archives is out
//! of scope for this project. Use a dedicated archiving library for that.
//!
//! So far it reads stored and Deflate members, with limits against zip
//! bombs. More codecs behind features and legacy code-page names are
//! planned (see `DESIGN.md` §17.7).
//!
//! It needs no OS and builds with `#![no_std]`. Features, all enabled by
//! default:
//!
//! - `std` forwards to `filer-api`;
//! - `all` enables every codec below;
//! - `deflate` decodes Deflate members, with `miniz_oxide`.

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
        clippy::panic,
        clippy::unwrap_used
    )
)]

extern crate alloc;

mod codec;

mod header;

mod index;

/// Re-exported from [`filer_api::archive`], where all archive drivers share
/// it.
pub use filer_api::archive::Limits;

mod zip;
pub use zip::*;

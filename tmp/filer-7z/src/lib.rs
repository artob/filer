// This is free and unencumbered software released into the public domain.

//! Read-only 7z archive driver for the Filer API.
//!
//! This crate reserves the name for a driver over 7z archives, the format
//! of 7-Zip, and does not export any items yet.
//!
//! **Read-only, permanently.** Filer reads archives as directories; it
//! never creates or modifies them, and never will: writing archives is out
//! of scope for this project. Use a dedicated archiving library for that.
//!
//! Unlike RAR's, the format is open: 7-Zip's author documents it in the
//! LZMA SDK, which he placed in the public domain. The work is in the
//! codecs, which must be pure-Rust decoders: LZMA and LZMA2 above all,
//! then PPMd, BZip2, Deflate, and the BCJ and BCJ2 filters for
//! executables; AES-encrypted archives would be
//! [`Unsupported`](filer_api::ErrorKind::Unsupported). The index lies at
//! the archive's end, usually compressed itself, so opening one already
//! takes a decoder. Archives are also usually "solid" (members compressed
//! as one stream), which makes reading a member at an offset costly. See
//! `DESIGN.md` §17.7.
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

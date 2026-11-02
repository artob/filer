// This is free and unencumbered software released into the public domain.

//! Read-only cpio archive driver for the Filer API.
//!
//! This crate will implement [`filer_api::Driver`] over cpio archives: Linux
//! initramfs images and the payload of RPM packages. It does not export any
//! items yet.
//!
//! **Read-only, permanently.** Filer reads archives as directories; it
//! never creates or modifies them, and never will: writing archives is out
//! of scope for this project. Use a dedicated archiving library for that.
//!
//! The plan (see `DESIGN.md` §17.7): like TAR, cpio is a sequence of headers
//! each followed by its member's data, so one pass over a
//! [`filer_api::ReadAt`] source builds the index. The portable (`odc`),
//! new ASCII (`newc`, with or without checksums), and old binary formats
//! will be read; hardlinks, which cpio identifies by device and inode number
//! rather than by path, become one shared node. The shared archive rules
//! apply: names confined to the root, later duplicates winning, and strict
//! limits.
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

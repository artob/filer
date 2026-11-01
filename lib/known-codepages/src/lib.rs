// This is free and unencumbered software released into the public domain.

//! Single-byte DOS and Windows code pages, decoded by table.
//!
//! [`CodePage`] decodes bytes of the code pages that DOS and Windows
//! consoles used, which formats of that era store names in: FAT short
//! names, and ZIP entry names without the UTF-8 flag. Code page 437 is
//! always available; every other page is behind a feature of its number,
//! such as `cp850`, all of them enabled by `all`, which is a default
//! feature. The tables are generated from Python's codecs by
//! `scripts/tables.py`.
//!
//! The crate has no dependencies, needs no allocator, builds with
//! `#![no_std]`, and never panics; the `std` feature, enabled by default,
//! currently adds nothing.

#![no_std]
#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
// No input may make this crate panic.
#![deny(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable,
    clippy::unwrap_used
)]

mod code_page;
pub use code_page::*;

#[rustfmt::skip]
mod tables;

// This is free and unencumbered software released into the public domain.

//! FAT12/16/32 file system driver for the Filer API.
//!
//! [`Fat`] implements [`filer_api::Driver`] over FAT volumes, read from any
//! [`filer_api::ReadAt`] source: a disk image `File` of another driver,
//! bytes in memory, or a block device. It is read-only for now, and
//! designed so that writing can follow (see `DESIGN.md` §17.13).
//!
//! Volumes are untrusted input. A corrupt or hostile image yields errors,
//! never a panic, a hang, or unbounded memory use: the crate denies
//! `unwrap`, `expect`, `panic!`, unchecked indexing, and unchecked
//! arithmetic outright, and every inconsistency it finds is reported as a
//! [`FatError`].
//!
//! It needs no OS, builds with `#![no_std]`, and is pure Rust; the `std`
//! feature, enabled by default, forwards to `filer-api`. `all` currently
//! enables nothing. The `code-pages` feature, off by default, makes every
//! [`CodePage`] of `known-codepages` available for short names, beyond
//! code page 437. exFAT is a different format, in `filer-exfat`.

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

mod fat_error;
pub use fat_error::*;

mod fat_type;
pub use fat_type::*;

mod chain;

pub use known_codepages::CodePage;

mod dir_entry;

mod fat;
pub use fat::*;

mod fat_time;
mod geometry;

mod options;
pub use options::*;

mod utc_offset;
pub use utc_offset::*;

mod volume;

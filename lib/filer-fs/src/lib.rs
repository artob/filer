// This is free and unencumbered software released into the public domain.

//! Shared machinery of on-disk file system drivers for the Filer API.
//!
//! The file system drivers (`filer-fat`, `filer-ext4`, `filer-ntfs`,
//! `filer-udf`, `filer-xfs`, and those to come) differ in their on-disk
//! formats, but not in much else: each reads a volume from a
//! [`filer_api::ReadAt`] source, decodes fields of untrusted structures,
//! maps file blocks to volume blocks, turns names and kinds into Filer's,
//! and serves the result as a [`filer_api::Driver`]. This crate is where
//! what they share lives, so that each driver is its format and little
//! else (`DESIGN.md` §17.17, D54).
//!
//! So far: [`le`] and [`be`] read little- and big-endian fields, and
//! [`read_exact`] reads them from a source; [`wtf8`] encodes UTF-16
//! names; [`unix`] decodes Unix file types, and [`posix_acl`] encodes
//! POSIX ACLs as Linux returns them; [`map`] reads files through maps of
//! their blocks. A format implements [`FileSystem`], and [`Mounted`]
//! serves it as a driver.
//!
//! Like the drivers, it treats volumes as untrusted input: nothing in it
//! may panic, hang, or allocate without bound on any input, which its
//! root denies by lint. It needs no OS, builds with `#![no_std]`, and is
//! pure Rust; the `std` feature, enabled by default, forwards to
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

pub mod be;
pub mod le;

mod read;
pub use read::*;

pub mod map;

mod mount;
pub use mount::*;
pub mod posix_acl;
pub mod unix;
pub mod wtf8;

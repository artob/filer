// This is free and unencumbered software released into the public domain.

//! Shared machinery of archive drivers for the Filer API.
//!
//! Archive formats differ in their headers, but once indexed they are all
//! the same thing: a [`Tree`](filer_api::tree::Tree) of directories,
//! links, and members whose bytes lie at known offsets in a [`ReadAt`]
//! source. This crate holds everything but the header parsing, so that
//! each format crate (`filer-tar`, `filer-zip`, `filer-ar`, `filer-cpio`,
//! ...) is just a parser feeding an [`Index`], as the file system drivers
//! share `filer-fs` (`DESIGN.md` §17.7, D59):
//!
//! - [`Index`] builds the tree under the shared rules: member names are
//!   checked so that none can escape the archive root, directories implied
//!   by names are created, the later of duplicate members wins, hardlinks
//!   share one node, and members, names, and link targets are counted and
//!   measured against the [`Limits`];
//! - [`Archive`] serves the finished tree as a read-only [`Driver`];
//! - [`Codec`] and [`Decoder`] decode compressed members, which
//!   [`Member`] records with their format's codec, and [`Verified`]
//!   checks a [`Digest`] of what a decoder produced, such as a CRC-32;
//! - [`Streams`] keeps [`Stream`]s part way through what they decode,
//!   so that reading on from where a read stopped decodes no more, and
//!   [`Unpacked`] reads a packed range of a source decoded, as a source
//!   in turn: a compressed tarball, or a solid block;
//! - [`Limits`] bounds what an untrusted archive may make the index hold;
//! - [`Parser`] reads a format's headers, and its errors carry an
//!   [`ArchiveError`] naming the format, header offset, and reason.
//!
//! Decoders are pure Rust, one feature per algorithm, assessed in
//! `LEGAL.md` before adoption (D45): so far, [`Deflate`], over
//! `miniz_oxide`, behind `deflate`.
//!
//! **Read-only, permanently.** Filer reads archives as directories; it
//! never creates or modifies them, and never will: nothing here will
//! compress or write.
//!
//! Like the drivers, it treats archives as untrusted input: nothing in it
//! may panic, hang, or allocate without bound on any input, which its
//! root denies by lint. It needs no OS and builds with `#![no_std]`,
//! locking with a spin lock there. Features, all enabled by default:
//!
//! - `std` forwards to `filer-api`, and locks with the standard mutex;
//! - `all` enables every decoder below;
//! - `deflate` enables [`Deflate`].
//!
//! [`ReadAt`]: filer_api::ReadAt
//! [`Driver`]: filer_api::Driver

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

#[cfg(feature = "std")]
extern crate std;

mod archive_error;
pub use archive_error::*;

mod codec;
pub use codec::*;

mod driver;
pub use driver::*;

mod index;
pub use index::*;

mod lock;

mod limits;
pub use limits::*;

mod member;
pub use member::*;

mod parser;
pub use parser::*;

mod streams;
pub use streams::{Stream, StreamId, Streams};

mod verified;
pub use verified::*;

#[cfg(feature = "deflate")]
mod deflate;
#[cfg(feature = "deflate")]
pub use deflate::*;

#[cfg(test)]
mod toy;

mod unpacked;
pub use unpacked::*;

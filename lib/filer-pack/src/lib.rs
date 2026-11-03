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
//!   by names are created, the later of duplicate members wins, and
//!   hardlinks share one node;
//! - [`Archive`] serves the finished tree as a read-only [`Driver`];
//! - [`Codec`] and [`Decoder`] decode compressed members, which
//!   [`Member`] records with their format's codec;
//! - [`Limits`] bounds what an untrusted archive may make the index hold;
//! - [`ArchiveError`] names the format, header offset, and reason of a
//!   failed open.
//!
//! More is to move in (D59): the limit checks and the read helper each
//! parser repeats; one checksum-verifying decoder; a cache of decoded
//! streams, and a [`ReadAt`] source over one, for compressed tarballs and
//! solid archives; and pure-Rust decoders, one feature per algorithm.
//!
//! **Read-only, permanently.** Filer reads archives as directories; it
//! never creates or modifies them, and never will: nothing here will
//! compress or write.
//!
//! Like the drivers, it treats archives as untrusted input: nothing in it
//! may panic, hang, or allocate without bound on any input, which its
//! root denies by lint. It needs no OS and builds with `#![no_std]`,
//! locking with a spin lock there; the `std` feature, enabled by default,
//! forwards to `filer-api` and locks with the standard mutex. `all`
//! currently enables nothing.
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

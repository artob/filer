// This is free and unencumbered software released into the public domain.

//! A bzip2 decoder, written clean room from a description of the format,
//! and dedicated to the public domain.
//!
//! [`Decoder`] decodes bzip2 streams: streams one after another, each of
//! blocks checked against their CRCs, and their CRCs against the
//! stream's. After the first stream, what does not begin another ends the
//! input, unread, as bzip2 ignores trailing garbage. It decodes only:
//! nothing here compresses, and nothing will.
//!
//! It is sans-I/O: [`Decoder::decode`] takes what input there is and
//! fills what room there is, and says how much of each it used, so that
//! it serves blocking, async, and in-memory callers alike. A stream may be
//! given and taken in pieces of any size, down to a byte, and decodes the
//! same as whole. [`Decoder::checkpoint`] offers a decoder that goes on
//! from where one is, between blocks, so that a caller may resume reading
//! a long stream part way rather than from its start.
//!
//! Streams are untrusted input. Nothing in this crate may panic, loop
//! without end, or allocate without bound on any input, which its root
//! denies by lint: it gathers each block whole before it decodes it, so it
//! holds at most a block of packed bytes, about 900 KiB, and four bytes of
//! links per byte of the block; what a block decodes to, up to 50 times
//! larger, is produced as it is read. Randomized blocks, which bzip2
//! stopped writing in 1999, are [`Unsupported`](Error::Unsupported);
//! anything malformed is [`Corrupt`](Error::Corrupt).
//!
//! Its source is Joe Tsai's *bzip2 format* (2016), an independent
//! description of what bzip2 1.0.6 writes, with bzip2's own source and
//! Wuffs' decoder read for facts only, never copied. It is pure, safe Rust
//! without dependencies, and builds with `#![no_std]`, with an allocator.
//! The `std` feature, enabled by default, currently adds nothing.
//!
//! It was written for [Filer](https://github.com/artob/filer), whose
//! archive drivers read bzip2 through it, as one of its `decode-*` crates:
//! clean-room, public-domain decoders of the codecs that Filer reads.

#![no_std]
#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
// No input may make this crate panic.
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

mod decoder;
pub use decoder::*;

mod error;
pub use error::*;

// The README's example runs as a doctest.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

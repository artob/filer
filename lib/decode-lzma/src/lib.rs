// This is free and unencumbered software released into the public domain.

//! An LZMA and LZMA2 decoder, ported from the LZMA SDK, which Igor Pavlov
//! placed in the public domain, and dedicated to the public domain.
//!
//! [`Decoder`] decodes LZMA, the Lempel-Ziv-Markov chain algorithm of
//! 7-Zip, as 7z archives, ZIP's method 14, and `.lzma` files hold it, with
//! [`Decoder::lzma`]; and LZMA2, LZMA in chunks that may be stored as they
//! are or reset, as xz streams and 7z archives hold it, with
//! [`Decoder::lzma2`]. It consumes a stream to its end and no further, so
//! that a container reads on from what follows. It decodes only: nothing
//! here compresses, and nothing will.
//!
//! It is sans-I/O: [`Decoder::decode`] takes what input there is and
//! fills what room there is, and says how much of each it used, so that
//! it serves blocking, async, and in-memory callers alike. A stream may be
//! given and taken in pieces of any size, down to a byte, and decodes the
//! same as whole. [`Decoder::checkpoint`] offers a copy of the decoder
//! anywhere, its dictionary included, so that a caller may resume reading
//! a long stream part way rather than from its start.
//!
//! Streams are untrusted input. Nothing in this crate may panic, loop
//! without end, or allocate without bound on any input, which its root
//! denies by lint: its dictionary grows only as the stream does, up to
//! its size, and a dictionary over 128 MiB, which `xz -9` doubled reaches,
//! is [`Unsupported`](Error::Unsupported). What the LZMA SDK refuses, it
//! refuses as [`Corrupt`](Error::Corrupt): matches past the dictionary or
//! the start of the stream, data that ends otherwise than its size or
//! marker says, and LZMA2 chunks out of order.
//!
//! Its source is the LZMA SDK's specification and reference decoder, and
//! for LZMA2, its decoder, all public domain; no other decoder's code is
//! read. It is pure, safe Rust without dependencies, and builds with
//! `#![no_std]`, with an allocator. The `std` feature, enabled by
//! default, currently adds nothing.
//!
//! It is one of the `decode-*` crates: clean-room, public-domain decoders
//! of common codecs, each of the same shape.

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

mod model;
mod range;
mod window;

// The README's example runs as a doctest.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

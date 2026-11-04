// This is free and unencumbered software released into the public domain.

//! A Deflate decoder, written clean room from RFC 1951 alone, and
//! dedicated to the public domain.
//!
//! [`Decoder`] decodes raw Deflate streams (RFC 1951), as ZIP stores its
//! members, and as gzip (RFC 1952) and zlib (RFC 1950) frame them: blocks
//! stored as they are, or Huffman-coded with fixed or dynamic codes, and
//! matches reaching back up to 32 KiB. It consumes a stream to its end and
//! no further, so that a container reads on from what follows. It decodes
//! only: nothing here compresses, and nothing will.
//!
//! It is sans-I/O: [`Decoder::decode`] takes what input there is and
//! fills what room there is, and says how much of each it used, so that
//! it serves blocking, async, and in-memory callers alike. A stream may be
//! given and taken in pieces of any size, down to a byte, and decodes the
//! same as whole. [`Decoder::checkpoint`] offers a copy of the decoder
//! anywhere, its window and a few KiB, so that a caller may resume reading
//! a long stream part way rather than from its start.
//!
//! [`Decoder::deflate64`] decodes Deflate64, PKWARE's variant of Deflate,
//! ZIP's method 9 and one of 7z's: a 64 KiB window, length code 285 as 3
//! and 16 extra bits, for matches up to 65,538 bytes long, and distance
//! codes 30 and 31. A stream is one or the other, as its container says,
//! so the choice is per decoder, and the additive `deflate64` feature,
//! enabled by `all`, only adds that constructor.
//!
//! Streams are untrusted input. Nothing in this crate may panic, loop
//! without end, or allocate without bound on any input, which its root
//! denies by lint. What zlib refuses, it refuses as
//! [`Corrupt`](Error::Corrupt): Huffman codes that over-subscribe, or
//! leave codes unused but in the cases the RFC names; more than 286
//! literal/length or 30 distance codes; codes that never occur; and
//! distances past the start of the stream or the window.
//!
//! Its source is RFC 1951, and for Deflate64, which PKWARE does not
//! document, how it differs from Deflate, checked against what 7-Zip
//! writes; zlib's own code is not copied, nor any other decoder's. It is
//! pure, safe Rust without dependencies, and builds with `#![no_std]`,
//! with an allocator. The `std` feature, enabled by default, currently
//! adds nothing.
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

mod huffman;

// The README's example runs as a doctest.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

// This is free and unencumbered software released into the public domain.

//! A zstd decoder, written clean room from RFC 8878 alone, and dedicated
//! to the public domain.
//!
//! [`Decoder`] decodes zstd streams (RFC 8878): frames one after another,
//! each checked against its content size and XXH64 checksum, if it has
//! them, and skippable frames read past. It decodes only: nothing here
//! compresses, and nothing will.
//!
//! It is sans-I/O: [`Decoder::decode`] takes what input there is and
//! fills what room there is, and says how much of each it used, so that
//! it serves blocking, async, and in-memory callers alike. A stream may be
//! given and taken in pieces of any size, down to a byte, and decodes the
//! same as whole. [`Decoder::checkpoint`] offers a decoder that goes on
//! from where one is, so that a caller may resume reading a long stream
//! part way rather than from its start: between frames, a new one, and
//! within a frame, a copy of its window and tables.
//!
//! Streams are untrusted input. Nothing in this crate may panic, loop
//! without end, or allocate without bound on any input, which its root
//! denies by lint: it holds at most a block of packed bytes, 128 KiB, and
//! of decoded bytes the frame's window and two blocks, in a buffer that
//! decoding goes round, allocated as it fills. A frame whose window passes 128 MiB, the most zstd itself
//! decodes by default, or that needs a dictionary is
//! [`Unsupported`](Error::Unsupported); anything malformed, or that zstd
//! itself refuses, is [`Corrupt`](Error::Corrupt).
//!
//! Its sources are RFC 8878, and the xxHash specification for the XXH64
//! checksum; zstd's and xxHash's own code is not copied, nor any other
//! decoder's. It is pure, safe Rust, its XXH64 `compute-xxhash`'s, written
//! likewise, and builds with `#![no_std]`, with an allocator. The `std`
//! feature, enabled by default, currently adds nothing.
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

#[cfg(test)]
extern crate std;

// The crate's own tests share its integration tests' helpers.
#[cfg(test)]
extern crate self as decode_zstd;
#[cfg(test)]
#[path = "../tests/common/mod.rs"]
mod common;

mod bits;
mod block;
mod fse;
mod huffman;
mod literals;
mod sequences;
mod window;

mod decoder;
pub use decoder::*;

mod error;
pub use error::*;

// The README's example runs as a doctest.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

// This is free and unencumbered software released into the public domain.

//! A gzip decoder, written clean room from RFC 1952 alone, and dedicated to
//! the public domain.
//!
//! [`Decoder`] decodes gzip streams (RFC 1952), as `.gz` files, compressed
//! tarballs, and initramfs images hold them: members of Deflate data, each
//! with a header, whose optional fields it reads past, and a trailer,
//! whose CRC-32 and length of what the member decodes to it checks. It
//! decodes members that follow one another as one stream, as gzip does,
//! and consumes the stream to its last member's end and no further, so
//! that what follows, such as padding, is left unread. It decodes only:
//! nothing here compresses, and nothing will.
//!
//! It is sans-I/O: [`Decoder::decode`] takes what input there is and
//! fills what room there is, and says how much of each it used, so that
//! it serves blocking, async, and in-memory callers alike. A stream may be
//! given and taken in pieces of any size, down to a byte, and decodes the
//! same as whole. [`Decoder::checkpoint`] offers a copy of the decoder
//! anywhere, its window and a few KiB, so that a caller may resume reading
//! a long stream part way rather than from its start.
//!
//! Streams are untrusted input. Nothing in this crate may panic, loop
//! without end, or allocate without bound on any input, which its root
//! denies by lint. What gzip refuses, it refuses as
//! [`Corrupt`](Error::Corrupt): a method other than Deflate, reserved
//! flags, a header's or member's checksum that does not match, and a
//! stream that ends within a member.
//!
//! Its source is RFC 1952; zlib's own code is not copied, nor any other
//! decoder's. It decodes Deflate with `decode-deflate`, written likewise
//! from RFC 1951 alone, and computes CRC-32 with `compute-crc`. It is pure,
//! safe Rust, and builds with
//! `#![no_std]`, with an allocator. The `std` feature, enabled by
//! default, adds nothing by itself.
//!
//! The `intrinsics` feature, not enabled by default, computes CRC-32 by
//! the CPU's instructions where it has them, ARM's `crc` extension, as
//! `compute-crc`'s feature of that name does: chosen at run time with
//! `std`. It decodes a quarter faster on Apple M4.
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

mod decoder;
pub use decoder::*;

mod error;
pub use error::*;

// The README's example runs as a doctest.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

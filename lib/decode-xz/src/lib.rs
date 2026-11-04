// This is free and unencumbered software released into the public domain.

//! An xz decoder, written clean room from the `.xz` file format
//! specification, its branch converters ported from the LZMA SDK, both
//! public domain, and its SHA-256 from FIPS 180-4; and dedicated to the
//! public domain.
//!
//! [`Decoder`] decodes `.xz` files, as compressed tarballs, Debian
//! packages, and initramfs images hold them: streams, one after another,
//! with padding between them, of blocks, each of LZMA2, decoded by
//! `decode-lzma`, through Delta or a branch converter for x86, PowerPC,
//! IA-64, ARM, ARM Thumb, SPARC, ARM64, or RISC-V code; each block checked
//! against its CRC-32, CRC-64, or SHA-256, and each stream against its
//! index. It decodes only: nothing here compresses, and nothing will.
//!
//! It is sans-I/O: [`Decoder::decode`] takes what input there is and
//! fills what room there is, and says how much of each it used, so that
//! it serves blocking, async, and in-memory callers alike. A stream may be
//! given and taken in pieces of any size, down to a byte, and decodes the
//! same as whole. As each block decodes alone, [`Decoder::checkpoint`]
//! offers one between blocks, so that a caller may resume reading a long
//! stream at a block rather than from its start.
//!
//! Streams are untrusted input. Nothing in this crate may panic, loop
//! without end, or allocate without bound on any input, which its root
//! denies by lint. What the specification says a decoder must refuse, it
//! refuses as [`Corrupt`](Error::Corrupt); filters it does not know, and
//! dictionaries over 128 MiB, are [`Unsupported`](Error::Unsupported).
//!
//! Its sources are the `.xz` file format specification, the LZMA SDK's
//! branch converters, and FIPS 180-4; xz's own code is not copied, nor
//! any other decoder's. It is pure, safe Rust, and builds with
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

mod check;

mod decoder;
pub use decoder::*;

mod error;
pub use error::*;

mod filter;

// The README's example runs as a doctest.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

// This is free and unencumbered software released into the public domain.

//! An xz decoder, written clean room from the `.xz` file format
//! specification, its branch converters ported from the LZMA SDK, both
//! public domain; and dedicated to the public domain.
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
//! Its sources are the `.xz` file format specification and the LZMA SDK's
//! branch converters; xz's own code is not copied, nor any other
//! decoder's. Its checks are `compute-crc`'s and `compute-sha256`'s,
//! written likewise. It is pure, safe Rust, and builds with
//! `#![no_std]`, with an allocator. The `std` feature, enabled by
//! default, adds nothing by itself.
//!
//! The `intrinsics` feature, enabled by default (D66), computes the
//! checks, CRC-32, CRC-64, and SHA-256, by the CPU's instructions where it
//! has them, as `compute-crc`'s and `compute-sha256`'s features of that
//! name do: chosen at run time with `std`.
//! Without it, as `default-features = false, features = ["all", "std"]`
//! builds it, its dependencies are ours alone, all of them public domain,
//! and on x86-64, streams checked by SHA-256 decode 29% slower, Canterbury
//! at 112 MB/s, not 144, on Zen 5, and at 85, not 109, on Granite Rapids;
//! those checked by CRC-64, xz's default, 2% slower; and those checked by
//! CRC-32 within 1%. On Neoverse V3, those checked by SHA-256 decode 28%
//! slower, at 95 MB/s, not 131; by CRC-64, 4%; and by CRC-32, 3%. On
//! Apple M4, those checked by SHA-256 decode 22% slower, at 120 MB/s, not
//! 154; by CRC-64, 4%; and by CRC-32, 2% (2026-11-11).
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

mod filter;

// The README's example runs as a doctest.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

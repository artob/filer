// This is free and unencumbered software released into the public domain.

//! An LZ4 decoder, written clean room from LZ4's block and frame format
//! descriptions, and dedicated to the public domain.
//!
//! [`Decoder`] decodes LZ4 streams, as `.lz4` files and initramfs images
//! hold them: frames one after another, each checked against its
//! checksums and size, if it has them, its blocks linked or independent;
//! legacy frames, as `lz4 -l` writes them for the Linux kernel; and
//! skippable frames, read past. [`decode_block`] decodes a raw block, as
//! SquashFS, EROFS, ZFS, F2FS, and ReFS store them. It decodes only:
//! nothing here compresses, and nothing will.
//!
//! [`Decoder`] is sans-I/O: [`Decoder::decode`] takes what input there is
//! and fills what room there is, and says how much of each it used, so
//! that it serves blocking, async, and in-memory callers alike. A stream
//! may be given and taken in pieces of any size, down to a byte, and
//! decodes the same as whole. [`Decoder::checkpoint`] offers a copy of the
//! decoder anywhere, the 64 KiB a block may copy from and what is not yet
//! produced, so that a caller may resume reading a long stream part way
//! rather than from its start.
//!
//! Streams are untrusted input. Nothing in this crate may panic, loop
//! without end, or allocate without bound on any input, which its root
//! denies by lint: it holds at most a block of packed bytes, 4 MiB, or
//! 8 MiB in a legacy frame, and of decoded bytes a block and the 64 KiB
//! before it. A frame that needs a dictionary is
//! [`Unsupported`](Error::Unsupported); anything malformed is
//! [`Corrupt`](Error::Corrupt).
//!
//! Its sources are the LZ4 Block Format and LZ4 Frame Format
//! descriptions, and the xxHash specification for the XXH32 checksums;
//! LZ4's and xxHash's own code is not copied, nor any other decoder's. It
//! is pure, safe Rust, its XXH32 `compute-xxhash`'s, written likewise,
//! and builds with `#![no_std]`, with an allocator. The `std` feature,
//! enabled by default, currently adds nothing.
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

mod block;
pub use block::decode_block;

mod decoder;
pub use decoder::*;

mod error;
pub use error::*;

// The README's example runs as a doctest.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

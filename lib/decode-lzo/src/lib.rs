// This is free and unencumbered software released into the public domain.

//! An LZO decoder, written clean room from a description of the LZO1X
//! stream format, and dedicated to the public domain.
//!
//! [`decode_block`] decodes a raw LZO1X block, as btrfs, SquashFS, and
//! F2FS store them, and the Linux kernel's `lzo-rle` blocks, which zram
//! and F2FS may write. [`Decoder`] decodes lzop files, as `.lzo` files and
//! the Linux kernel's initramfs images hold them: files one after
//! another, each block checked against its Adler-32s or CRC-32s, if it
//! has them. It decodes only: nothing here compresses, and nothing will.
//!
//! [`Decoder`] is sans-I/O: [`Decoder::decode`] takes what input there is
//! and fills what room there is, and says how much of each it used, so
//! that it serves blocking, async, and in-memory callers alike. A stream
//! may be given and taken in pieces of any size, down to a byte, and
//! decodes the same as whole. Each block decodes alone, so
//! [`Decoder::checkpoint`] offers a copy of the decoder anywhere, holding
//! only what it has not yet produced, so that a caller may resume reading
//! a long stream part way rather than from its start.
//!
//! Streams are untrusted input. Nothing in this crate may panic, loop
//! without end, or allocate without bound on any input, which its root
//! denies by lint: it holds at most a block of packed bytes and a block
//! of what they decode to, 64 MiB each at most, as lzop allows, and
//! 256 KiB as it writes them. A file filtered, part of a multipart
//! archive, or compressed otherwise than by LZO1X, and a block that names
//! a version past `lzo-rle`'s, 1, are [`Unsupported`](Error::Unsupported);
//! anything malformed is [`Corrupt`](Error::Corrupt).
//!
//! Its sources are the Linux kernel's description of the LZO1X stream
//! format, for facts only, and lzop's published layout, checked against
//! files that `lzop` writes; LZO's, lzop's, and lzokay's own code is not
//! read, nor any other decoder's. It is pure, safe Rust, its Adler-32
//! `compute-adler32`'s and its CRC-32 `compute-crc`'s, written likewise,
//! and builds with `#![no_std]`, with an allocator. The `std` feature,
//! enabled by default, adds nothing by itself.
//!
//! The `intrinsics` feature, enabled by default (D66), computes lzop's
//! CRC-32s by the CPU's instructions where it has them, ARM's `crc`
//! extension, as `compute-crc`'s feature of that name does: chosen at
//! run time with `std`. Without it, as `default-features = false,
//! features = ["all", "std"]` builds it, its dependencies are ours alone,
//! all of them public domain, and files checked by CRC-32, as `lzop
//! --crc32` writes them, decode 13% slower on Apple M4, Canterbury at 778
//! MB/s, not 897; those checked by Adler-32, lzop's default, and raw
//! blocks no slower (2026-11-08). On x86-64, where carry-less
//! multiplication takes CRC-32, files checked by it decode 15% slower on
//! Zen 5, Canterbury at 854 MB/s, not 982, and 10% on Granite Rapids, at
//! 614, not 678; the others no slower (2026-11-10).
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

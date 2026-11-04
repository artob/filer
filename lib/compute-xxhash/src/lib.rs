// This is free and unencumbered software released into the public domain.

//! The xxHash hashes, written clean room from the xxHash specification,
//! and dedicated to the public domain, each behind a feature of its name,
//! both enabled by `all`, over bytes fed in pieces, as they arrive, and
//! from any seed: [`Xxh32`], which LZ4 frames check their headers, blocks,
//! and content with; and [`Xxh64`], which zstd checks frames with.
//!
//! The source is the xxHash specification, whose notice lets anyone copy
//! and distribute it; xxHash's own code is not copied. It is pure, safe
//! Rust without dependencies, and builds with `#![no_std]`, without an
//! allocator. The `std` feature, enabled by default, currently adds
//! nothing.
//!
//! It is one of the `compute-*` crates: clean-room, public-domain
//! checksums and hashes, each of the same shape.

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

#[cfg(feature = "xxh32")]
mod xxh32;
#[cfg(feature = "xxh32")]
pub use xxh32::*;

#[cfg(feature = "xxh64")]
mod xxh64;
#[cfg(feature = "xxh64")]
pub use xxh64::*;

// The README's example runs as a doctest.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

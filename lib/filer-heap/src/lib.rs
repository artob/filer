// This is free and unencumbered software released into the public domain.

//! In-memory driver for the Filer API.
//!
//! [`Heap`] is a filesystem held entirely in memory: the reference
//! implementation of the driver contract and the test bed for code built on
//! [`filer_api`]. It needs no OS and builds with `#![no_std]`, using only
//! `alloc`; the `std` feature, enabled by default, forwards to `filer-api`.
//! `all` currently enables nothing.
//!
//! So far it is read-only: a [`HeapBuilder`] fixes its contents at
//! construction, and it answers the read operations of [`filer_api::Op`].
//!
//! # Examples
//!
//! ```
//! use filer_api::Driver;
//! use filer_heap::Heap;
//!
//! let heap = Heap::builder()
//!     .file("config/app.toml", "debug = true")
//!     .build();
//! let root = heap.root();
//! assert_eq!(root.driver(), heap.id());
//! ```

#![no_std]
#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
// No input may make this crate panic (DESIGN.md D47).
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

mod heap;
pub use heap::*;

mod heap_builder;
pub use heap_builder::*;

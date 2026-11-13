// This is free and unencumbered software released into the public domain.

//! WASI backend for the Filer API.
//!
//! This crate will implement [`filer_api`] on WebAssembly System Interface
//! targets, such as `wasm32-wasip1` and `wasm32-wasip2`, and does not export
//! any items yet. Its items will be gated on `cfg(target_os = "wasi")`, so the
//! crate still builds, empty, for other targets. It builds with `#![no_std]`;
//! the `std` feature, enabled by default, forwards to `filer-api`. `all`
//! currently enables nothing.

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

// The backend is to implement `filer-api` (D30); until it does, this keeps
// Cargo from warning that the dependency goes unused.
use filer_api as _;

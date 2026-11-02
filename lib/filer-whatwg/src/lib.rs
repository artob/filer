// This is free and unencumbered software released into the public domain.

//! WHATWG File System Standard backend for the Filer API, for browsers.
//!
//! This crate will implement [`filer_api`] over the [WHATWG File System
//! Standard]: the Origin Private File System (OPFS), and directories the
//! user grants through the File System Access picker
//! (`showDirectoryPicker()`), which yields the same handle types. Only
//! where the root handle comes from differs. It is for browsers; Node and
//! Deno do not implement this API.
//!
//! The standard's model is already capability-based: there are no paths
//! from a global root, only directory handles a page was given, which map
//! directly onto [`filer_api::Dir`].
//!
//! It does not export any items yet. Its items will be gated on
//! `cfg(all(target_family = "wasm", target_os = "unknown"))`, so the crate
//! still builds, empty, for other targets. It builds with `#![no_std]`; the
//! `std` feature, enabled by default, forwards to `filer-api`. `all`
//! currently enables nothing.
//!
//! [WHATWG File System Standard]: https://fs.spec.whatwg.org/

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

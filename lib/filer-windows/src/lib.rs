// This is free and unencumbered software released into the public domain.

//! Windows backend for the Filer API.
//!
//! [`Windows`] implements [`filer_api::Driver`] on Windows over
//! [`windows-sys`]. So far it opens roots, files, and directories, with
//! paths confined by the generic [`filer_api::BlockingResolver`], and
//! reads files, metadata, directories, and links, executed by the [blocking](Windows::blocking) executor or
//! on a [thread pool](Windows::thread_pool). All of its `unsafe` is in one
//! private module of safe wrappers over the system calls.
//!
//! Items are gated on `cfg(windows)` and the `std` feature, enabled by
//! default, so the crate still builds, empty, without `std` and on other
//! targets such as Linux and WASI. `all` currently enables nothing.
//!
//! [`windows-sys`]: https://docs.rs/windows-sys

#![no_std]
#![deny(unsafe_code)]
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
// The crate docs link to items that exist only on Windows with `std`.
#![cfg_attr(
    not(all(windows, feature = "std")),
    allow(rustdoc::broken_intra_doc_links)
)]

#[cfg(feature = "std")]
extern crate std;

#[cfg(all(feature = "std", any(windows, test)))]
mod dir_info;

#[cfg(all(windows, feature = "std"))]
mod error;

#[cfg(all(windows, feature = "std"))]
mod node;

#[cfg(all(windows, feature = "std"))]
#[allow(unsafe_code, reason = "all of the crate's `unsafe` is here")]
mod sys;

#[cfg(all(windows, feature = "std"))]
mod windows;
#[cfg(all(windows, feature = "std"))]
pub use windows::*;

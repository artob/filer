// This is free and unencumbered software released into the public domain.

//! POSIX backend for the Filer API.
//!
//! [`Posix`] implements [`filer_api::Driver`] on Unix platforms (Linux,
//! macOS, the BSDs) over [`rustix`]. So far it supports the read
//! operations, executed by the [blocking](Posix::blocking) executor, with
//! paths confined by the generic [`filer_api::BlockingResolver`].
//!
//! Items are gated on `cfg(unix)` and the `std` feature, enabled by
//! default, so the crate still builds, empty, without `std` and on other
//! targets such as Windows and WASI. `all` currently enables nothing.
//!
//! [`rustix`]: https://docs.rs/rustix

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
// The crate docs link to items that exist only on Unix with `std`.
#![cfg_attr(
    not(all(unix, feature = "std")),
    allow(rustdoc::broken_intra_doc_links)
)]

#[cfg(feature = "std")]
extern crate std;

#[cfg(all(unix, feature = "std"))]
mod errno;

#[cfg(all(any(target_os = "linux", target_os = "android"), feature = "std"))]
mod kernel;

#[cfg(all(unix, feature = "std"))]
mod node;

#[cfg(all(unix, feature = "std"))]
mod posix;
#[cfg(all(unix, feature = "std"))]
pub use posix::*;

#[cfg(all(unix, feature = "std"))]
mod stat;

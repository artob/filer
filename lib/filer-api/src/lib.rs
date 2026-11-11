// This is free and unencumbered software released into the public domain.

//! Platform-independent Filer API.
//!
//! This crate will define the types and traits that platform backends such as
//! `filer-posix` implement. Applications use the frontend: a [`Dir`] is an
//! open directory, the unit of authority, through which files ([`File`])
//! are opened, entries listed ([`ReadDir`]), and paths resolved, confined
//! beneath it. Below the frontend
//! is the driver boundary: a [`Driver`] receives each operation as an
//! [`Op`] value and answers with a [`Completion`]. So far only read
//! operations exist.
//!
//! Around them are the lexical [`path`] types, the on-disk name types
//! ([`EntryName`], [`ForeignName`], [`LinkTarget`]), driver identity and
//! handles ([`DriverKey`], [`DriverId`], [`Handle`]), [`Metadata`], owned
//! I/O buffers ([`IoBuf`], [`IoBufMut`], [`BufResult`]), the [`Error`]
//! type, and for drivers, the generic [`Resolver`] (and
//! [`BlockingResolver`]), the [`tree`] of virtual drivers, and the
//! [`checksum`]s they share.
//!
//! It builds with `#![no_std]` but requires `alloc`; the `std` feature,
//! enabled by default, adds `std::io::Error` interop. `all` currently
//! enables nothing. The `intrinsics` feature, enabled by default (D66),
//! computes the [`checksum`]s by the CPU's instructions where it has
//! them, as `compute-crc`'s feature of that name does: each CRC by
//! carry-less multiplication, x86-64's and ARM's, chosen at run time with
//! `std`. Without it, as `default-features = false, features = ["all",
//! "std"]` builds it, its dependencies are ours alone, all of them public
//! domain, and those CRCs are as much slower as `compute-crc`'s docs say:
//! 9 to 23 times over long inputs on x86-64 (Zen 5 and Granite Rapids),
//! and 9 to 12 times on ARM (Neoverse V3).

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

#[cfg(feature = "std")]
extern crate std;

pub mod checksum;
pub mod path;
pub mod tree;

mod ambient_authority;
pub use ambient_authority::*;

mod blocking_resolver;
pub use blocking_resolver::*;

mod buf_result;
pub use buf_result::*;

mod completion;
pub use completion::*;

mod dir;
pub use dir::*;

#[cfg(feature = "std")]
mod default_pool;
#[cfg(feature = "std")]
pub use default_pool::*;

mod dir_cookie;
pub use dir_cookie::*;

mod driver;
pub use driver::*;

mod driver_id;
pub use driver_id::*;

mod driver_key;
pub use driver_key::*;

mod dyn_driver;
pub use dyn_driver::*;

mod entry;
pub use entry::*;

mod entry_name;
pub use entry_name::*;

mod erased_buf_mut;
pub use erased_buf_mut::*;

mod error;
pub use error::*;

mod error_kind;
pub use error_kind::*;

mod escaped;

mod fallback;
pub use fallback::*;

mod file;
pub use file::*;

mod file_attrs;
pub use file_attrs::*;

mod file_id;
pub use file_id::*;

mod file_kind;
pub use file_kind::*;

mod foreign_name;
pub use foreign_name::*;

mod handle;
pub use handle::*;

mod handle_kind;
pub use handle_kind::*;

mod io_buf;
pub use io_buf::*;

mod link_target;
pub use link_target::*;

mod meta_mask;
pub use meta_mask::*;

mod metadata;
pub use metadata::*;

mod mode;
pub use mode::*;

mod op;
pub use op::*;

mod op_kind;
pub use op_kind::*;

mod open_how;
pub use open_how::*;

mod output;
pub use output::*;

mod raw_os_error;
pub use raw_os_error::*;

mod read_at;
pub use read_at::*;

mod read_dir;
pub use read_dir::*;

mod resolve;
pub use resolve::*;

mod resolver;
pub use resolver::*;

mod walk;

mod rights;
pub use rights::*;

#[cfg(feature = "std")]
mod run_on;
#[cfg(feature = "std")]
pub use run_on::*;

mod spawner;
pub use spawner::*;

mod target;
pub use target::*;

mod timestamp;
pub use timestamp::*;

mod xattr;

mod xattr_entry;
pub use xattr_entry::*;

mod xattr_name;
pub use xattr_name::*;

mod xattr_ns;
pub use xattr_ns::*;

mod xattr_op;
pub use xattr_op::*;

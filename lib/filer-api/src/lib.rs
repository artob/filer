// This is free and unencumbered software released into the public domain.

//! Platform-independent Filer API.
//!
//! This crate will define the types and traits that platform backends such as
//! `filer-posix` implement. At its core is the driver boundary: a
//! [`Driver`] receives each operation as an [`Op`] value and answers with a
//! [`Completion`]. So far only read operations exist.
//!
//! Around it are the lexical [`path`] types, the on-disk name types
//! ([`EntryName`], [`ForeignName`], [`LinkTarget`]), driver identity and
//! handles ([`DriverKey`], [`DriverId`], [`Handle`]), [`Metadata`], owned
//! I/O buffers ([`IoBuf`], [`IoBufMut`], [`BufResult`]), and the [`Error`]
//! type.
//!
//! It builds with `#![no_std]` but requires `alloc`; the `std` feature,
//! enabled by default, adds `std::io::Error` interop. `all` currently
//! enables nothing.

#![no_std]
#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]

extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

pub mod path;

mod ambient_authority;
pub use ambient_authority::*;

mod buf_result;
pub use buf_result::*;

mod completion;
pub use completion::*;

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

mod resolve;
pub use resolve::*;

mod resolver;
pub use resolver::*;

mod target;
pub use target::*;

mod timestamp;
pub use timestamp::*;

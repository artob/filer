// This is free and unencumbered software released into the public domain.

//! Platform-independent Filer API.
//!
//! This crate will define the types and traits that platform backends such as
//! `filer-posix` implement. So far it provides the lexical [`path`] types,
//! the on-disk name types ([`EntryName`], [`ForeignName`], [`LinkTarget`]),
//! driver identity ([`DriverKey`], [`DriverId`]), and the [`Error`] type.
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

mod driver_id;
pub use driver_id::*;

mod driver_key;
pub use driver_key::*;

mod entry_name;
pub use entry_name::*;

mod error;
pub use error::*;

mod error_kind;
pub use error_kind::*;

mod escaped;

mod fallback;
pub use fallback::*;

mod foreign_name;
pub use foreign_name::*;

mod link_target;
pub use link_target::*;

mod op_kind;
pub use op_kind::*;

mod raw_os_error;
pub use raw_os_error::*;

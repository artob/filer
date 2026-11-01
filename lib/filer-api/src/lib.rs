// This is free and unencumbered software released into the public domain.

//! Platform-independent Filer API.
//!
//! This crate will define the types and traits that platform backends such as
//! `filer-posix` implement. So far it provides the lexical [`path`] types. It
//! builds with `#![no_std]` but requires `alloc`; the `std` feature, enabled
//! by default, will gate APIs that need the standard library. `all` currently
//! enables nothing.

#![no_std]
#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]

extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

pub mod path;

// This is free and unencumbered software released into the public domain.

//! Platform-independent Filer API.
//!
//! This crate will define the types and traits that platform backends such as
//! `filer-posix` implement, and does not export any items yet. It builds with
//! `#![no_std]`; the `std` feature, enabled by default, will gate APIs that
//! need the standard library. `all` currently enables nothing.

#![no_std]
#![deny(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]

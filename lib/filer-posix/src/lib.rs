// This is free and unencumbered software released into the public domain.

//! POSIX backend for the Filer API.
//!
//! This crate will implement [`filer_api`] on Unix platforms and does not
//! export any items yet. Its items will be gated on `cfg(unix)`, so the crate
//! still builds, empty, for other targets such as Windows. It builds with
//! `#![no_std]`; the `std` feature, enabled by default, forwards to
//! `filer-api`. `all` currently enables nothing.

#![no_std]
#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]

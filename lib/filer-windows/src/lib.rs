// This is free and unencumbered software released into the public domain.

//! Windows backend for the Filer API.
//!
//! This crate will implement [`filer_api`] on Windows and does not export any
//! items yet. Its items will be gated on `cfg(windows)`, so the crate still
//! builds, empty, for other targets. It builds with `#![no_std]`; the `std`
//! feature, enabled by default, forwards to `filer-api`. `all` currently
//! enables nothing.

#![no_std]
#![deny(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]

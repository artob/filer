// This is free and unencumbered software released into the public domain.

//! WASI backend for the Filer API.
//!
//! This crate will implement [`filer_api`] on WebAssembly System Interface
//! targets, such as `wasm32-wasip1` and `wasm32-wasip2`, and does not export
//! any items yet. Its items will be gated on `cfg(target_os = "wasi")`, so the
//! crate still builds, empty, for other targets. It builds with `#![no_std]`;
//! the `std` feature, enabled by default, forwards to `filer-api`. `all`
//! currently enables nothing.

#![no_std]
#![deny(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]

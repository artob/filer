// This is free and unencumbered software released into the public domain.

//! TAR archive driver for the Filer API.
//!
//! This crate will implement [`filer_api::Driver`] over TAR archives,
//! read-only: a `Dir` over an archive's contents, opened from any
//! [`filer_api::ReadAt`] source, such as a `File` from another driver or
//! bytes in memory. It does not export any items yet.
//!
//! The plan (see `DESIGN.md` §17.7): one pass over the 512-byte headers
//! builds an index, after which member contents, being contiguous, are
//! truly random-access. Compressed archives (gzip, zstd, xz) will be
//! indexed on open, since seeking back in them means decompressing again.
//! Paths are confined to the archive root, later duplicate entries win,
//! hardlinks share one node, and strict limits guard against hostile
//! archives. Forward-only sources will get a streaming visitor instead of
//! a `Dir`.
//!
//! It needs no OS and builds with `#![no_std]`; the `std` feature, enabled
//! by default, forwards to `filer-api`. `all` currently enables nothing.

#![no_std]
#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]

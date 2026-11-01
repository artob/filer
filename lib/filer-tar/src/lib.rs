// This is free and unencumbered software released into the public domain.

//! TAR archive driver for the Filer API.
//!
//! [`Tar`] serves the contents of a TAR archive, read-only, as a
//! [`Dir`](filer_api::Dir): open the archive from any
//! [`ReadAt`](filer_api::ReadAt) source, such as a `File` from another
//! driver or bytes in memory, then use it like any other directory. Paths
//! stay confined to the archive, and a hostile archive fails to open rather
//! than exposing anything outside it (see [`Tar`] and [`Limits`]).
//!
//! So far it reads uncompressed archives with random access. Compressed
//! archives (gzip, zstd, xz) and forward-only sources are planned (see
//! `DESIGN.md` §17.7).
//!
//! It needs no OS and builds with `#![no_std]`; the `std` feature, enabled
//! by default, forwards to `filer-api`. `all` currently enables nothing.

#![no_std]
#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]

extern crate alloc;

mod header;

mod index;

mod limits;
pub use limits::*;

mod tar;
pub use tar::*;

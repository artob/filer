// This is free and unencumbered software released into the public domain.

//! ZIP archive driver for the Filer API.
//!
//! This crate will implement [`filer_api::Driver`] over ZIP archives,
//! read-only: a `Dir` over an archive's contents, opened from any
//! [`filer_api::ReadAt`] source, such as a `File` from another driver or
//! bytes in memory. It does not export any items yet.
//!
//! **Read-only, permanently.** Filer reads archives as directories; it
//! never creates or modifies them, and never will: writing archives is out
//! of scope for this project. Use a dedicated archiving library for that.
//!
//! The plan (see `DESIGN.md` §17.7): the central directory at the end of
//! the archive indexes the members, so the source must be random-access
//! with a known size. Members are compressed individually (stored and
//! Deflate first, other codecs behind features). Paths are confined to the
//! archive root ("zip slip" cannot happen), directories missing from the
//! archive are synthesized, later duplicate entries win, local and central
//! headers must agree, and strict limits on decompression ratio, total
//! size, and entry count guard against zip bombs. Decoding legacy CP437
//! names is a stretch goal.
//!
//! It needs no OS and builds with `#![no_std]`; the `std` feature, enabled
//! by default, forwards to `filer-api`. `all` currently enables nothing.

#![no_std]
#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]

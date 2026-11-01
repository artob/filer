// This is free and unencumbered software released into the public domain.

//! Read-only ar archive driver for the Filer API.
//!
//! This crate will implement [`filer_api::Driver`] over `ar` archives:
//! Debian packages (`.deb`, an ar archive holding compressed tarballs, which
//! `filer-tar` can then open in turn) and static libraries (`.a`). It does
//! not export any items yet.
//!
//! **Read-only, permanently.** Filer reads archives as directories; it
//! never creates or modifies them, and never will: writing archives is out
//! of scope for this project. Use a dedicated archiving library for that.
//!
//! The plan (see `DESIGN.md` §17.7): ar archives are flat, a sequence of
//! 60-byte headers each followed by its member's data, so one pass over a
//! [`filer_api::ReadAt`] source builds the index. Both the GNU and BSD
//! long-name schemes will be read, symbol tables hidden, and the shared
//! archive rules applied: names confined to the root, later duplicates
//! winning, and strict limits.
//!
//! It needs no OS and builds with `#![no_std]`; the `std` feature, enabled
//! by default, forwards to `filer-api`. `all` currently enables nothing.

#![no_std]
#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]

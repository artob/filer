// This is free and unencumbered software released into the public domain.

//! Read-only RAR archive driver for the Filer API.
//!
//! This crate reserves the name for a driver over RAR archives (versions 4
//! and 5) and does not export any items yet.
//!
//! **Read-only, permanently.** Filer reads archives as directories; it
//! never creates or modifies them, and never will: writing archives is out
//! of scope for this project. Use a dedicated archiving library for that.
//!
//! An implementation needs care with licensing. RAR's compression is
//! proprietary, and the license of the official `unrar` source forbids
//! using it to recreate the RAR compression algorithm, which keeps it out of
//! this public-domain project. A reader would have to be a clean-room
//! implementation of decompression only, as libarchive's is. RAR archives
//! are also often "solid" (members compressed as one stream), which makes
//! reading a member at an offset costly. See `DESIGN.md` §17.7.
//!
//! It needs no OS and builds with `#![no_std]`; the `std` feature, enabled
//! by default, forwards to `filer-api`. `all` currently enables nothing.

#![no_std]
#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]

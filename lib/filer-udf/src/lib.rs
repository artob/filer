// This is free and unencumbered software released into the public domain.

//! Read-only UDF file system driver for the Filer API.
//!
//! This crate reserves the name for a driver over UDF (Universal Disk
//! Format, ECMA-167 as profiled by OSTA) volumes, such as DVD and Blu-ray
//! images, read from any [`filer_api::ReadAt`] source, and does not export
//! any items yet. Writing is out of scope for now.
//!
//! What a reader must handle: the volume recognition sequence and the
//! anchor volume descriptor pointer (at sector 256, or at the end of the
//! volume); the volume descriptor sequence and its partition and logical
//! volume descriptors; partition maps, physical, virtual (VAT), sparable,
//! and metadata partitions; file entries and extended file entries, with
//! data embedded or in short, long, or extended allocation descriptors;
//! file identifier descriptors; names as OSTA compressed Unicode (CS0,
//! 8- or 16-bit); UDF revisions 1.02 to 2.60; and sector sizes other
//! than 2048. Many discs also carry ISO 9660 as a bridge; that format
//! would be its own driver. See `DESIGN.md` §17.12.
//!
//! **Development.** Fixtures are made with `mkudffs`, in the `udftools`
//! package on Debian and Ubuntu; the differential test compares with the
//! Linux kernel's `udf` driver, a module in Ubuntu's standard
//! `linux-modules-$(uname -r)` package.
//!
//! Volumes are untrusted input: like `filer-fat`, the driver will yield
//! errors, never a panic, a hang, or unbounded memory use, and every codec
//! and checksum will be pure Rust. It needs no OS and builds with
//! `#![no_std]`; the `std` feature, enabled by default, forwards to
//! `filer-api`. `all` currently enables nothing.

#![no_std]
#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]

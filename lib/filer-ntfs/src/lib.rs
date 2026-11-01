// This is free and unencumbered software released into the public domain.

//! Read-only NTFS file system driver for the Filer API.
//!
//! This crate reserves the name for a driver over NTFS volumes, read from
//! any [`filer_api::ReadAt`] source, and does not export any items yet.
//! Writing is out of scope for now.
//!
//! What a reader must handle: the boot sector and the master file table
//! (MFT), whose records carry update sequence arrays (fixups) to check
//! and apply; resident and non-resident attributes, the latter as data
//! runs; directories as B+ tree indexes (`$INDEX_ROOT` and
//! `$INDEX_ALLOCATION`); names in UTF-16, with case-insensitive matching
//! by the volume's own `$UpCase` table, and the DOS (8.3) namespace
//! alongside the Win32 one; hard links; sparse and LZNT1-compressed
//! data, decompressed in pure Rust; alternate data streams; reparse
//! points (symbolic links and junctions, mapped to symlinks where they
//! can be confined); and a volume marked dirty, whose `$LogFile` has not
//! been replayed. See `DESIGN.md` §17.12.
//!
//! **Development.** Fixtures are made with `mkntfs` and written through
//! the FUSE driver `ntfs-3g`, both in the `ntfs-3g` package on Debian and
//! Ubuntu; the differential test compares with the Linux kernel's `ntfs3`
//! driver, a module that Ubuntu ships in `linux-modules-extra-$(uname -r)`
//! (kept current by `linux-generic`).
//!
//! Volumes are untrusted input: like `filer-fat`, the driver will yield
//! errors, never a panic, a hang, or unbounded memory use, and every codec
//! and checksum will be pure Rust. It needs no OS and builds with
//! `#![no_std]`; the `std` feature, enabled by default, forwards to
//! `filer-api`. `all` currently enables nothing.

#![no_std]
#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]

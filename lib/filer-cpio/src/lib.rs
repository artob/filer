// This is free and unencumbered software released into the public domain.

//! cpio archive driver for the Filer API.
//!
//! [`Cpio`] serves the contents of a cpio archive, read-only, as a
//! [`Dir`](filer_api::Dir): open the archive from any
//! [`ReadAt`](filer_api::ReadAt) source, such as a `File` from another
//! driver or bytes in memory, then use it like any other directory. That
//! covers uncompressed Linux initramfs images and the payloads of RPM
//! packages, once decompressed. Paths stay confined to the archive, and a
//! hostile archive fails to open rather than exposing anything outside it
//! (see [`Cpio`] and [`Limits`]).
//!
//! **Read-only, permanently.** Filer reads archives as directories; it
//! never creates or modifies them, and never will: writing archives is out
//! of scope for this project. Use a dedicated archiving library for that.
//!
//! It needs no OS and builds with `#![no_std]`; the `std` feature, enabled
//! by default, forwards to `filer-api`. `all` currently enables nothing.
//!
//! # Examples
//!
//! An initramfs image, as the Linux kernel unpacks it: an early archive of
//! microcode, then the main one, which read as one tree. Its hardlinks are
//! one file, and its symlinks resolve within it:
//!
//! ```
//! use filer_api::MetaMask;
//! use filer_cpio::Cpio;
//!
//! # filer_spec::block_on(async {
//! let image: &'static [u8] = include_bytes!("../tests/data/initramfs.img");
//! let root = Cpio::open(image).await?;
//! // An archive lists names in byte order.
//! let names = root.names().await?;
//! assert_eq!(names, ["dir", "empty", "fifo", "hard1", "kernel", "link"]);
//!
//! let microcode = root.open_file("kernel/x86/microcode/GenuineIntel.bin").await?;
//! assert_eq!(microcode.read_to_end().await?, b"not really microcode\n");
//! let hard1 = root.metadata("hard1", MetaMask::ID).await?;
//! let hard2 = root.metadata("dir/hard2", MetaMask::ID).await?;
//! assert_eq!(hard1.id, hard2.id);
//! let link = root.open_file("link").await?;
//! assert_eq!(link.read_to_end().await?, b"Hello, world!\n");
//! # Ok::<(), filer_api::Error>(())
//! # }).unwrap();
//! ```

#![no_std]
#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
// No input may make this crate panic (DESIGN.md D47).
#![deny(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::let_underscore_must_use,
    clippy::panic,
    clippy::string_slice,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable,
    clippy::unwrap_used,
    unused_must_use
)]
// Tests may panic: that is how they fail.
#![cfg_attr(
    test,
    allow(
        clippy::arithmetic_side_effects,
        clippy::as_conversions,
        clippy::expect_used,
        clippy::indexing_slicing,
        clippy::let_underscore_must_use,
        clippy::panic,
        clippy::unreachable,
        clippy::unwrap_used
    )
)]

extern crate alloc;

mod codec;

mod header;

mod index;

/// Re-exported from [`filer_api::archive`], where all archive drivers share
/// it.
pub use filer_api::archive::Limits;

mod cpio;
pub use cpio::*;

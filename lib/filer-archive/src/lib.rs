// This is free and unencumbered software released into the public domain.

//! Read-only archives of any supported format for the Filer API.
//!
//! [`Archive`] opens an archive in whichever format it is in, picked from
//! its file name, and serves it as a [`Dir`](filer_api::Dir), read-only,
//! through that format's own driver. It is the one dependency a program
//! needs to read archives whatever their format, as `filer-system` is for
//! the native file system:
//!
//! | Format | Extensions | Feature | Driver |
//! | --- | --- | --- | --- |
//! | ar | `.a`, `.ar`, `.deb`, `.lib`, `.udeb` | `ar` | `filer-ar` |
//! | cpio | `.cpio` | `cpio` | `filer-cpio` |
//! | TAR | `.tar` | `tar` | `filer-tar` |
//! | ZIP | `.zip`, `.jar`, `.apk`, `.whl`, `.epub`, `.docx`, ... | `zip`, `deflate` | `filer-zip` |
//!
//! Each format's feature enables its driver, which is then also
//! re-exported here as a module (as [`zip`] for `filer-zip`), for what is
//! particular to it; `deflate` lets `filer-zip` decode Deflate, the method
//! most ZIP archives use. `rar` and `7z` enable `filer-rar` and
//! `filer-7z` (as `sevenz`, since a module's name cannot begin with a
//! digit), which read nothing yet, so [`Format`] names neither until they
//! do. `all`, enabled by default, enables every one.
//! Without a format's feature, [`Format`] still names it, and opening it
//! fails with [`Unsupported`](filer_api::ErrorKind::Unsupported).
//!
//! The format comes from the name's extension for now; detecting it from
//! the archive's first bytes is planned. Compressed tarballs (`.tar.gz`
//! and the like) are not read yet.
//!
//! **Read-only, permanently.** Filer reads archives as directories; it
//! never creates or modifies them, and never will: writing archives is out
//! of scope for this project. Use a dedicated archiving library for that.
//!
//! It needs no OS and builds with `#![no_std]`; the `std` feature, enabled
//! by default, forwards to `filer-api` and the drivers.
//!
//! # Examples
//!
//! Whatever the format, an archive reads the same way:
//!
//! ```
//! use filer_archive::Archive;
//!
//! # #[cfg(all(feature = "tar", feature = "zip"))]
//! # filer_spec::block_on(async {
//! for (name, bytes) in [
//!     ("hello.tar", &include_bytes!("../tests/data/hello.tar")[..]),
//!     ("hello.zip", &include_bytes!("../tests/data/hello.zip")[..]),
//! ] {
//!     let root = Archive::open(bytes, name).await?;
//!     let file = root.open_file("hello/world.txt").await?;
//!     assert_eq!(file.read_to_end().await?, b"Hello, world!\n");
//! }
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
// The crate docs link to items that exist only with some features.
#![cfg_attr(not(feature = "zip"), allow(rustdoc::broken_intra_doc_links))]

extern crate alloc;

mod archive;
pub use archive::*;

mod format;
pub use format::*;

/// Re-exported from [`filer_pack`], where all archive drivers share it.
pub use filer_pack::Limits;

#[cfg(feature = "7z")]
#[doc(no_inline)]
pub use filer_7z as sevenz;

#[cfg(feature = "ar")]
#[doc(no_inline)]
pub use filer_ar as ar;

#[cfg(feature = "cpio")]
#[doc(no_inline)]
pub use filer_cpio as cpio;

#[cfg(feature = "rar")]
#[doc(no_inline)]
pub use filer_rar as rar;

#[cfg(feature = "tar")]
#[doc(no_inline)]
pub use filer_tar as tar;

#[cfg(feature = "zip")]
#[doc(no_inline)]
pub use filer_zip as zip;

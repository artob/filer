// This is free and unencumbered software released into the public domain.

//! Cross-platform Filer API over the native filesystem driver.
//!
//! This is the one dependency a program needs to use Filer in place of
//! `std::fs`. It re-exports the whole [`filer_api`] crate, so `Dir`,
//! `File`, `rel!`, and the rest are available from here, and adds
//! [`System`], a driver that delegates to the platform's native one:
//!
//! | Platform | Driver |
//! | --- | --- |
//! | Linux, macOS, the BSDs, other Unix | `filer-posix` |
//! | Windows | `filer-windows` (not implemented yet) |
//! | WASI | `filer-wasi` (not implemented yet) |
//! | Browsers (`wasm32-unknown-unknown`) | `filer-whatwg` (not implemented yet) |
//!
//! Where no driver exists yet, `System` still compiles, so that portable
//! code builds everywhere, but every operation fails with
//! [`ErrorKind::Unsupported`]. Nothing is emulated.
//!
//! Start with [`open_ambient`]. The crate builds with `#![no_std]`, in which
//! case it is just the re-export; `System` and `open_ambient` need the
//! `std` feature, enabled by default. `all` currently enables nothing.

#![no_std]
#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
// The crate docs link to items that exist only with `std`.
#![cfg_attr(not(feature = "std"), allow(rustdoc::broken_intra_doc_links))]

#[cfg(feature = "std")]
extern crate std;

#[doc(no_inline)]
pub use filer_api::*;

#[cfg(feature = "std")]
mod system;
#[cfg(feature = "std")]
pub use system::*;

#[cfg(all(feature = "std", not(unix)))]
mod unsupported;

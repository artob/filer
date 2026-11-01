// This is free and unencumbered software released into the public domain.

//! Executable specification of the Filer driver contract.
//!
//! Every driver, in this repository or not, should pass this specification
//! in its tests: it is the contract of [`filer_api::Driver`] written as
//! checks. The driver's test supplies a setup function that creates a
//! described [`Tree`] and returns its root [`Dir`](filer_api::Dir); [`run`]
//! then checks opening, reading, errors, symlink policies and escapes,
//! loops, metadata, extended attributes, links, listings, rights, and
//! isolation between driver instances, through the frontend. A [`Profile`] says what the
//! environment can create, so cases skip rather than fail.
//!
//! So far the specification covers read operations only.
//!
//! # Examples
//!
//! ```
//! use filer_api::{Dir, Driver};
//! use filer_spec::{Profile, Tree};
//!
//! fn check<D: Driver>(setup: impl FnMut(&Tree) -> Dir<D>) {
//!     filer_spec::run_blocking(Profile::default(), setup).assert_passed();
//! }
//! ```
//!
//! The crate builds with `#![no_std]`; the `std` feature, enabled by
//! default, lets [`block_on`] sleep instead of spin. `all` currently
//! enables nothing.

#![no_std]
#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]

extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

mod block_on;
pub use block_on::*;

mod cases;

mod profile;
pub use profile::*;

mod report;
pub use report::*;

mod run;
pub use run::*;

mod tree;
pub use tree::*;

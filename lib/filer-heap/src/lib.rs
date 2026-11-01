// This is free and unencumbered software released into the public domain.

//! In-memory driver for the Filer API.
//!
//! [`Heap`] is a filesystem held entirely in memory: the reference
//! implementation of the driver contract and the test bed for code built on
//! [`filer_api`]. It needs no OS and builds with `#![no_std]`, using only
//! `alloc`; the `std` feature, enabled by default, forwards to `filer-api`.
//! `all` currently enables nothing.
//!
//! So far it is read-only: a [`HeapBuilder`] fixes its contents at
//! construction, and it answers the read operations of [`filer_api::Op`].
//!
//! # Examples
//!
//! ```
//! use filer_api::Driver;
//! use filer_heap::Heap;
//!
//! let heap = Heap::builder()
//!     .file("config/app.toml", "debug = true")
//!     .build();
//! let root = heap.root();
//! assert_eq!(root.driver(), heap.id());
//! ```

#![no_std]
#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]

extern crate alloc;

mod heap;
pub use heap::*;

mod heap_builder;
pub use heap_builder::*;

// This is free and unencumbered software released into the public domain.

//! Shared machinery for read-only archive drivers.
//!
//! Archive formats differ in their headers, but once indexed they are all
//! the same thing: a [`Tree`](crate::tree::Tree) of directories, links,
//! and members whose bytes lie at known offsets in a [`ReadAt`] source.
//! This module holds everything but the header parsing, so that each format
//! crate (`filer-tar`, `filer-zip`, `filer-ar`, `filer-cpio`, ...) is just a
//! parser feeding an [`Index`]:
//!
//! - [`Index`] builds the tree under the shared rules: member names are
//!   checked so that none can escape the archive root, directories implied
//!   by names are created, the later of duplicate members wins, and
//!   hardlinks share one node;
//! - [`Archive`] serves the finished tree as a read-only [`Driver`];
//! - [`Codec`] and [`Decoder`] decode compressed members, which
//!   [`Member`] records with their format's codec;
//! - [`Limits`] bounds what an untrusted archive may make the index hold;
//! - [`ArchiveError`] names the format, header offset, and reason of a
//!   failed open.
//!
//! Archive drivers are read-only, permanently: Filer reads archives as
//! directories and never creates or modifies them.
//!
//! [`ReadAt`]: crate::ReadAt
//! [`Driver`]: crate::Driver

mod archive_error;
pub use archive_error::*;

mod codec;
pub use codec::*;

mod driver;
pub use driver::*;

mod index;
pub use index::*;

mod lock;

mod limits;
pub use limits::*;

mod member;
pub use member::*;

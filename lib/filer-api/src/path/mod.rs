// This is free and unencumbered software released into the public domain.

//! Lexical names and relative paths.
//!
//! A [`Name`] is one path component; a [`RelPath`] is zero or more names
//! joined by `/`, always relative to a directory handle and never containing
//! `..`. Each has an owned form, [`NameBuf`] and [`RelPathBuf`].
//!
//! Every name a caller can type is valid UTF-8 and is checked here, before
//! any driver sees it. The checks are purely lexical: a [`Name`] that passes
//! them may still be refused by a particular directory, for example because
//! of its length or a reserved word, and that is reported by the driver.
//!
//! This module depends only on `core` and `alloc`, never on the rest of the
//! crate, so that it can later move into a separate crate without breaking
//! users.

mod components;
pub use components::*;

mod macros;

mod name;
pub use name::*;

mod name_buf;
pub use name_buf::*;

mod name_error;
pub use name_error::*;

mod rel_path;
pub use rel_path::*;

mod rel_path_buf;
pub use rel_path_buf::*;

mod rel_path_error;
pub use rel_path_error::*;

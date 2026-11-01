// This is free and unencumbered software released into the public domain.

//! Lexical names and relative paths.
//!
//! Every name a caller can type is valid UTF-8 and is checked here, before
//! any driver sees it. The checks are purely lexical: a [`Name`] that passes
//! them may still be refused by a particular directory, for example because
//! of its length or a reserved word, and that is reported by the driver.
//!
//! This module depends only on `core` and `alloc`, never on the rest of the
//! crate, so that it can later move into a separate crate without breaking
//! users.

mod name;
pub use name::*;

mod name_buf;
pub use name_buf::*;

mod name_error;
pub use name_error::*;

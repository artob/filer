// This is free and unencumbered software released into the public domain.

use crate::{ErasedBufMut, Handle, OpKind, XattrEntry};

/// An operation on extended attributes, carried by
/// [`Op::Xattr`](crate::Op::Xattr).
///
/// Operations go through an open handle (as `fgetxattr` does), never a
/// path, so the file cannot be swapped between resolving and reading.
/// Only reading exists so far. Drivers without extended attributes fail
/// with [`Unsupported`](crate::ErrorKind::Unsupported).
#[derive(Debug)]
#[non_exhaustive]
pub enum XattrOp {
    /// Reads the value of the attribute `name` of `handle` into the spare
    /// capacity of `buf`, completing with
    /// [`Output::Xattr`](crate::Output::Xattr) and returning `buf` in the
    /// completion.
    ///
    /// The output is the value's whole length, or `None` if the attribute
    /// does not exist. The driver copies as much of the value as fits, so a
    /// length above the spare capacity means the value was cut short.
    Get {
        /// The file or directory.
        handle: Handle,
        /// The attribute.
        name: XattrEntry,
        /// The buffer to append to.
        buf: ErasedBufMut,
    },

    /// Lists the attributes of `handle`, completing with
    /// [`Output::XattrNames`](crate::Output::XattrNames).
    List {
        /// The file or directory.
        handle: Handle,
    },
}

impl XattrOp {
    /// Returns the kind of this operation.
    #[must_use]
    pub fn kind(&self) -> OpKind {
        match self {
            Self::Get { .. } => OpKind::GetXattr,
            Self::List { .. } => OpKind::ListXattrs,
        }
    }
}

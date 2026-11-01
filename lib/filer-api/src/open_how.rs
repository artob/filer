// This is free and unencumbered software released into the public domain.

use crate::{HandleKind, Resolve};

/// How an [`Op::Open`](crate::Op::Open) opens its path.
///
/// Only read-only opens exist so far: a directory, or a regular file for
/// reading. Opening a file that turns out to be a FIFO, socket, or device
/// fails with [`NotRegularFile`](crate::ErrorKind::NotRegularFile), and
/// opening the wrong kind fails with
/// [`NotADirectory`](crate::ErrorKind::NotADirectory) or
/// [`IsADirectory`](crate::ErrorKind::IsADirectory).
///
/// # Examples
///
/// ```
/// use filer_api::{HandleKind, OpenHow, Symlinks};
///
/// let mut how = OpenHow::file();
/// how.resolve.symlinks = Symlinks::Deny;
/// assert_eq!(how.kind, HandleKind::File);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct OpenHow {
    /// What to open.
    pub kind: HandleKind,

    /// How to resolve the path.
    pub resolve: Resolve,
}

impl OpenHow {
    /// Opens a directory, with the default resolution policy.
    #[must_use]
    pub fn dir() -> Self {
        Self {
            kind: HandleKind::Dir,
            resolve: Resolve::default(),
        }
    }

    /// Opens a regular file for reading, with the default resolution
    /// policy.
    #[must_use]
    pub fn file() -> Self {
        Self {
            kind: HandleKind::File,
            resolve: Resolve::default(),
        }
    }
}

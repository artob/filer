// This is free and unencumbered software released into the public domain.

use crate::{DirCookie, Entry, Handle, LinkTarget, Metadata, XattrEntry};
use alloc::vec::Vec;

/// The successful result of an [`Op`](crate::Op), one variant per kind of
/// result.
///
/// The frontend knows which variant to expect from the operation it
/// submitted; any other variant is a driver bug.
#[derive(Debug)]
#[non_exhaustive]
pub enum Output {
    /// The operation succeeded with nothing to return.
    Done,

    /// A file or directory was opened.
    Opened(Handle),

    /// This many bytes were read into the returned buffer.
    Read(usize),

    /// The requested metadata.
    Metadata(Metadata),

    /// A batch of directory entries.
    Entries {
        /// The entries, in no particular order.
        entries: Vec<Entry>,
        /// Where to continue, or `None` if the listing is complete.
        next: Option<DirCookie>,
    },

    /// A symlink's target.
    LinkTarget(LinkTarget),

    /// The whole length of an extended attribute's value, of which as much
    /// as fit was read into the returned buffer, or `None` if the
    /// attribute does not exist.
    Xattr(Option<u64>),

    /// The names of a file's extended attributes, in no particular order.
    XattrNames(Vec<XattrEntry>),
}

// This is free and unencumbered software released into the public domain.

use crate::{Timestamp, archive::Stored};

/// Where a regular member's bytes lie in an archive and how they are
/// encoded, with its modification time if the format records one.
///
/// `C` is the format's [`Codec`](crate::archive::Codec); formats without
/// compression use the default, [`Stored`].
///
/// # Examples
///
/// ```
/// use filer_api::archive::Member;
///
/// let member = Member::new(512, 14);
/// assert_eq!((member.offset, member.size, member.packed), (512, 14, 14));
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct Member<C = Stored> {
    /// The offset of the member's first byte in the archive.
    pub offset: u64,

    /// The member's length in bytes, decoded.
    pub size: u64,

    /// The member's length in the archive: compressed, or the same as
    /// `size` if stored as is.
    pub packed: u64,

    /// How the member is encoded.
    pub codec: C,

    /// The member's modification time, if known.
    pub modified: Option<Timestamp>,
}

impl Member {
    /// Returns a member of `size` bytes at `offset`, stored as is, with no
    /// known modification time.
    #[must_use]
    pub const fn new(offset: u64, size: u64) -> Self {
        Self::encoded(offset, size, size, Stored)
    }
}

impl<C> Member<C> {
    /// Returns a member at `offset` of `packed` bytes encoded by `codec`,
    /// `size` bytes once decoded, with no known modification time.
    #[must_use]
    pub const fn encoded(offset: u64, packed: u64, size: u64, codec: C) -> Self {
        Self {
            offset,
            size,
            packed,
            codec,
            modified: None,
        }
    }

    /// Returns this member with its modification time.
    #[must_use]
    pub fn with_modified(mut self, modified: Option<Timestamp>) -> Self {
        self.modified = modified;
        self
    }
}

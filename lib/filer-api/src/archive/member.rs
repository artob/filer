// This is free and unencumbered software released into the public domain.

use crate::Timestamp;

/// Where a regular member's bytes lie in an archive, stored as is (not
/// compressed), with its modification time if the format records one.
///
/// # Examples
///
/// ```
/// use filer_api::archive::Member;
///
/// let member = Member::new(512, 14);
/// assert_eq!((member.offset, member.size), (512, 14));
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct Member {
    /// The offset of the member's first byte in the archive.
    pub offset: u64,

    /// The member's length in bytes.
    pub size: u64,

    /// The member's modification time, if known.
    pub modified: Option<Timestamp>,
}

impl Member {
    /// Returns a member of `size` bytes at `offset`, with no known
    /// modification time.
    #[must_use]
    pub const fn new(offset: u64, size: u64) -> Self {
        Self {
            offset,
            size,
            modified: None,
        }
    }

    /// Returns this member with its modification time.
    #[must_use]
    pub const fn with_modified(mut self, modified: Option<Timestamp>) -> Self {
        self.modified = modified;
        self
    }
}

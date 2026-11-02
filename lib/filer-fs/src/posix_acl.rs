// This is free and unencumbered software released into the public domain.

//! POSIX ACLs in the form Linux's `getxattr` returns them, as
//! `system.posix_acl_access` and `system.posix_acl_default`: a
//! little-endian version, 2, then each entry's tag, permissions, and id,
//! in eight bytes. The id is a named user's or group's; the other entries
//! (the owner, owning group, mask, and others) have `u32::MAX`.
//!
//! File systems store ACLs in forms of their own, such as ext4's, whose
//! entries without an id are shorter, and XFS's, big-endian. A driver
//! decodes its form, and encodes Linux's with an [`Encoder`], which checks
//! each entry's tag.
//!
//! # Examples
//!
//! ```
//! use filer_fs::posix_acl::{self, Encoder};
//!
//! let mut acl = Encoder::new();
//! acl.push(posix_acl::USER_OBJ, 6, 0).unwrap();
//! acl.push(posix_acl::USER, 4, 1000).unwrap();
//! assert_eq!(acl.push(0x40, 0, 0), None);
//! let bytes = acl.finish();
//! assert_eq!(bytes.len(), 4 + 2 * 8);
//! assert_eq!(bytes[16..20], 1000u32.to_le_bytes());
//! ```

use alloc::vec::Vec;

/// The tag of the owner's entry.
pub const USER_OBJ: u16 = 0x01;
/// The tag of a named user's entry.
pub const USER: u16 = 0x02;
/// The tag of the owning group's entry.
pub const GROUP_OBJ: u16 = 0x04;
/// The tag of a named group's entry.
pub const GROUP: u16 = 0x08;
/// The tag of the mask's entry.
pub const MASK: u16 = 0x10;
/// The tag of the entry for others.
pub const OTHER: u16 = 0x20;

/// The id of entries that name no user or group.
const NO_ID: u32 = u32::MAX;

/// Returns `true` if entries of `tag` name a user or a group, and so have
/// an id.
#[must_use]
pub const fn is_named(tag: u16) -> bool {
    matches!(tag, USER | GROUP)
}

/// Encodes an ACL, entry by entry, as Linux's `getxattr` returns it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Encoder(Vec<u8>);

impl Encoder {
    /// Returns an encoder of an ACL with no entries yet.
    #[must_use]
    pub fn new() -> Self {
        Self(2u32.to_le_bytes().to_vec())
    }

    /// Appends the entry of `tag`, with permissions `perm` and, if the tag
    /// [`is_named`], the user or group `id`; other entries ignore `id`.
    /// Returns `None`, appending nothing, if `tag` is unknown.
    #[must_use]
    pub fn push(&mut self, tag: u16, perm: u16, id: u32) -> Option<()> {
        let id = match tag {
            USER | GROUP => id,
            USER_OBJ | GROUP_OBJ | MASK | OTHER => NO_ID,
            _ => return None,
        };
        self.0.extend_from_slice(&tag.to_le_bytes());
        self.0.extend_from_slice(&perm.to_le_bytes());
        self.0.extend_from_slice(&id.to_le_bytes());
        Some(())
    }

    /// Returns the encoded ACL.
    #[must_use]
    pub fn finish(self) -> Vec<u8> {
        self.0
    }
}

impl Default for Encoder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_ids_only_for_named_entries() {
        let mut acl = Encoder::new();
        for (tag, perm) in [(USER_OBJ, 7), (GROUP, 5), (MASK, 5), (OTHER, 0)] {
            acl.push(tag, perm, 42).unwrap();
        }
        let ids: Vec<u32> = acl.finish()[4..]
            .as_chunks::<8>()
            .0
            .iter()
            .map(|entry| u32::from_le_bytes(entry[4..].try_into().unwrap()))
            .collect();
        assert_eq!(ids, [NO_ID, 42, NO_ID, NO_ID]);
    }

    #[test]
    fn refuses_unknown_tags() {
        let mut acl = Encoder::new();
        assert_eq!(acl.push(0, 0, 0), None);
        assert_eq!(acl.push(0x03, 0, 0), None);
        assert_eq!(acl.finish(), 2u32.to_le_bytes());
    }
}

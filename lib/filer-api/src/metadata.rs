// This is free and unencumbered software released into the public domain.

use crate::{FileId, FileKind, MetaMask, Timestamp};

/// Metadata about a filesystem entry.
///
/// Every field is optional: `None` means the field was not requested in
/// the [`MetaMask`] or the driver cannot provide it, never zero. Drivers
/// build values from [`Metadata::default`] and set fields directly, since
/// the struct is `#[non_exhaustive]`.
///
/// # Examples
///
/// ```
/// use filer_api::{FileKind, MetaMask, Metadata};
///
/// let mut metadata = Metadata::default();
/// metadata.kind = Some(FileKind::File);
/// metadata.len = Some(42);
/// assert_eq!(metadata.mask(), MetaMask::KIND | MetaMask::LEN);
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct Metadata {
    /// The entry's type.
    pub kind: Option<FileKind>,

    /// The size in bytes: the file's length, or for a symlink the length of
    /// its target. For directories the value is filesystem-specific.
    pub len: Option<u64>,

    /// The last modification of the contents (`mtime`).
    pub modified: Option<Timestamp>,

    /// The last change to the contents or metadata (POSIX `ctime`). It is
    /// set by the filesystem and cannot be set by users.
    pub changed: Option<Timestamp>,

    /// The creation time (birth time), where the filesystem records it.
    pub created: Option<Timestamp>,

    /// The last access, often coarse or disabled (`noatime`, `relatime`).
    pub accessed: Option<Timestamp>,

    /// The file's identity within its filesystem.
    pub id: Option<FileId>,
}

impl Metadata {
    /// Returns the mask of the fields that are present.
    #[must_use]
    pub fn mask(&self) -> MetaMask {
        let fields = [
            (MetaMask::KIND, self.kind.is_some()),
            (MetaMask::LEN, self.len.is_some()),
            (MetaMask::MTIME, self.modified.is_some()),
            (MetaMask::CTIME, self.changed.is_some()),
            (MetaMask::BTIME, self.created.is_some()),
            (MetaMask::ATIME, self.accessed.is_some()),
            (MetaMask::ID, self.id.is_some()),
        ];
        let mut mask = MetaMask::empty();
        for (bit, present) in fields {
            mask.set(bit, present);
        }
        mask
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_nothing() {
        assert_eq!(Metadata::default().mask(), MetaMask::empty());
    }

    #[test]
    fn reports_every_present_field() {
        let time = Timestamp::new(0, 0);
        let metadata = Metadata {
            kind: Some(FileKind::Dir),
            len: Some(0),
            modified: Some(time),
            changed: Some(time),
            created: Some(time),
            accessed: Some(time),
            id: Some(FileId::new(1, 2)),
        };
        assert_eq!(metadata.mask(), MetaMask::all());
    }
}

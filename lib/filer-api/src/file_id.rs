// This is free and unencumbered software released into the public domain.

/// The identity of a file within its filesystem, as reported by the driver.
///
/// A `FileId` pairs a volume identifier with a 128-bit file identifier:
/// `(st_dev, st_ino)` on POSIX, `FILE_ID_INFO` on Windows, where ReFS file
/// ids are 128-bit. It is opaque and supports only comparison and hashing.
///
/// Equal ids from the same driver denote the same file while it exists, but
/// ids are not a perfect identity test: btrfs subvolumes, overlayfs, and
/// NFS can reuse or alias them, and a deleted file's id may be reused.
/// Comparing two open handles is the robust check. Ids are not
/// serializable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileId {
    volume: u64,
    file: u128,
}

impl FileId {
    /// Returns the id of file `file` on volume `volume`, for drivers.
    #[must_use]
    pub const fn new(volume: u64, file: u128) -> Self {
        Self { volume, file }
    }
}

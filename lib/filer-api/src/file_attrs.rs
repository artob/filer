// This is free and unencumbered software released into the public domain.

bitflags::bitflags! {
    /// File flags, as reported in [`Metadata::attrs`](crate::Metadata::attrs):
    /// Linux `chattr` and BSD `chflags` flags, and Windows file attributes.
    ///
    /// A driver reports the flags its filesystem has, and the set says
    /// nothing about the others. Windows' read-only attribute is a
    /// permission, not a flag, and its directory and reparse-point
    /// attributes are the [`FileKind`](crate::FileKind).
    ///
    /// More flags are added as drivers need them; existing bit values never
    /// change.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct FileAttrs: u32 {
        /// Hidden from ordinary listings (Windows, FAT, and NTFS; macOS
        /// `UF_HIDDEN`).
        const HIDDEN = 1 << 0;
        /// Used by the operating system (Windows, FAT, and NTFS).
        const SYSTEM = 1 << 1;
        /// Changed since last backed up (Windows, FAT, and NTFS).
        const ARCHIVE = 1 << 2;
        /// Cannot be changed, renamed, linked to, or removed (Linux `i`,
        /// BSD `schg` and `uchg`).
        const IMMUTABLE = 1 << 3;
        /// Can only be appended to (Linux `a`, BSD `sappnd` and `uappnd`).
        const APPEND_ONLY = 1 << 4;
        /// Not copied on write (btrfs `C`); only takes effect on an empty
        /// file.
        const NOCOW = 1 << 5;
        /// Compressed by the filesystem (Linux `c`, NTFS, APFS).
        const COMPRESSED = 1 << 6;
        /// Encrypted by the filesystem (Linux `E`, NTFS EFS).
        const ENCRYPTED = 1 << 7;
        /// Protected by fs-verity (Linux `V`).
        const VERITY = 1 << 8;
        /// Accessed directly, without the page cache (Linux `x`).
        const DAX = 1 << 9;
        /// Sparse: unwritten ranges take no space (NTFS, ReFS).
        const SPARSE = 1 << 10;
        /// Moved to offline storage (Windows).
        const OFFLINE = 1 << 11;
        /// Its contents are not stored locally and are fetched when
        /// accessed, as for cloud placeholders (macOS `SF_DATALESS`,
        /// Windows recall on open or on data access).
        const DATALESS = 1 << 12;
    }
}

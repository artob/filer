// This is free and unencumbered software released into the public domain.

/// The type of a filesystem entry.
///
/// Directory listings may not know an entry's kind (POSIX `DT_UNKNOWN`), so
/// they report `Option<FileKind>`; metadata always knows it when requested.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FileKind {
    /// A regular file.
    File,

    /// A directory.
    Dir,

    /// A symbolic link (on Windows, a symlink or junction reparse point).
    Symlink,

    /// A named pipe (FIFO).
    Fifo,

    /// A Unix domain socket.
    Socket,

    /// A block device.
    BlockDevice,

    /// A character device.
    CharDevice,

    /// Any other kind, such as a BSD whiteout.
    Other,
}

impl FileKind {
    /// Returns the kind that the type bits (`S_IFMT`) of a Unix `mode`
    /// name, as Unix file systems and archives record them, or `None` if
    /// they name none.
    ///
    /// # Examples
    ///
    /// ```
    /// use filer_api::FileKind;
    ///
    /// assert_eq!(FileKind::from_unix_mode(0o100_644), Some(FileKind::File));
    /// assert_eq!(FileKind::from_unix_mode(0o040_755), Some(FileKind::Dir));
    /// assert_eq!(FileKind::from_unix_mode(0o000_644), None);
    /// ```
    #[must_use]
    pub const fn from_unix_mode(mode: u32) -> Option<Self> {
        Some(match mode & 0o170_000 {
            0o010_000 => Self::Fifo,
            0o020_000 => Self::CharDevice,
            0o040_000 => Self::Dir,
            0o060_000 => Self::BlockDevice,
            0o100_000 => Self::File,
            0o120_000 => Self::Symlink,
            0o140_000 => Self::Socket,
            _ => return None,
        })
    }

    /// Returns `true` for a regular file.
    #[must_use]
    pub const fn is_file(self) -> bool {
        matches!(self, Self::File)
    }

    /// Returns `true` for a directory.
    #[must_use]
    pub const fn is_dir(self) -> bool {
        matches!(self, Self::Dir)
    }

    /// Returns `true` for a symbolic link.
    #[must_use]
    pub const fn is_symlink(self) -> bool {
        matches!(self, Self::Symlink)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_unix_modes_by_their_type_bits() {
        assert_eq!(FileKind::from_unix_mode(0o040_755), Some(FileKind::Dir));
        assert_eq!(FileKind::from_unix_mode(0o120_777), Some(FileKind::Symlink));
        assert_eq!(FileKind::from_unix_mode(0o140_000), Some(FileKind::Socket));
        assert_eq!(FileKind::from_unix_mode(0o010_000), Some(FileKind::Fifo));
        assert_eq!(
            FileKind::from_unix_mode(0o020_644),
            Some(FileKind::CharDevice)
        );
        assert_eq!(
            FileKind::from_unix_mode(0o060_644),
            Some(FileKind::BlockDevice)
        );
        assert_eq!(FileKind::from_unix_mode(0o000_644), None);
        assert_eq!(FileKind::from_unix_mode(0o170_000), None);
    }
}

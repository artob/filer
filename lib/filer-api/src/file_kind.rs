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

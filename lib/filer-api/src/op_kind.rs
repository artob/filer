// This is free and unencumbered software released into the public domain.

/// The kind of driver operation an [`Error`](crate::Error) came from.
///
/// Only read operations exist so far; others are added with their
/// operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum OpKind {
    /// Opening a file or directory.
    Open,

    /// Closing a handle.
    Close,

    /// Reading file contents.
    Read,

    /// Reading metadata.
    Stat,

    /// Listing a directory.
    ReadDir,

    /// Reading a symlink's target.
    ReadLink,
}

impl OpKind {
    /// Returns the operation's name in `snake_case`, as used in messages.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Close => "close",
            Self::Read => "read",
            Self::Stat => "stat",
            Self::ReadDir => "read_dir",
            Self::ReadLink => "read_link",
        }
    }
}

impl core::fmt::Display for OpKind {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

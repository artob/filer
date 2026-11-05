// This is free and unencumbered software released into the public domain.

use super::RelPathError;

/// Why a string does not normalize to a [`RelPath`](super::RelPath); see
/// [`RelPathBuf::normalize_lexically`](super::RelPathBuf::normalize_lexically).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum NormalizeError {
    /// The string starts with a separator, naming a root: `/etc`, or on
    /// Windows `\Users`, `\\server\share`, or `\\?\C:\`.
    Root,

    /// The string starts with a drive prefix, a letter and a colon, as in
    /// `C:\Users` or the drive-relative `C:Users`.
    DrivePrefix,

    /// A component is not a valid [`Name`](super::Name): `..`, which would
    /// leave the directory, or one containing NUL. Its index counts every
    /// component of the string, including the `.` and empty ones dropped.
    Component(RelPathError),
}

impl core::fmt::Display for NormalizeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Root => f.write_str("path starts at a root"),
            Self::DrivePrefix => f.write_str("path starts with a drive prefix"),
            Self::Component(error) => core::fmt::Display::fmt(error, f),
        }
    }
}

impl core::error::Error for NormalizeError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Component(error) => Some(error),
            Self::Root | Self::DrivePrefix => None,
        }
    }
}

impl From<RelPathError> for NormalizeError {
    fn from(error: RelPathError) -> Self {
        Self::Component(error)
    }
}

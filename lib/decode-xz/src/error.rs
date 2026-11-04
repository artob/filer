// This is free and unencumbered software released into the public domain.

/// Why a stream did not decode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Error {
    /// The stream is malformed: not xz, damaged, or cut short, or its
    /// headers, data, checks, and index disagree.
    Corrupt,
    /// The stream is well formed, but needs what this decoder does not
    /// do: a filter it does not know, or a dictionary over 128 MiB.
    Unsupported,
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::Corrupt => "corrupt xz stream",
            Self::Unsupported => "unsupported xz stream",
        })
    }
}

impl core::error::Error for Error {}

pub(crate) const fn corrupt() -> Error {
    Error::Corrupt
}

pub(crate) const fn unsupported() -> Error {
    Error::Unsupported
}

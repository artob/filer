// This is free and unencumbered software released into the public domain.

/// Why a stream did not decode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Error {
    /// The stream is malformed: not zstd, damaged, or cut short, or it
    /// breaks a rule that zstd itself enforces.
    Corrupt,
    /// The stream is well formed, but needs what this decoder does not
    /// do: a dictionary, or a window larger than 128 MiB.
    Unsupported,
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::Corrupt => "corrupt zstd stream",
            Self::Unsupported => "unsupported zstd stream",
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

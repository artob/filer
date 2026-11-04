// This is free and unencumbered software released into the public domain.

/// Why a stream did not decode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Error {
    /// The stream is malformed: not zlib, damaged, or cut short, its
    /// Deflate data corrupt, or its Adler-32 not that of what it decodes
    /// to.
    Corrupt,
    /// The stream needs a preset dictionary, which this decoder is not
    /// given.
    Unsupported,
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::Corrupt => "corrupt zlib stream",
            Self::Unsupported => "zlib stream needing a preset dictionary",
        })
    }
}

impl core::error::Error for Error {}

pub(crate) const fn corrupt() -> Error {
    Error::Corrupt
}

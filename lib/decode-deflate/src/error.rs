// This is free and unencumbered software released into the public domain.

/// Why a stream did not decode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Error {
    /// The stream is malformed: not Deflate, damaged, or cut short, or it
    /// breaks a rule that zlib enforces.
    Corrupt,
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::Corrupt => "corrupt Deflate stream",
        })
    }
}

impl core::error::Error for Error {}

pub(crate) const fn corrupt() -> Error {
    Error::Corrupt
}

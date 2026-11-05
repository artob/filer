// This is free and unencumbered software released into the public domain.

/// Why a stream did not decode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Error {
    /// The stream or block is malformed: not LZ4, damaged, or cut short,
    /// or its checksums or size not those of what it decodes to.
    Corrupt,
    /// The stream is well formed, but needs what this decoder does not
    /// do: a dictionary.
    Unsupported,
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::Corrupt => "corrupt LZ4 data",
            Self::Unsupported => "unsupported LZ4 stream",
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

// This is free and unencumbered software released into the public domain.

use crate::{Error, ErrorKind, OpKind};

/// Why an archive could not be opened: its format, the offset of the
/// header at fault, and the reason. Archive drivers return it as the
/// [`source`](core::error::Error::source) of their [`Error`].
///
/// # Examples
///
/// ```
/// use filer_api::{ErrorKind, archive::ArchiveError};
///
/// let error = ArchiveError::new("tar", 1024, "bad checksum").into_error(ErrorKind::Corrupt);
/// let source = core::error::Error::source(&error).unwrap();
/// assert_eq!(source.to_string(), "tar header at offset 1024: bad checksum");
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ArchiveError {
    format: &'static str,
    offset: u64,
    reason: &'static str,
}

impl ArchiveError {
    /// Returns an error about the `format` header at `offset`.
    #[must_use]
    pub const fn new(format: &'static str, offset: u64, reason: &'static str) -> Self {
        Self {
            format,
            offset,
            reason,
        }
    }

    /// Returns the archive format, such as `"tar"`.
    #[must_use]
    pub const fn format(&self) -> &'static str {
        self.format
    }

    /// Returns the offset of the header at fault.
    #[must_use]
    pub const fn offset(&self) -> u64 {
        self.offset
    }

    /// Returns what was wrong.
    #[must_use]
    pub const fn reason(&self) -> &'static str {
        self.reason
    }

    /// Returns an [`Error`] of `kind` from opening the archive, with this
    /// as its source.
    #[must_use]
    pub fn into_error(self, kind: ErrorKind) -> Error {
        Error::new(kind, OpKind::Open).with_source(self)
    }
}

impl core::fmt::Display for ArchiveError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{} header at offset {}: {}",
            self.format, self.offset, self.reason
        )
    }
}

impl core::error::Error for ArchiveError {}

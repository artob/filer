// This is free and unencumbered software released into the public domain.

use crate::ArchiveError;
use alloc::vec::Vec;
use filer_api::{BufResult, Error, ErrorKind, ReadAt};

/// What every format's header parser shares: its reads of the source, and
/// its errors, which name the format and the offset of the header at fault
/// (see [`ArchiveError`]).
///
/// # Examples
///
/// ```
/// use filer_api::ErrorKind;
/// use filer_pack::Parser;
///
/// const TAR: Parser = Parser::new("tar");
///
/// let error = TAR.fail(ErrorKind::Corrupt, 1024, "bad checksum");
/// let source = core::error::Error::source(&error).unwrap();
/// assert_eq!(source.to_string(), "tar header at offset 1024: bad checksum");
///
/// let archive: &'static [u8] = b"0123";
/// assert_eq!(filer_spec::block_on(TAR.read(&archive, 1, 2))?, b"12");
/// let truncated = filer_spec::block_on(TAR.read(&archive, 3, 2)).unwrap_err();
/// assert_eq!(truncated.kind(), ErrorKind::Corrupt);
/// # Ok::<(), filer_api::Error>(())
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Parser {
    format: &'static str,
}

impl Parser {
    /// Returns the parser of `format`, such as `"tar"`, which errors name.
    #[must_use]
    pub const fn new(format: &'static str) -> Self {
        Self { format }
    }

    /// Returns the name of the format, such as `"tar"`.
    #[must_use]
    pub const fn format(self) -> &'static str {
        self.format
    }

    /// Returns an error of `kind` from opening the archive, for `reason`,
    /// about the header at `offset`.
    #[must_use]
    pub fn fail(self, kind: ErrorKind, offset: u64, reason: &'static str) -> Error {
        ArchiveError::new(self.format, offset, reason).into_error(kind)
    }

    /// Reads exactly `len` bytes at `offset` of `source`.
    ///
    /// # Errors
    ///
    /// [`Corrupt`](ErrorKind::Corrupt), naming `offset`, if the source ends
    /// first, as a truncated archive does; otherwise the source's own.
    pub async fn read<S: ReadAt>(
        self,
        source: &S,
        offset: u64,
        len: usize,
    ) -> Result<Vec<u8>, Error> {
        let BufResult(result, buf) = source.read_exact_at(Vec::with_capacity(len), offset).await;
        match result {
            Ok(()) => Ok(buf),
            Err(error) if error.kind() == ErrorKind::UnexpectedEof => {
                Err(self.fail(ErrorKind::Corrupt, offset, "truncated"))
            },
            Err(error) => Err(error),
        }
    }
}

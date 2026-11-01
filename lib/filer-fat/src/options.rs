// This is free and unencumbered software released into the public domain.

use crate::{CodePage, UtcOffset};

/// How [`Fat::open_with`](crate::Fat::open_with) reads a volume.
///
/// # Examples
///
/// ```
/// use filer_fat::{Options, UtcOffset};
///
/// let options = Options::new().with_utc_offset(UtcOffset::from_minutes(-300).unwrap());
/// assert_eq!(options.utc_offset().minutes(), -300);
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct Options {
    utc_offset: UtcOffset,
    code_page: CodePage,
}

impl Options {
    /// Returns the default options: times in UTC, and short names in code
    /// page 437.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            utc_offset: UtcOffset::UTC,
            code_page: CodePage::Cp437,
        }
    }

    /// Reads the volume's times, which are local, at `offset` from UTC.
    #[must_use]
    pub const fn with_utc_offset(mut self, offset: UtcOffset) -> Self {
        self.utc_offset = offset;
        self
    }

    /// Returns the offset from UTC that times are read at.
    #[must_use]
    pub const fn utc_offset(&self) -> UtcOffset {
        self.utc_offset
    }

    /// Decodes short names from `page`.
    #[must_use]
    pub const fn with_code_page(mut self, page: CodePage) -> Self {
        self.code_page = page;
        self
    }

    /// Returns the code page that short names are decoded from.
    #[must_use]
    pub const fn code_page(&self) -> CodePage {
        self.code_page
    }
}

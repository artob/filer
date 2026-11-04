// This is free and unencumbered software released into the public domain.

use crate::{CodePage, Limits};

/// How [`Zip::open_with`](crate::Zip::open_with) reads an archive: within
/// its [`Limits`], and with names that DOS and Windows hosts wrote without
/// the UTF-8 flag read in its [`CodePage`].
///
/// Code page 437, the default, is always available; the `code-pages`
/// feature makes every other page of `known-codepages` available.
///
/// # Examples
///
/// ```
/// use filer_zip::{CodePage, Limits, Options};
///
/// let options = Options::new().with_limits(Limits::default().with_max_entries(10));
/// assert_eq!(options.limits().max_entries, 10);
/// assert_eq!(options.code_page(), CodePage::Cp437);
/// // Limits alone are options too.
/// assert_eq!(Options::from(Limits::default()), Options::new());
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Options {
    limits: Limits,
    code_page: CodePage,
}

impl Options {
    /// Returns the default options: the default [`Limits`], and names in
    /// code page 437.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Reads the archive within `limits`.
    #[must_use]
    pub const fn with_limits(mut self, limits: Limits) -> Self {
        self.limits = limits;
        self
    }

    /// Returns the limits the archive is read within.
    #[must_use]
    pub const fn limits(&self) -> Limits {
        self.limits
    }

    /// Reads names that DOS and Windows hosts wrote without the UTF-8 flag
    /// in `page`.
    #[must_use]
    pub const fn with_code_page(mut self, page: CodePage) -> Self {
        self.code_page = page;
        self
    }

    /// Returns the code page names without the UTF-8 flag are read in.
    #[must_use]
    pub const fn code_page(&self) -> CodePage {
        self.code_page
    }
}

impl From<Limits> for Options {
    fn from(limits: Limits) -> Self {
        Self::new().with_limits(limits)
    }
}

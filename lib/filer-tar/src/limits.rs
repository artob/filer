// This is free and unencumbered software released into the public domain.

/// Safety limits applied while indexing an archive.
///
/// Archives are untrusted input; the defaults are strict and suit typical
/// source and release tarballs. Exceeding a limit fails the open with
/// [`LimitExceeded`](filer_api::ErrorKind::LimitExceeded).
///
/// # Examples
///
/// ```
/// use filer_tar::Limits;
///
/// let limits = Limits::default().with_max_entries(1_000_000);
/// assert_eq!(limits.max_entries, 1_000_000);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Limits {
    /// The most members the index may hold, which bounds its memory.
    /// Default: 100,000.
    pub max_entries: usize,

    /// The longest member name or link target, in bytes. Default: 4,096.
    pub max_path_len: usize,

    /// The largest PAX extended header or GNU long-name record, in bytes.
    /// Default: 64 KiB.
    pub max_extension_len: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_entries: 100_000,
            max_path_len: 4096,
            max_extension_len: 64 * 1024,
        }
    }
}

impl Limits {
    /// Sets the most members the index may hold.
    #[must_use]
    pub const fn with_max_entries(mut self, max_entries: usize) -> Self {
        self.max_entries = max_entries;
        self
    }

    /// Sets the longest member name or link target, in bytes.
    #[must_use]
    pub const fn with_max_path_len(mut self, max_path_len: usize) -> Self {
        self.max_path_len = max_path_len;
        self
    }

    /// Sets the largest extended header record, in bytes.
    #[must_use]
    pub const fn with_max_extension_len(mut self, max_extension_len: u64) -> Self {
        self.max_extension_len = max_extension_len;
        self
    }
}

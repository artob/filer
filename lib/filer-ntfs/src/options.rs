// This is free and unencumbered software released into the public domain.

/// How [`Ntfs::open_with`](crate::Ntfs::open_with) reads a volume.
///
/// # Examples
///
/// ```
/// use filer_ntfs::Options;
///
/// let options = Options::new().allow_dirty(true);
/// assert!(options.allows_dirty());
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct Options {
    allow_dirty: bool,
}

impl Options {
    /// Returns the default options: dirty volumes are refused.
    #[must_use]
    pub const fn new() -> Self {
        Self { allow_dirty: false }
    }

    /// Opens volumes marked dirty, whose `$LogFile` was not replayed after
    /// an unclean unmount or a hibernation, if `allow`.
    ///
    /// Such a volume is read as it is on disk, without the changes its log
    /// holds: what was being written when it was last used may be missing
    /// or half done, though every structure is still checked as always.
    #[must_use]
    pub const fn allow_dirty(mut self, allow: bool) -> Self {
        self.allow_dirty = allow;
        self
    }

    /// Returns `true` if dirty volumes are opened.
    #[must_use]
    pub const fn allows_dirty(&self) -> bool {
        self.allow_dirty
    }
}

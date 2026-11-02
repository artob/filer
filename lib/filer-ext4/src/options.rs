// This is free and unencumbered software released into the public domain.

/// How [`Ext4::open_with`](crate::Ext4::open_with) reads a volume.
///
/// # Examples
///
/// ```
/// use filer_ext4::Options;
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
    /// Returns the default options: volumes whose journal needs recovery
    /// are refused.
    #[must_use]
    pub const fn new() -> Self {
        Self { allow_dirty: false }
    }

    /// Opens volumes whose journal needs recovery, if `allow`, as they are
    /// on disk, without replaying the journal.
    ///
    /// What was being written when such a volume was last used may then be
    /// missing or half done, though every structure is still checked as
    /// always.
    #[must_use]
    pub const fn allow_dirty(mut self, allow: bool) -> Self {
        self.allow_dirty = allow;
        self
    }

    /// Returns `true` if volumes whose journal needs recovery are opened.
    #[must_use]
    pub const fn allows_dirty(&self) -> bool {
        self.allow_dirty
    }
}

// This is free and unencumbered software released into the public domain.

/// How [`Xfs::open_with`](crate::Xfs::open_with) reads a volume.
///
/// # Examples
///
/// ```
/// use filer_xfs::Options;
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
    /// Returns the default options: volumes whose log needs recovery, or
    /// cannot be checked, are refused.
    #[must_use]
    pub const fn new() -> Self {
        Self { allow_dirty: false }
    }

    /// Opens volumes whose log was not left clean, if `allow`: those not
    /// unmounted cleanly, and those whose log is on another device, which
    /// cannot be checked.
    ///
    /// Such a volume is read as it is on disk, without the changes its log
    /// holds: what was being written when it was last used may be missing
    /// or half done, though every structure is still checked as always.
    #[must_use]
    pub const fn allow_dirty(mut self, allow: bool) -> Self {
        self.allow_dirty = allow;
        self
    }

    /// Returns `true` if volumes whose log was not left clean are opened.
    #[must_use]
    pub const fn allows_dirty(&self) -> bool {
        self.allow_dirty
    }
}

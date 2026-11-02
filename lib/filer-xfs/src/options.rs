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
    /// Returns the default options: volumes whose log needs recovery are
    /// read as recovery leaves them, replayed in memory; those whose log
    /// holds work recovery would do that this driver does not, or is on
    /// another device, which cannot be checked, are refused.
    #[must_use]
    pub const fn new() -> Self {
        Self { allow_dirty: false }
    }

    /// Reads volumes as they are on disk, if `allow`, whatever their logs:
    /// those not unmounted cleanly, without replaying their logs, and
    /// those whose log is on another device, which cannot be checked.
    ///
    /// What was being written when such a volume was last used may then be
    /// missing or half done, though every structure is still checked as
    /// always.
    #[must_use]
    pub const fn allow_dirty(mut self, allow: bool) -> Self {
        self.allow_dirty = allow;
        self
    }

    /// Returns `true` if volumes are read as they are on disk, whatever
    /// their logs.
    #[must_use]
    pub const fn allows_dirty(&self) -> bool {
        self.allow_dirty
    }
}

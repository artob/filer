// This is free and unencumbered software released into the public domain.

/// A fallback a driver took while performing an operation, such as
/// `openat2` falling back to a userspace walk.
///
/// Fallbacks are reported, never silent: an [`Error`](crate::Error) carries
/// the one taken before it failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Fallback {
    from: &'static str,
    to: &'static str,
}

impl Fallback {
    /// Records a fallback from the mechanism `from` to the mechanism `to`.
    #[must_use]
    pub const fn new(from: &'static str, to: &'static str) -> Self {
        Self { from, to }
    }

    /// Returns the mechanism that was tried first.
    #[must_use]
    pub const fn from(&self) -> &'static str {
        self.from
    }

    /// Returns the mechanism that was used instead.
    #[must_use]
    pub const fn to(&self) -> &'static str {
        self.to
    }
}

impl core::fmt::Display for Fallback {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{} -> {}", self.from, self.to)
    }
}

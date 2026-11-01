// This is free and unencumbered software released into the public domain.

use super::NameError;

/// Why a string is not a valid [`RelPath`](super::RelPath).
///
/// Every rejection is a component that is not a valid
/// [`Name`](super::Name): a leading, trailing, or doubled `/` makes an empty
/// component, and `.` and `..` are rejected as components.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RelPathError {
    index: usize,
    reason: NameError,
}

impl RelPathError {
    pub(super) const fn new(index: usize, reason: NameError) -> Self {
        Self { index, reason }
    }

    /// Returns the zero-based index of the first invalid component.
    #[must_use]
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Returns why that component is invalid.
    #[must_use]
    pub const fn reason(&self) -> NameError {
        self.reason
    }
}

impl core::fmt::Display for RelPathError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "invalid path component {}: {}", self.index, self.reason)
    }
}

impl core::error::Error for RelPathError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        Some(&self.reason)
    }
}

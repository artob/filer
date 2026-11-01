// This is free and unencumbered software released into the public domain.

use crate::{DriverId, escaped::Escaped};
use alloc::boxed::Box;

/// A name found on disk that is not valid UTF-8 (on Windows, not
/// well-formed UTF-16).
///
/// Enumeration is total: such names are surfaced instead of skipped, so
/// listings have no holes and recursive deletes can see every entry. A
/// `ForeignName` can be used to open, stat, remove, or rename *from* the
/// entry it names, but only with handles from the driver instance that
/// produced it ([`origin`](Self::origin)); it can never name a new entry or
/// be a rename destination.
///
/// It has no public constructor: drivers create foreign names through
/// their private [`DriverKey`](crate::DriverKey), so they cannot be forged.
/// Its bytes are never valid UTF-8, contain neither `/` nor NUL, and are
/// raw bytes on POSIX and WTF-8 on Windows.
///
/// [`Display`](core::fmt::Display) renders invalid bytes as `\xNN`; the
/// rendering is lossy and cannot be parsed back. Foreign names are not
/// serializable.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ForeignName {
    raw: Box<[u8]>,
    origin: DriverId,
}

impl ForeignName {
    /// Returns a foreign name; the caller has checked `raw`'s invariants.
    pub(crate) fn new(raw: &[u8], origin: DriverId) -> Self {
        Self {
            raw: Box::from(raw),
            origin,
        }
    }

    /// Returns the name's bytes as the driver read them.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.raw
    }

    /// Returns the id of the driver instance that produced this name.
    #[must_use]
    pub fn origin(&self) -> DriverId {
        self.origin
    }
}

impl core::fmt::Debug for ForeignName {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ForeignName")
            .field("raw", &Escaped(&self.raw))
            .field("origin", &self.origin)
            .finish()
    }
}

impl core::fmt::Display for ForeignName {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Display::fmt(&Escaped(&self.raw), f)
    }
}

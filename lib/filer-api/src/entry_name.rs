// This is free and unencumbered software released into the public domain.

use crate::{
    ForeignName,
    path::{Name, NameBuf},
};

/// The name of a directory entry, as yielded by enumeration: a valid
/// [`Name`] or a [`ForeignName`].
///
/// Operations on existing entries (open, stat, remove, rename from) accept
/// an `EntryName`; operations that create entries accept only a [`Name`].
///
/// A UTF-8 name compares equal to a string of its text, and a foreign name
/// to none:
///
/// ```
/// use filer_api::{DriverKey, EntryName};
///
/// let key = DriverKey::new();
/// assert_eq!(key.entry_name(b"notes.txt").unwrap(), "notes.txt");
/// assert_ne!(key.entry_name(b"caf\xE9").unwrap(), "caf\u{FFFD}");
/// ```
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EntryName {
    /// A valid UTF-8 name.
    Utf8(NameBuf),

    /// A name that is not valid UTF-8, usable only with its origin driver.
    Foreign(ForeignName),
}

impl EntryName {
    /// Returns the UTF-8 name, or `None` for a foreign name.
    #[must_use]
    pub fn as_name(&self) -> Option<&Name> {
        match self {
            Self::Utf8(name) => Some(name),
            Self::Foreign(_) => None,
        }
    }

    /// Returns `true` if the bytes `raw` can name an entry, whether as a
    /// [`Name`] or a [`ForeignName`]: they are not empty, `.`, or `..`, and
    /// hold no `/` or NUL. These are the names
    /// [`DriverKey::entry_name`](crate::DriverKey::entry_name) accepts, so a
    /// driver can check a name read from disk where it finds it.
    ///
    /// # Examples
    ///
    /// ```
    /// use filer_api::EntryName;
    ///
    /// assert!(EntryName::is_valid(b"notes.txt"));
    /// assert!(EntryName::is_valid(b"caf\xE9"));
    /// assert!(!EntryName::is_valid(b".."));
    /// assert!(!EntryName::is_valid(b"a/b"));
    /// ```
    #[must_use]
    pub const fn is_valid(raw: &[u8]) -> bool {
        crate::path::name::validate(raw).is_ok()
    }

    /// Returns `true` for a foreign name.
    #[must_use]
    pub fn is_foreign(&self) -> bool {
        matches!(self, Self::Foreign(_))
    }
}

impl PartialEq<str> for EntryName {
    fn eq(&self, other: &str) -> bool {
        self.as_name().is_some_and(|name| name.as_str() == other)
    }
}

impl PartialEq<&str> for EntryName {
    fn eq(&self, other: &&str) -> bool {
        *self == **other
    }
}

impl From<NameBuf> for EntryName {
    fn from(name: NameBuf) -> Self {
        Self::Utf8(name)
    }
}

impl From<&Name> for EntryName {
    fn from(name: &Name) -> Self {
        Self::Utf8(NameBuf::from(name))
    }
}

impl From<ForeignName> for EntryName {
    fn from(name: ForeignName) -> Self {
        Self::Foreign(name)
    }
}

impl core::fmt::Display for EntryName {
    /// Renders UTF-8 names as is and foreign names lossily (`\xNN`).
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Utf8(name) => core::fmt::Display::fmt(name, f),
            Self::Foreign(name) => core::fmt::Display::fmt(name, f),
        }
    }
}

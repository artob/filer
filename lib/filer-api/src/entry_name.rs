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

    /// Returns `true` for a foreign name.
    #[must_use]
    pub fn is_foreign(&self) -> bool {
        matches!(self, Self::Foreign(_))
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

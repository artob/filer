// This is free and unencumbered software released into the public domain.

use crate::{ForeignName, XattrName, XattrNameError, XattrNs};

/// The name of an extended attribute, as yielded by listing: a valid
/// [`XattrName`], or a namespace and a [`ForeignName`].
///
/// As with directory entries ([`EntryName`](crate::EntryName)), listing is
/// total: a name that is not valid UTF-8 (on NTFS, a stream name with an
/// unpaired surrogate) is surfaced as foreign rather than skipped, and can
/// be read back, with the driver instance that listed it.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum XattrEntry {
    /// A valid UTF-8 name.
    Utf8(XattrName),

    /// A name that is not valid UTF-8, usable only with its origin driver.
    Foreign {
        /// The namespace.
        ns: XattrNs,
        /// The name within the namespace. Unlike a foreign entry name, it
        /// may contain `/`; it never contains NUL.
        name: ForeignName,
    },
}

impl XattrEntry {
    /// Returns the namespace.
    #[must_use]
    pub fn ns(&self) -> XattrNs {
        match self {
            Self::Utf8(name) => name.ns(),
            Self::Foreign { ns, .. } => *ns,
        }
    }

    /// Returns the UTF-8 name, or `None` for a foreign name.
    #[must_use]
    pub fn as_name(&self) -> Option<&XattrName> {
        match self {
            Self::Utf8(name) => Some(name),
            Self::Foreign { .. } => None,
        }
    }

    /// Returns `true` for a foreign name.
    #[must_use]
    pub fn is_foreign(&self) -> bool {
        matches!(self, Self::Foreign { .. })
    }
}

impl From<XattrName> for XattrEntry {
    fn from(name: XattrName) -> Self {
        Self::Utf8(name)
    }
}

impl From<&XattrName> for XattrEntry {
    fn from(name: &XattrName) -> Self {
        Self::Utf8(name.clone())
    }
}

impl TryFrom<&str> for XattrEntry {
    type Error = XattrNameError;

    /// Parses `ns.name`, such as `user.comment`.
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        s.parse().map(Self::Utf8)
    }
}

impl core::fmt::Display for XattrEntry {
    /// Renders UTF-8 names as is and foreign names lossily (`\xNN`).
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Utf8(name) => core::fmt::Display::fmt(name, f),
            Self::Foreign { ns, name } => write!(f, "{ns}.{name}"),
        }
    }
}

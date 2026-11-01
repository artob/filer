// This is free and unencumbered software released into the public domain.

use super::{Name, NameError, RelPath};
use alloc::{boxed::Box, string::String};

/// One valid path component; owned form of [`Name`].
///
/// `NameBuf` dereferences to [`Name`] and hashes, compares, and orders the
/// same way, so it can be looked up by `&Name` in maps and sets.
///
/// # Examples
///
/// ```
/// use filer_api::path::{Name, NameBuf};
///
/// let owned: NameBuf = "app.toml".parse().unwrap();
/// let borrowed: &Name = &owned;
/// assert_eq!(borrowed.as_str(), "app.toml");
/// assert!("a/b".parse::<NameBuf>().is_err());
/// ```
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NameBuf(Box<str>);

impl NameBuf {
    /// Borrows this name as a [`Name`].
    #[must_use]
    pub fn as_name(&self) -> &Name {
        Name::from_str_unchecked(&self.0)
    }

    /// Converts this name into a [`String`].
    #[must_use]
    pub fn into_string(self) -> String {
        self.0.into_string()
    }
}

impl core::ops::Deref for NameBuf {
    type Target = Name;

    fn deref(&self) -> &Name {
        self.as_name()
    }
}

impl core::borrow::Borrow<Name> for NameBuf {
    fn borrow(&self) -> &Name {
        self.as_name()
    }
}

impl AsRef<Name> for NameBuf {
    fn as_ref(&self) -> &Name {
        self.as_name()
    }
}

impl AsRef<RelPath> for NameBuf {
    fn as_ref(&self) -> &RelPath {
        self.as_rel_path()
    }
}

impl AsRef<str> for NameBuf {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<&Name> for NameBuf {
    fn from(name: &Name) -> Self {
        Self(Box::from(name.as_str()))
    }
}

impl From<NameBuf> for String {
    fn from(name: NameBuf) -> Self {
        name.into_string()
    }
}

impl TryFrom<&str> for NameBuf {
    type Error = NameError;

    fn try_from(s: &str) -> Result<Self, NameError> {
        Name::new(s).map(Self::from)
    }
}

impl TryFrom<String> for NameBuf {
    type Error = NameError;

    fn try_from(s: String) -> Result<Self, NameError> {
        Name::new(&s)?;
        Ok(Self(s.into_boxed_str()))
    }
}

impl core::str::FromStr for NameBuf {
    type Err = NameError;

    fn from_str(s: &str) -> Result<Self, NameError> {
        Self::try_from(s)
    }
}

impl PartialEq<Name> for NameBuf {
    fn eq(&self, other: &Name) -> bool {
        **self == *other
    }
}

impl PartialEq<str> for NameBuf {
    fn eq(&self, other: &str) -> bool {
        *self.0 == *other
    }
}

impl core::fmt::Debug for NameBuf {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(self.as_name(), f)
    }
}

impl core::fmt::Display for NameBuf {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Display::fmt(self.as_name(), f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{borrow::ToOwned, collections::BTreeSet, string::ToString};

    #[test]
    fn round_trips() {
        let name = Name::new("app.toml").unwrap();
        let owned = name.to_owned();
        assert_eq!(&*owned, name);
        assert_eq!(owned.to_string(), "app.toml");
        assert_eq!(owned.clone().into_string(), "app.toml");
        assert_eq!("app.toml".parse::<NameBuf>(), Ok(owned.clone()));
        assert_eq!(NameBuf::try_from("app.toml".to_string()), Ok(owned));
    }

    #[test]
    fn rejects_like_name() {
        assert_eq!(NameBuf::try_from(""), Err(NameError::Empty));
        assert_eq!(
            NameBuf::try_from("a/b".to_string()),
            Err(NameError::ContainsSlash)
        );
        assert_eq!("a\0".parse::<NameBuf>(), Err(NameError::ContainsNul));
        assert_eq!("..".parse::<NameBuf>(), Err(NameError::DotOrDotDot));
    }

    #[test]
    fn looks_up_by_borrowed_name() {
        let set: BTreeSet<NameBuf> = ["b", "a"].iter().map(|s| s.parse().unwrap()).collect();
        assert!(set.contains(Name::new("a").unwrap()));
        let ordered: alloc::vec::Vec<&str> = set.iter().map(|n| n.as_str()).collect();
        assert_eq!(ordered, ["a", "b"]);
    }
}

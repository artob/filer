// This is free and unencumbered software released into the public domain.

use super::{Name, NameBuf, RelPath, RelPathError};
use alloc::string::String;

/// Zero or more [`Name`]s joined by `/`; owned form of [`RelPath`].
///
/// `RelPathBuf` dereferences to [`RelPath`] and hashes, compares, and
/// orders the same way, so it can be looked up by `&RelPath` in maps and
/// sets. The default value is the empty path.
///
/// # Examples
///
/// ```
/// use filer_api::path::{Name, RelPathBuf};
///
/// let mut path = RelPathBuf::new();
/// path.push(Name::new("config").unwrap());
/// path.push(Name::new("app.toml").unwrap());
/// assert_eq!(path.as_str(), "config/app.toml");
/// assert!(path.pop());
/// assert_eq!(path.as_str(), "config");
/// ```
#[derive(Clone, Default)]
pub struct RelPathBuf(String);

impl RelPathBuf {
    /// Returns a new, empty path.
    #[must_use]
    pub const fn new() -> Self {
        Self(String::new())
    }

    /// Borrows this path as a [`RelPath`].
    #[must_use]
    pub fn as_rel_path(&self) -> &RelPath {
        RelPath::from_str_unchecked(&self.0)
    }

    /// Converts this path into a [`String`].
    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }

    /// Appends `path`; appending the empty path changes nothing.
    pub fn push<P: AsRef<RelPath> + ?Sized>(&mut self, path: &P) {
        let path = path.as_ref().as_str();
        if path.is_empty() {
            return;
        }
        if !self.0.is_empty() {
            self.0.push('/');
        }
        self.0.push_str(path);
    }

    /// Removes the last component. Returns `false`, changing nothing, if the
    /// path was already empty.
    pub fn pop(&mut self) -> bool {
        let Some((parent, _)) = self.as_rel_path().split_last() else {
            return false;
        };
        let len = parent.as_str().len();
        self.0.truncate(len);
        true
    }
}

impl core::ops::Deref for RelPathBuf {
    type Target = RelPath;

    fn deref(&self) -> &RelPath {
        self.as_rel_path()
    }
}

impl core::borrow::Borrow<RelPath> for RelPathBuf {
    fn borrow(&self) -> &RelPath {
        self.as_rel_path()
    }
}

impl AsRef<RelPath> for RelPathBuf {
    fn as_ref(&self) -> &RelPath {
        self.as_rel_path()
    }
}

impl AsRef<str> for RelPathBuf {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<&RelPath> for RelPathBuf {
    fn from(path: &RelPath) -> Self {
        Self(String::from(path.as_str()))
    }
}

impl From<&Name> for RelPathBuf {
    fn from(name: &Name) -> Self {
        Self::from(name.as_rel_path())
    }
}

impl From<NameBuf> for RelPathBuf {
    fn from(name: NameBuf) -> Self {
        Self(name.into_string())
    }
}

impl From<RelPathBuf> for String {
    fn from(path: RelPathBuf) -> Self {
        path.into_string()
    }
}

impl TryFrom<&str> for RelPathBuf {
    type Error = RelPathError;

    fn try_from(s: &str) -> Result<Self, RelPathError> {
        RelPath::new(s).map(Self::from)
    }
}

impl TryFrom<String> for RelPathBuf {
    type Error = RelPathError;

    fn try_from(s: String) -> Result<Self, RelPathError> {
        RelPath::new(&s)?;
        Ok(Self(s))
    }
}

impl core::str::FromStr for RelPathBuf {
    type Err = RelPathError;

    fn from_str(s: &str) -> Result<Self, RelPathError> {
        Self::try_from(s)
    }
}

impl PartialEq for RelPathBuf {
    fn eq(&self, other: &Self) -> bool {
        **self == **other
    }
}

impl Eq for RelPathBuf {}

impl PartialEq<RelPath> for RelPathBuf {
    fn eq(&self, other: &RelPath) -> bool {
        **self == *other
    }
}

impl PartialEq<&RelPath> for RelPathBuf {
    fn eq(&self, other: &&RelPath) -> bool {
        **self == **other
    }
}

impl PartialEq<str> for RelPathBuf {
    fn eq(&self, other: &str) -> bool {
        *self.0 == *other
    }
}

impl Ord for RelPathBuf {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        (**self).cmp(&**other)
    }
}

impl PartialOrd for RelPathBuf {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl core::hash::Hash for RelPathBuf {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.as_rel_path().hash(state);
    }
}

impl core::fmt::Debug for RelPathBuf {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(self.as_rel_path(), f)
    }
}

impl core::fmt::Display for RelPathBuf {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Display::fmt(self.as_rel_path(), f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path::NameError;
    use alloc::{borrow::ToOwned, collections::BTreeSet, string::ToString};

    #[test]
    fn round_trips() {
        let path = RelPath::new("a/b").unwrap();
        let owned = path.to_owned();
        assert_eq!(&*owned, path);
        assert_eq!(owned.to_string(), "a/b");
        assert_eq!("a/b".parse::<RelPathBuf>(), Ok(owned.clone()));
        assert_eq!(RelPathBuf::try_from("a/b".to_string()), Ok(owned.clone()));
        assert_eq!(owned.into_string(), "a/b");
    }

    #[test]
    fn rejects_like_rel_path() {
        let error = RelPathBuf::try_from("a//b".to_string()).unwrap_err();
        assert_eq!((error.index(), error.reason()), (1, NameError::Empty));
        assert!("..".parse::<RelPathBuf>().is_err());
    }

    #[test]
    fn pushes_and_pops() {
        let mut path = RelPathBuf::new();
        path.push(RelPath::empty());
        assert_eq!(path, RelPath::empty());
        path.push(RelPath::new("a/b").unwrap());
        path.push(Name::new("c").unwrap());
        assert_eq!(path, RelPath::new("a/b/c").unwrap());
        assert!(path.pop());
        assert!(path.pop());
        assert_eq!(path, RelPath::new("a").unwrap());
        assert!(path.pop());
        assert!(path.is_empty());
        assert!(!path.pop());
    }

    #[test]
    fn looks_up_by_borrowed_path() {
        let set: BTreeSet<RelPathBuf> = ["a-b", "a/b"].iter().map(|s| s.parse().unwrap()).collect();
        assert!(set.contains(RelPath::new("a/b").unwrap()));
        let ordered: alloc::vec::Vec<&str> = set.iter().map(|p| p.as_str()).collect();
        assert_eq!(ordered, ["a/b", "a-b"]);
    }
}

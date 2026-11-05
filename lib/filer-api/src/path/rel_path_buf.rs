// This is free and unencumbered software released into the public domain.

use super::{Name, NameBuf, NormalizeError, RelPath, RelPathError, Separators, name};
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

    /// Reads `s` as a relative path, leniently: drops `.` components and
    /// the empty ones that doubled, leading `./`, or trailing separators
    /// make, and joins what remains with `/`. `separators` says whether
    /// `\` separates components too, as on Windows.
    ///
    /// This is for paths typed by users or taken from elsewhere; a string
    /// that is already a valid [`RelPath`] reads as itself. Nothing is
    /// resolved: `..` is refused, not applied, since a path must not
    /// leave its directory, and so are roots and drive prefixes, even
    /// with `/` alone separating, since `C:` names a drive on Windows. A
    /// file named `C:` stays reachable through [`RelPath::new`].
    ///
    /// # Errors
    ///
    /// Returns [`NormalizeError::Root`] if `s` starts with a separator,
    /// [`NormalizeError::DrivePrefix`] if it starts with a letter and a
    /// colon, and [`NormalizeError::Component`] for the first component
    /// that is `..` or contains NUL.
    ///
    /// # Examples
    ///
    /// ```
    /// use filer_api::path::{NormalizeError, RelPathBuf, Separators};
    ///
    /// let path = RelPathBuf::normalize_lexically("./config//app.toml/", Separators::Slash);
    /// assert_eq!(path.unwrap().as_str(), "config/app.toml");
    ///
    /// let path = RelPathBuf::normalize_lexically(r"config\.\app.toml", Separators::SlashOrBackslash);
    /// assert_eq!(path.unwrap().as_str(), "config/app.toml");
    ///
    /// let error = RelPathBuf::normalize_lexically("/etc", Separators::Slash);
    /// assert_eq!(error, Err(NormalizeError::Root));
    /// ```
    pub fn normalize_lexically(s: &str, separators: Separators) -> Result<Self, NormalizeError> {
        if s.starts_with(|c| separators.contains(c)) {
            return Err(NormalizeError::Root);
        }
        if matches!(s.as_bytes(), [letter, b':', ..] if letter.is_ascii_alphabetic()) {
            return Err(NormalizeError::DrivePrefix);
        }
        let mut path = String::with_capacity(s.len());
        for (index, component) in s.split(|c| separators.contains(c)).enumerate() {
            if component.is_empty() || component == "." {
                continue;
            }
            name::validate(component.as_bytes())
                .map_err(|reason| RelPathError::new(index, reason))?;
            if !path.is_empty() {
                path.push('/');
            }
            path.push_str(component);
        }
        Ok(Self(path))
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

    fn normalize(s: &str, separators: Separators) -> Result<String, NormalizeError> {
        RelPathBuf::normalize_lexically(s, separators).map(RelPathBuf::into_string)
    }

    fn component(index: usize, reason: NameError) -> Result<String, NormalizeError> {
        Err(NormalizeError::Component(RelPathError::new(index, reason)))
    }

    #[test]
    fn normalizes_posix_style() {
        let posix = |s| normalize(s, Separators::Slash);
        for (s, normal) in [
            ("", ""),
            (".", ""),
            ("./", ""),
            ("././/.", ""),
            ("a", "a"),
            ("a/b", "a/b"),
            ("./a", "a"),
            ("a/", "a"),
            ("a//b", "a/b"),
            ("a/./b/.", "a/b"),
            ("a/.b/..c/...", "a/.b/..c/..."),
            // A backslash is part of a name.
            (r"a\b", r"a\b"),
            (r"a\..\b", r"a\..\b"),
            (r"\a", r"\a"),
            // A colon after more than one letter, or later, is too.
            ("ab:c", "ab:c"),
            ("a/C:", "a/C:"),
        ] {
            assert_eq!(posix(s).as_deref(), Ok(normal), "{s:?}");
            assert_eq!(RelPath::new(normal).map(|_| ()), Ok(()), "{s:?}");
        }
        assert_eq!(posix("/"), Err(NormalizeError::Root));
        assert_eq!(posix("/a"), Err(NormalizeError::Root));
        assert_eq!(posix("//a"), Err(NormalizeError::Root));
        assert_eq!(posix("C:/a"), Err(NormalizeError::DrivePrefix));
        assert_eq!(posix("c:"), Err(NormalizeError::DrivePrefix));
        assert_eq!(posix(".."), component(0, NameError::DotOrDotDot));
        assert_eq!(posix("a/../b"), component(1, NameError::DotOrDotDot));
        assert_eq!(posix("./a//.."), component(3, NameError::DotOrDotDot));
        assert_eq!(posix("a/b\0c"), component(1, NameError::ContainsNul));
    }

    #[test]
    fn normalizes_windows_style() {
        let windows = |s| normalize(s, Separators::SlashOrBackslash);
        for (s, normal) in [
            ("", ""),
            (r".\", ""),
            (r"a\b", "a/b"),
            (r"a/b\c", "a/b/c"),
            (r".\a\.\b\", "a/b"),
            (r"a\\\b", "a/b"),
            ("ab:c", "ab:c"),
        ] {
            assert_eq!(windows(s).as_deref(), Ok(normal), "{s:?}");
        }
        for root in [
            r"\",
            r"\a",
            r"\\server\share",
            r"\\?\C:\a",
            r"\\.\COM1",
            "/a",
        ] {
            assert_eq!(windows(root), Err(NormalizeError::Root), "{root:?}");
        }
        for drive in [r"C:\a", "C:/a", "C:a", "z:", r"c:\"] {
            assert_eq!(
                windows(drive),
                Err(NormalizeError::DrivePrefix),
                "{drive:?}"
            );
        }
        assert_eq!(windows(r"a\..\b"), component(1, NameError::DotOrDotDot));
        assert_eq!(windows(r"a\.\..\b"), component(2, NameError::DotOrDotDot));
    }

    #[test]
    fn reports_its_errors() {
        let error = RelPathBuf::normalize_lexically("a/..", Separators::Slash).unwrap_err();
        assert_eq!(
            error.to_string(),
            "invalid path component 1: name is '.' or '..'"
        );
        assert!(core::error::Error::source(&error).is_some());
        assert_eq!(NormalizeError::Root.to_string(), "path starts at a root");
        assert!(core::error::Error::source(&NormalizeError::DrivePrefix).is_none());
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

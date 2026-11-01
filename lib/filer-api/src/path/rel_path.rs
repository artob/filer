// This is free and unencumbered software released into the public domain.

use super::{Components, Name, RelPathBuf, RelPathError, name};
use ref_cast::{RefCastCustom, ref_cast_custom};

/// Zero or more [`Name`]s joined by `/`, such as `config/app.toml`;
/// borrowed form.
///
/// A `RelPath` is always relative to some directory handle and can never
/// leave it lexically:
///
/// - the empty path `""` means the directory itself;
/// - components are separated by exactly one `/`, with no leading or
///   trailing `/`;
/// - every component is a valid [`Name`], so `.` and `..` never appear.
///
/// The separator is `/` on every platform; drivers translate it. Symlinks
/// met during resolution may still point elsewhere; confining them is the
/// driver's job, not this type's.
///
/// Paths compare equal when their strings do, and order component by
/// component (so `a/b` sorts before `a-b`). The owned form is
/// [`RelPathBuf`].
///
/// # Examples
///
/// ```
/// use filer_api::path::RelPath;
///
/// let path = RelPath::new("config/app.toml").unwrap();
/// let names: Vec<&str> = path.components().map(|n| n.as_str()).collect();
/// assert_eq!(names, ["config", "app.toml"]);
/// assert!(RelPath::new("config/../secret").is_err());
/// ```
#[derive(PartialEq, Eq, Hash, RefCastCustom)]
#[repr(transparent)]
pub struct RelPath(str);

impl RelPath {
    /// Checks that `s` is a valid relative path and borrows it as one.
    ///
    /// # Errors
    ///
    /// Returns the first invalid component; see [`RelPathError`].
    pub const fn new(s: &str) -> Result<&Self, RelPathError> {
        match validate(s) {
            Ok(()) => Ok(Self::from_str_unchecked(s)),
            Err(error) => Err(error),
        }
    }

    /// Returns the empty path, which means the directory itself.
    #[must_use]
    pub const fn empty() -> &'static Self {
        Self::from_str_unchecked("")
    }

    /// Borrows `s` as a path without checking it; `s` must be valid.
    #[ref_cast_custom]
    pub(super) const fn from_str_unchecked(s: &str) -> &Self;

    /// Returns the path as a string slice.
    #[must_use]
    pub const fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns `true` for the empty path.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns an iterator over the path's components, first to last.
    #[must_use]
    pub fn components(&self) -> Components<'_> {
        Components::new(&self.0)
    }

    /// Splits off the first component, or returns `None` for the empty path.
    ///
    /// ```
    /// use filer_api::path::RelPath;
    ///
    /// let (first, rest) = RelPath::new("a/b/c").unwrap().split_first().unwrap();
    /// assert_eq!((first.as_str(), rest.as_str()), ("a", "b/c"));
    /// ```
    #[must_use]
    pub fn split_first(&self) -> Option<(&Name, &Self)> {
        if self.is_empty() {
            return None;
        }
        let (first, rest) = self.0.split_once('/').unwrap_or((&self.0, ""));
        Some((
            Name::from_str_unchecked(first),
            Self::from_str_unchecked(rest),
        ))
    }

    /// Splits off the last component, or returns `None` for the empty path.
    ///
    /// ```
    /// use filer_api::path::RelPath;
    ///
    /// let (parent, last) = RelPath::new("a/b/c").unwrap().split_last().unwrap();
    /// assert_eq!((parent.as_str(), last.as_str()), ("a/b", "c"));
    /// ```
    #[must_use]
    pub fn split_last(&self) -> Option<(&Self, &Name)> {
        if self.is_empty() {
            return None;
        }
        let (parent, last) = self.0.rsplit_once('/').unwrap_or(("", &self.0));
        Some((
            Self::from_str_unchecked(parent),
            Name::from_str_unchecked(last),
        ))
    }

    /// Returns the path without its last component, or `None` for the empty
    /// path. The parent of a single name is the empty path.
    #[must_use]
    pub fn parent(&self) -> Option<&Self> {
        self.split_last().map(|(parent, _)| parent)
    }

    /// Returns a new path with `path` appended.
    ///
    /// ```
    /// use filer_api::path::{Name, RelPath};
    ///
    /// let dir = RelPath::new("config").unwrap();
    /// let file = dir.join(Name::new("app.toml").unwrap());
    /// assert_eq!(file.as_str(), "config/app.toml");
    /// ```
    #[must_use]
    pub fn join<P: AsRef<Self> + ?Sized>(&self, path: &P) -> RelPathBuf {
        let mut joined = RelPathBuf::from(self);
        joined.push(path);
        joined
    }

    /// Returns `true` if `base` is a component-wise prefix of this path.
    /// The empty path is a prefix of every path.
    #[must_use]
    pub fn starts_with<P: AsRef<Self> + ?Sized>(&self, base: &P) -> bool {
        self.strip_prefix(base).is_some()
    }

    /// Returns the rest of this path after the component-wise prefix
    /// `base`, or `None` if `base` is not a prefix.
    ///
    /// ```
    /// use filer_api::path::RelPath;
    ///
    /// let path = RelPath::new("a/bc/d").unwrap();
    /// let rest = path.strip_prefix(RelPath::new("a/bc").unwrap()).unwrap();
    /// assert_eq!(rest.as_str(), "d");
    /// assert!(path.strip_prefix(RelPath::new("a/b").unwrap()).is_none());
    /// ```
    #[must_use]
    pub fn strip_prefix<P: AsRef<Self> + ?Sized>(&self, base: &P) -> Option<&Self> {
        let base = base.as_ref().as_str();
        if base.is_empty() {
            return Some(self);
        }
        let rest = self.0.strip_prefix(base)?;
        if rest.is_empty() {
            Some(Self::empty())
        } else {
            rest.strip_prefix('/').map(Self::from_str_unchecked)
        }
    }
}

/// Checks the rules documented on [`RelPath`].
const fn validate(s: &str) -> Result<(), RelPathError> {
    let mut rest = s.as_bytes();
    if rest.is_empty() {
        return Ok(());
    }
    let mut index = 0;
    loop {
        let mut end = 0;
        while end < rest.len() && rest[end] != b'/' {
            end += 1;
        }
        let (component, tail) = rest.split_at(end);
        if let Err(reason) = name::validate(component) {
            return Err(RelPathError::new(index, reason));
        }
        if tail.is_empty() {
            return Ok(());
        }
        rest = tail.split_at(1).1;
        index += 1;
    }
}

impl alloc::borrow::ToOwned for RelPath {
    type Owned = RelPathBuf;

    fn to_owned(&self) -> RelPathBuf {
        RelPathBuf::from(self)
    }
}

impl<'a> TryFrom<&'a str> for &'a RelPath {
    type Error = RelPathError;

    fn try_from(s: &'a str) -> Result<Self, RelPathError> {
        RelPath::new(s)
    }
}

impl<'a> From<&'a Name> for &'a RelPath {
    fn from(name: &'a Name) -> Self {
        name.as_rel_path()
    }
}

impl Default for &RelPath {
    fn default() -> Self {
        RelPath::empty()
    }
}

impl AsRef<RelPath> for RelPath {
    fn as_ref(&self) -> &RelPath {
        self
    }
}

impl AsRef<str> for RelPath {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl PartialEq<str> for RelPath {
    fn eq(&self, other: &str) -> bool {
        self.0 == *other
    }
}

impl PartialEq<RelPathBuf> for RelPath {
    fn eq(&self, other: &RelPathBuf) -> bool {
        *self == **other
    }
}

impl Ord for RelPath {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.components().cmp(other.components())
    }
}

impl PartialOrd for RelPath {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl core::fmt::Debug for RelPath {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(&self.0, f)
    }
}

impl core::fmt::Display for RelPath {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path::NameError;
    use alloc::vec::Vec;

    fn path(s: &str) -> &RelPath {
        RelPath::new(s).unwrap()
    }

    fn names(path: &RelPath) -> Vec<&str> {
        path.components().map(Name::as_str).collect()
    }

    fn rejects(s: &str, index: usize, reason: NameError) {
        assert_eq!(
            RelPath::new(s),
            Err(RelPathError::new(index, reason)),
            "{s:?}"
        );
    }

    #[test]
    fn accepts_the_empty_path() {
        let empty = path("");
        assert!(empty.is_empty());
        assert_eq!(empty, RelPath::empty());
        assert_eq!(names(empty), Vec::<&str>::new());
        assert_eq!(empty.split_first(), None);
        assert_eq!(empty.split_last(), None);
        assert_eq!(empty.parent(), None);
    }

    #[test]
    fn accepts_one_component() {
        let one = path("a");
        assert_eq!(names(one), ["a"]);
        assert_eq!(
            one.split_first(),
            Some((Name::new("a").unwrap(), RelPath::empty()))
        );
        assert_eq!(one.parent(), Some(RelPath::empty()));
    }

    #[test]
    fn accepts_many_components() {
        let many = path("a/.b/c..");
        assert_eq!(names(many), ["a", ".b", "c.."]);
        let reversed: Vec<&str> = many.components().rev().map(Name::as_str).collect();
        assert_eq!(reversed, ["c..", ".b", "a"]);
        assert_eq!(many.parent(), Some(path("a/.b")));
    }

    #[test]
    fn rejects_dot_components() {
        rejects(".", 0, NameError::DotOrDotDot);
        rejects("..", 0, NameError::DotOrDotDot);
        rejects("a/./b", 1, NameError::DotOrDotDot);
        rejects("a/..", 1, NameError::DotOrDotDot);
    }

    #[test]
    fn rejects_misplaced_slashes() {
        rejects("/", 0, NameError::Empty);
        rejects("/a", 0, NameError::Empty);
        rejects("a/", 1, NameError::Empty);
        rejects("a//b", 1, NameError::Empty);
    }

    #[test]
    fn rejects_nul() {
        rejects("a/b\0", 1, NameError::ContainsNul);
    }

    #[test]
    fn joins() {
        assert_eq!(RelPath::empty().join(path("a/b")), path("a/b"));
        assert_eq!(path("a").join(RelPath::empty()), path("a"));
        assert_eq!(path("a").join(Name::new("b").unwrap()), path("a/b"));
    }

    #[test]
    fn strips_prefixes_by_component() {
        let p = path("a/bc/d");
        assert_eq!(p.strip_prefix(RelPath::empty()), Some(p));
        assert_eq!(p.strip_prefix(path("a")), Some(path("bc/d")));
        assert_eq!(p.strip_prefix(path("a/bc/d")), Some(RelPath::empty()));
        assert_eq!(p.strip_prefix(path("a/b")), None);
        assert_eq!(p.strip_prefix(path("b")), None);
        assert!(p.starts_with(path("a/bc")));
        assert!(!path("a").starts_with(path("a/bc")));
    }

    #[test]
    fn orders_by_component() {
        assert!(path("a/b") < path("a-b"));
        assert!(path("") < path("a"));
        assert!(path("a") < path("a/b"));
    }
}

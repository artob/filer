// This is free and unencumbered software released into the public domain.

use super::{NameBuf, NameError, RelPath};
use ref_cast::{RefCastCustom, ref_cast_custom};

/// One valid path component, such as `app.toml`; borrowed form.
///
/// A `Name` is non-empty UTF-8, contains neither `/` nor NUL, and is
/// neither `.` nor `..`. Nothing else is rejected: control characters,
/// backslashes, colons, and trailing dots are all valid here.
///
/// Acceptance is lexical only. Whether a given directory accepts a name
/// (its length, case collisions, Windows reserved names, and so on) depends
/// on the filesystem and is decided by the driver at use.
///
/// The owned form is [`NameBuf`].
///
/// # Examples
///
/// ```
/// use filer_api::path::{Name, NameError};
///
/// let name = Name::new("app.toml").unwrap();
/// assert_eq!(name.as_str(), "app.toml");
/// assert_eq!(Name::new(".."), Err(NameError::DotOrDotDot));
/// ```
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, RefCastCustom)]
#[repr(transparent)]
pub struct Name(str);

impl Name {
    /// Checks that `s` is a valid name and borrows it as one.
    ///
    /// # Errors
    ///
    /// Returns the first rule `s` breaks; see [`NameError`].
    pub const fn new(s: &str) -> Result<&Self, NameError> {
        match validate(s.as_bytes()) {
            Ok(()) => Ok(Self::from_str_unchecked(s)),
            Err(error) => Err(error),
        }
    }

    /// Borrows `s` as a name without checking it; `s` must be valid.
    #[ref_cast_custom]
    pub(super) const fn from_str_unchecked(s: &str) -> &Self;

    /// Returns the name as a string slice.
    #[must_use]
    pub const fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns the name as a one-component [`RelPath`].
    #[must_use]
    pub const fn as_rel_path(&self) -> &RelPath {
        RelPath::from_str_unchecked(&self.0)
    }
}

/// Checks the rules documented on [`Name`], on the bytes of a `str`.
pub(super) const fn validate(bytes: &[u8]) -> Result<(), NameError> {
    match bytes {
        [] => return Err(NameError::Empty),
        [b'.'] | [b'.', b'.'] => return Err(NameError::DotOrDotDot),
        _ => {},
    }
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'/' => return Err(NameError::ContainsSlash),
            0 => return Err(NameError::ContainsNul),
            _ => i += 1,
        }
    }
    Ok(())
}

impl alloc::borrow::ToOwned for Name {
    type Owned = NameBuf;

    fn to_owned(&self) -> NameBuf {
        NameBuf::from(self)
    }
}

impl<'a> TryFrom<&'a str> for &'a Name {
    type Error = NameError;

    fn try_from(s: &'a str) -> Result<Self, NameError> {
        Name::new(s)
    }
}

impl AsRef<Name> for Name {
    fn as_ref(&self) -> &Name {
        self
    }
}

impl AsRef<RelPath> for Name {
    fn as_ref(&self) -> &RelPath {
        self.as_rel_path()
    }
}

impl AsRef<str> for Name {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl PartialEq<str> for Name {
    fn eq(&self, other: &str) -> bool {
        self.0 == *other
    }
}

impl PartialEq<NameBuf> for Name {
    fn eq(&self, other: &NameBuf) -> bool {
        *self == **other
    }
}

impl core::fmt::Debug for Name {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(&self.0, f)
    }
}

impl core::fmt::Display for Name {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_names() {
        for s in ["a", "app.toml", "...", ".hidden", "a\\b", "c:", "x.", "\t"] {
            assert_eq!(Name::new(s).map(Name::as_str), Ok(s));
        }
    }

    #[test]
    fn rejects_empty() {
        assert_eq!(Name::new(""), Err(NameError::Empty));
    }

    #[test]
    fn rejects_dot_and_dot_dot() {
        assert_eq!(Name::new("."), Err(NameError::DotOrDotDot));
        assert_eq!(Name::new(".."), Err(NameError::DotOrDotDot));
    }

    #[test]
    fn rejects_slash() {
        for s in ["/", "a/b", "/a", "a/"] {
            assert_eq!(Name::new(s), Err(NameError::ContainsSlash));
        }
    }

    #[test]
    fn rejects_nul() {
        for s in ["\0", "a\0b", "a\0"] {
            assert_eq!(Name::new(s), Err(NameError::ContainsNul));
        }
    }

    #[test]
    fn formats_like_a_string() {
        let name = Name::new("a b").unwrap();
        assert_eq!(alloc::format!("{name}"), "a b");
        assert_eq!(alloc::format!("{name:?}"), "\"a b\"");
    }
}

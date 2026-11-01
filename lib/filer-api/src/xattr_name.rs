// This is free and unencumbered software released into the public domain.

use crate::XattrNs;
use alloc::boxed::Box;

/// The name of an extended attribute: a namespace and a valid UTF-8 name
/// within it, written `user.name`.
///
/// The name within the namespace is not empty and has no NUL; it may
/// contain `.` and `/`. Length limits are the filesystem's, reported as
/// [`NameTooLong`](crate::ErrorKind::NameTooLong).
///
/// # Examples
///
/// ```
/// use filer_api::{XattrName, XattrNs};
///
/// let name: XattrName = "user.xdg.origin.url".parse().unwrap();
/// assert_eq!(name.ns(), XattrNs::User);
/// assert_eq!(name.name(), "xdg.origin.url");
/// assert_eq!(name.to_string(), "user.xdg.origin.url");
/// assert!("origin".parse::<XattrName>().is_err());
/// ```
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct XattrName {
    ns: XattrNs,
    name: Box<str>,
}

impl XattrName {
    /// Returns the attribute `name` in the namespace `ns`.
    ///
    /// # Errors
    ///
    /// [`XattrNameError::Empty`] or [`XattrNameError::ContainsNul`].
    pub fn new(ns: XattrNs, name: &str) -> Result<Self, XattrNameError> {
        check(name.as_bytes())?;
        Ok(Self {
            ns,
            name: Box::from(name),
        })
    }

    /// Returns the namespace.
    #[must_use]
    pub fn ns(&self) -> XattrNs {
        self.ns
    }

    /// Returns the name within the namespace.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// Checks a name within a namespace, in any encoding.
pub(crate) fn check(name: &[u8]) -> Result<(), XattrNameError> {
    if name.is_empty() {
        return Err(XattrNameError::Empty);
    }
    if name.contains(&0) {
        return Err(XattrNameError::ContainsNul);
    }
    Ok(())
}

impl core::str::FromStr for XattrName {
    type Err = XattrNameError;

    /// Parses `ns.name`, such as `user.comment`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (prefix, name) = s.split_once('.').ok_or(XattrNameError::NoNamespace)?;
        let ns = XattrNs::from_prefix(prefix).ok_or(XattrNameError::NoNamespace)?;
        Self::new(ns, name)
    }
}

impl TryFrom<&str> for XattrName {
    type Error = XattrNameError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl core::fmt::Display for XattrName {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}.{}", self.ns, self.name)
    }
}

/// Why a string is not a valid [`XattrName`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum XattrNameError {
    /// The name within the namespace is empty.
    Empty,

    /// The name contains a NUL character.
    ContainsNul,

    /// The string does not start with a known namespace and a `.`.
    NoNamespace,
}

impl core::fmt::Display for XattrNameError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::Empty => "attribute name is empty",
            Self::ContainsNul => "attribute name contains NUL",
            Self::NoNamespace => "attribute name lacks a namespace such as 'user.'",
        })
    }
}

impl core::error::Error for XattrNameError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_names() {
        let name: XattrName = "security.selinux".parse().unwrap();
        assert_eq!((name.ns(), name.name()), (XattrNs::Security, "selinux"));
        let name = XattrName::try_from("user.a/b..c").unwrap();
        assert_eq!(name.name(), "a/b..c");
    }

    #[test]
    fn rejects_invalid_names() {
        let cases = [
            ("user.", XattrNameError::Empty),
            ("user.a\0b", XattrNameError::ContainsNul),
            ("comment", XattrNameError::NoNamespace),
            ("os2.comment", XattrNameError::NoNamespace),
            (".comment", XattrNameError::NoNamespace),
        ];
        for (s, error) in cases {
            assert_eq!(s.parse::<XattrName>(), Err(error), "{s:?}");
        }
        assert_eq!(
            XattrName::new(XattrNs::User, ""),
            Err(XattrNameError::Empty)
        );
    }
}

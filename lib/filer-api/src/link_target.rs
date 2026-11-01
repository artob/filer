// This is free and unencumbered software released into the public domain.

use crate::escaped::Escaped;
use alloc::{boxed::Box, string::String, vec::Vec};

/// The target of a symlink, as read from disk.
///
/// A target is an arbitrary string the symlink stores, not a confined path:
/// it may be absolute or contain `..`, and is only interpreted (under the
/// handle's resolution policy) when the link is followed. Targets that are
/// not valid UTF-8 (on Windows, not well-formed UTF-16) are kept as bytes.
///
/// Reading a link yields a `LinkTarget`; creating one accepts only UTF-8.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum LinkTarget {
    /// A valid UTF-8 target.
    Utf8(String),

    /// A target that is not valid UTF-8: raw bytes on POSIX, WTF-8 on
    /// Windows.
    Foreign(Box<[u8]>),
}

impl LinkTarget {
    /// Classifies a target read from disk by whether it is valid UTF-8.
    #[must_use]
    pub fn from_bytes(raw: Vec<u8>) -> Self {
        match String::from_utf8(raw) {
            Ok(s) => Self::Utf8(s),
            Err(error) => Self::Foreign(error.into_bytes().into_boxed_slice()),
        }
    }

    /// Returns the target as a string, or `None` if it is not UTF-8.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Utf8(s) => Some(s),
            Self::Foreign(_) => None,
        }
    }

    /// Returns the target's bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Self::Utf8(s) => s.as_bytes(),
            Self::Foreign(raw) => raw,
        }
    }
}

impl core::fmt::Display for LinkTarget {
    /// Renders UTF-8 targets as is and others lossily (`\xNN`).
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Display::fmt(&Escaped(self.as_bytes()), f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{string::ToString, vec};

    #[test]
    fn keeps_utf8_targets_as_strings() {
        let target = LinkTarget::from_bytes(b"../etc/passwd".to_vec());
        assert_eq!(target, LinkTarget::Utf8("../etc/passwd".to_string()));
        assert_eq!(target.as_str(), Some("../etc/passwd"));
    }

    #[test]
    fn keeps_other_targets_as_bytes() {
        let target = LinkTarget::from_bytes(vec![b'/', 0xFF]);
        assert_eq!(target.as_str(), None);
        assert_eq!(target.as_bytes(), b"/\xFF");
        assert_eq!(target.to_string(), "/\\xFF");
    }
}

// This is free and unencumbered software released into the public domain.

use crate::{
    DriverId, EntryName, ForeignName,
    path::{Name, NameBuf, NameError},
};

/// The private credential of one driver instance.
///
/// A driver creates one key when it is constructed and keeps it private
/// (sharing it among clones through an `Arc` if needed). The key fixes the
/// driver's [`DriverId`] and is the only way to construct a [`ForeignName`]
/// carrying that id, so code outside the driver cannot forge foreign names
/// the driver would accept. Anyone may create a key, but names from it
/// carry an id no other driver has.
///
/// The key is deliberately neither `Clone` nor `Copy`.
///
/// # Examples
///
/// ```
/// use filer_api::{DriverKey, EntryName};
///
/// let key = DriverKey::new();
/// let utf8 = key.entry_name(b"app.toml").unwrap();
/// assert_eq!(utf8.as_name().map(|n| n.as_str()), Some("app.toml"));
///
/// let foreign = key.entry_name(b"caf\xE9").unwrap();
/// assert!(foreign.is_foreign());
/// assert_eq!(foreign.to_string(), "caf\\xE9");
/// ```
#[derive(Debug)]
pub struct DriverKey {
    id: DriverId,
}

impl DriverKey {
    /// Returns a key with a fresh [`DriverId`].
    ///
    /// # Panics
    ///
    /// Panics if the process has exhausted the id space; see [`DriverId`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            id: DriverId::next(),
        }
    }

    /// Returns the id of the driver holding this key.
    #[must_use]
    pub fn id(&self) -> DriverId {
        self.id
    }

    /// Classifies a name read from disk: valid UTF-8 becomes a checked
    /// [`Name`], anything else a [`ForeignName`] from this driver.
    ///
    /// On Windows, pass the name as WTF-8, so that only names with unpaired
    /// surrogates become foreign.
    ///
    /// # Errors
    ///
    /// Returns a [`NameError`] for bytes that are not a name at all: empty,
    /// containing `/` or NUL, or `.` or `..`. Directory listings should
    /// skip `.` and `..` before calling this.
    pub fn entry_name(&self, raw: &[u8]) -> Result<EntryName, NameError> {
        if let Ok(s) = core::str::from_utf8(raw) {
            return Name::new(s).map(|name| EntryName::Utf8(NameBuf::from(name)));
        }
        for &byte in raw {
            match byte {
                b'/' => return Err(NameError::ContainsSlash),
                0 => return Err(NameError::ContainsNul),
                _ => {},
            }
        }
        Ok(EntryName::Foreign(ForeignName::new(raw, self.id)))
    }
}

impl Default for DriverKey {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mints_distinct_ids() {
        let (a, b) = (DriverKey::new(), DriverKey::new());
        assert_ne!(a.id(), b.id());
    }

    #[test]
    fn classifies_utf8_names() {
        let key = DriverKey::new();
        let name = key.entry_name("café".as_bytes()).unwrap();
        assert_eq!(name, EntryName::Utf8("café".parse().unwrap()));
        assert_eq!(key.entry_name(b"."), Err(NameError::DotOrDotDot));
        assert_eq!(key.entry_name(b""), Err(NameError::Empty));
        assert_eq!(key.entry_name(b"a/b"), Err(NameError::ContainsSlash));
    }

    #[test]
    fn classifies_foreign_names() {
        let key = DriverKey::new();
        let EntryName::Foreign(name) = key.entry_name(b"\xFF").unwrap() else {
            panic!("expected a foreign name");
        };
        assert_eq!(name.as_bytes(), b"\xFF");
        assert_eq!(name.origin(), key.id());
        assert_eq!(key.entry_name(b"\xFF/"), Err(NameError::ContainsSlash));
        assert_eq!(key.entry_name(b"\xFF\0"), Err(NameError::ContainsNul));
    }

    #[test]
    fn distinguishes_foreign_names_by_origin() {
        let (a, b) = (DriverKey::new(), DriverKey::new());
        let raw = b"\xC0\xAF";
        assert_eq!(a.entry_name(raw), a.entry_name(raw));
        assert_ne!(a.entry_name(raw), b.entry_name(raw));
    }
}

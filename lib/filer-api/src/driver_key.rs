// This is free and unencumbered software released into the public domain.

use crate::{
    DriverId, EntryName, ForeignName, Handle, HandleKind, XattrEntry, XattrName, XattrNameError,
    XattrNs,
    path::{Name, NameBuf, NameError},
};
use core::any::Any;

/// The private credential of one driver instance.
///
/// A driver creates one key when it is constructed and keeps it private
/// (sharing it among clones through an `Arc` if needed). The key fixes the
/// driver's [`DriverId`] and is the only way to construct a [`ForeignName`]
/// or a [`Handle`] carrying that id, or to read a handle's payload, so code
/// outside the driver cannot forge names or handles the driver would
/// accept. Anyone may create a key, but what it mints carries an id no
/// other driver has.
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

    /// Classifies the name of an extended attribute in `ns` read from disk:
    /// valid UTF-8 becomes an [`XattrName`], anything else a foreign name
    /// from this driver. On Windows, pass the name as WTF-8.
    ///
    /// # Errors
    ///
    /// Returns an [`XattrNameError`] for an empty name or one containing
    /// NUL.
    pub fn xattr_entry(&self, ns: XattrNs, raw: &[u8]) -> Result<XattrEntry, XattrNameError> {
        crate::xattr_name::check(raw)?;
        if let Ok(s) = core::str::from_utf8(raw) {
            return XattrName::new(ns, s).map(XattrEntry::Utf8);
        }
        let name = ForeignName::new(raw, self.id);
        Ok(XattrEntry::Foreign { ns, name })
    }

    /// Mints a handle of `kind` for this driver, owning `payload`.
    ///
    /// The payload is the driver's private state for the open file or
    /// directory; it is dropped when the last clone of the handle is.
    pub fn handle<P: Any + Send + Sync>(&self, kind: HandleKind, payload: P) -> Handle {
        Handle::new(self.id, kind, payload)
    }

    /// Returns the payload of `handle` if this driver minted it with a
    /// payload of type `P`.
    ///
    /// `None` usually means the handle came from another driver instance,
    /// which drivers report as
    /// [`ErrorKind::CrossDriver`](crate::ErrorKind::CrossDriver); a payload
    /// of an unexpected type is a bug in the driver.
    #[must_use]
    pub fn payload<'h, P: Any>(&self, handle: &'h Handle) -> Option<&'h P> {
        if handle.driver() != self.id {
            return None;
        }
        handle.payload()
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
    fn classifies_xattr_names() {
        let key = DriverKey::new();
        let utf8 = key.xattr_entry(XattrNs::User, b"a/b").unwrap();
        assert_eq!(utf8, XattrEntry::Utf8("user.a/b".parse().unwrap()));
        let XattrEntry::Foreign { ns, name } = key.xattr_entry(XattrNs::User, b"/\xFF").unwrap()
        else {
            panic!("expected a foreign name");
        };
        assert_eq!((ns, name.as_bytes()), (XattrNs::User, &b"/\xFF"[..]));
        assert_eq!(name.origin(), key.id());
        let error = key.xattr_entry(XattrNs::User, b"\xFF\0");
        assert_eq!(error, Err(XattrNameError::ContainsNul));
        let error = key.xattr_entry(XattrNs::System, b"");
        assert_eq!(error, Err(XattrNameError::Empty));
    }

    #[test]
    fn mints_handles_with_payloads() {
        let key = DriverKey::new();
        let handle = key.handle(HandleKind::File, 7_u32);
        assert_eq!(handle.kind(), HandleKind::File);
        assert_eq!(handle.driver(), key.id());
        assert_eq!(key.payload::<u32>(&handle.clone()), Some(&7));
        assert_eq!(key.payload::<u64>(&handle), None);
    }

    #[test]
    fn rejects_handles_from_other_drivers() {
        let (a, b) = (DriverKey::new(), DriverKey::new());
        let handle = a.handle(HandleKind::Dir, 7_u32);
        assert_eq!(b.payload::<u32>(&handle), None);
    }

    #[test]
    fn drops_the_payload_with_the_last_clone() {
        let key = DriverKey::new();
        let probe = alloc::sync::Arc::new(());
        let handle = key.handle(HandleKind::File, probe.clone());
        let clone = handle.clone();
        drop(handle);
        assert_eq!(alloc::sync::Arc::strong_count(&probe), 2);
        drop(clone);
        assert_eq!(alloc::sync::Arc::strong_count(&probe), 1);
    }

    #[test]
    fn distinguishes_foreign_names_by_origin() {
        let (a, b) = (DriverKey::new(), DriverKey::new());
        let raw = b"\xC0\xAF";
        assert_eq!(a.entry_name(raw), a.entry_name(raw));
        assert_ne!(a.entry_name(raw), b.entry_name(raw));
    }
}

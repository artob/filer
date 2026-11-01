// This is free and unencumbered software released into the public domain.

use crate::{ErrorKind, Fallback, OpKind, RawOsError, path::RelPathBuf};
use alloc::boxed::Box;

/// A [`Result`](core::result::Result) whose error defaults to [`Error`].
pub type Result<T, E = Error> = core::result::Result<T, E>;

/// A boxed error that can serve as an [`Error`]'s source.
type Source = Box<dyn core::error::Error + Send + Sync>;

/// An error from a filesystem operation.
///
/// Besides its [`ErrorKind`] and the [`OpKind`] that failed, an error
/// carries whatever context the driver had: the path or paths involved
/// (always relative to the handle they were resolved against, never
/// absolute), the handle's optional label, the raw OS error code, any
/// [`Fallback`] taken, and an underlying source error.
///
/// The error is one pointer wide, so `Result<T>` stays small.
///
/// # Examples
///
/// ```
/// use filer_api::{Error, ErrorKind, OpKind, RawOsError, rel};
///
/// let error = Error::new(ErrorKind::NotFound, OpKind::Open)
///     .with_path(rel!("config/app.toml"))
///     .with_label("/srv/app")
///     .with_os(RawOsError::Errno(2));
/// assert_eq!(
///     error.to_string(),
///     r#"open "config/app.toml" in /srv/app: not found (os error 2)"#,
/// );
/// ```
pub struct Error(Box<Inner>);

struct Inner {
    kind: ErrorKind,
    op: OpKind,
    path: Option<RelPathBuf>,
    target: Option<RelPathBuf>,
    label: Option<Box<str>>,
    os: Option<RawOsError>,
    fallback: Option<Fallback>,
    source: Option<Source>,
}

impl Error {
    /// Returns an error of `kind` from the operation `op`, without context.
    #[must_use]
    pub fn new(kind: ErrorKind, op: OpKind) -> Self {
        Self(Box::new(Inner {
            kind,
            op,
            path: None,
            target: None,
            label: None,
            os: None,
            fallback: None,
            source: None,
        }))
    }

    /// Sets the path the operation acted on, relative to its handle.
    #[must_use]
    pub fn with_path(mut self, path: impl Into<RelPathBuf>) -> Self {
        self.0.path = Some(path.into());
        self
    }

    /// Sets the second path of a two-path operation, such as the
    /// destination of a rename, relative to its handle.
    #[must_use]
    pub fn with_target(mut self, target: impl Into<RelPathBuf>) -> Self {
        self.0.target = Some(target.into());
        self
    }

    /// Sets the label of the handle the paths are relative to, typically
    /// the native path it was opened from.
    #[must_use]
    pub fn with_label(mut self, label: &str) -> Self {
        self.0.label = Some(Box::from(label));
        self
    }

    /// Sets the OS error code the driver received.
    #[must_use]
    pub fn with_os(mut self, os: RawOsError) -> Self {
        self.0.os = Some(os);
        self
    }

    /// Records a fallback the driver took before failing.
    #[must_use]
    pub fn with_fallback(mut self, fallback: Fallback) -> Self {
        self.0.fallback = Some(fallback);
        self
    }

    /// Sets the underlying error, returned by
    /// [`source`](core::error::Error::source).
    #[must_use]
    pub fn with_source<E>(mut self, source: E) -> Self
    where
        E: core::error::Error + Send + Sync + 'static,
    {
        self.0.source = Some(Box::new(source));
        self
    }

    /// Returns the error's category.
    #[must_use]
    pub fn kind(&self) -> ErrorKind {
        self.0.kind
    }

    /// Returns the kind of operation that failed.
    #[must_use]
    pub fn op(&self) -> OpKind {
        self.0.op
    }

    /// Returns the path the operation acted on, if known.
    #[must_use]
    pub fn path(&self) -> Option<&crate::path::RelPath> {
        self.0.path.as_deref()
    }

    /// Returns the second path of a two-path operation, if any.
    #[must_use]
    pub fn target(&self) -> Option<&crate::path::RelPath> {
        self.0.target.as_deref()
    }

    /// Returns the label of the handle the paths are relative to, if set.
    #[must_use]
    pub fn label(&self) -> Option<&str> {
        self.0.label.as_deref()
    }

    /// Returns the OS error code the driver received, if any.
    #[must_use]
    pub fn os(&self) -> Option<RawOsError> {
        self.0.os
    }

    /// Returns the fallback the driver took before failing, if any.
    #[must_use]
    pub fn fallback(&self) -> Option<Fallback> {
        self.0.fallback
    }
}

impl core::fmt::Display for Error {
    /// Formats as `op "path" -> "target" in label: kind (os error N)`,
    /// omitting what is unknown.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let inner = &*self.0;
        write!(f, "{}", inner.op)?;
        if let Some(path) = &inner.path {
            write!(f, " {path:?}")?;
        }
        if let Some(target) = &inner.target {
            write!(f, " -> {target:?}")?;
        }
        if let Some(label) = &inner.label {
            write!(f, " in {label}")?;
        }
        write!(f, ": {}", inner.kind)?;
        if let Some(os) = &inner.os {
            write!(f, " ({os})")?;
        }
        if let Some(fallback) = &inner.fallback {
            write!(f, " (fallback: {fallback})")?;
        }
        Ok(())
    }
}

impl core::fmt::Debug for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let inner = &*self.0;
        f.debug_struct("Error")
            .field("kind", &inner.kind)
            .field("op", &inner.op)
            .field("path", &inner.path)
            .field("target", &inner.target)
            .field("label", &inner.label)
            .field("os", &inner.os)
            .field("fallback", &inner.fallback)
            .field("source", &inner.source)
            .finish()
    }
}

impl core::error::Error for Error {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match &self.0.source {
            Some(source) => Some(&**source),
            None => None,
        }
    }
}

#[cfg(feature = "std")]
impl From<Error> for std::io::Error {
    /// Converts to an I/O error of the closest `std::io::ErrorKind`,
    /// keeping the [`Error`] itself as the payload.
    fn from(error: Error) -> Self {
        Self::new(error.kind().into(), error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{name, rel};
    use alloc::string::ToString;
    use core::error::Error as _;

    #[test]
    fn displays_the_bare_minimum() {
        let error = Error::new(ErrorKind::Unsupported, OpKind::ReadLink);
        assert_eq!(error.to_string(), "read_link: unsupported");
    }

    #[test]
    fn displays_every_field() {
        let error = Error::new(ErrorKind::EscapeBlocked, OpKind::Open)
            .with_path(rel!("a/link"))
            .with_target(name!("b"))
            .with_label("<app>")
            .with_os(RawOsError::Errno(18))
            .with_fallback(Fallback::new("openat2", "userspace walk"));
        assert_eq!(
            error.to_string(),
            "open \"a/link\" -> \"b\" in <app>: escape blocked (os error 18) \
             (fallback: openat2 -> userspace walk)",
        );
    }

    #[test]
    fn displays_the_empty_path() {
        let error = Error::new(ErrorKind::PermissionDenied, OpKind::ReadDir)
            .with_path(crate::path::RelPath::empty());
        assert_eq!(error.to_string(), "read_dir \"\": permission denied");
    }

    #[test]
    fn keeps_context_and_source() {
        let source = crate::path::RelPath::new("..").unwrap_err();
        let error = Error::new(ErrorKind::InvalidName, OpKind::Stat)
            .with_os(RawOsError::Windows(123))
            .with_source(source);
        assert_eq!(error.kind(), ErrorKind::InvalidName);
        assert_eq!(error.op(), OpKind::Stat);
        assert_eq!(error.os(), Some(RawOsError::Windows(123)));
        assert_eq!(error.path(), None);
        assert_eq!(error.source().unwrap().to_string(), source.to_string());
    }

    #[test]
    fn is_one_pointer_wide() {
        assert_eq!(size_of::<Error>(), size_of::<usize>());
        assert_eq!(size_of::<Result<()>>(), size_of::<usize>());
    }

    #[cfg(feature = "std")]
    #[test]
    fn converts_to_io_error() {
        use std::io::ErrorKind as Io;
        let cases = [
            (ErrorKind::NotFound, Io::NotFound),
            (ErrorKind::RightsDenied, Io::PermissionDenied),
            (ErrorKind::EscapeBlocked, Io::PermissionDenied),
            (ErrorKind::InvalidNameForTarget, Io::InvalidFilename),
            (ErrorKind::ReadOnly, Io::ReadOnlyFilesystem),
            (ErrorKind::Busy, Io::ResourceBusy),
            (ErrorKind::Stale, Io::StaleNetworkFileHandle),
            (ErrorKind::SymlinkLoop, Io::Other),
            (ErrorKind::Poisoned, Io::Other),
        ];
        for (kind, io_kind) in cases {
            let io_error = std::io::Error::from(Error::new(kind, OpKind::Read));
            assert_eq!(io_error.kind(), io_kind, "{kind:?}");
            let payload = io_error.into_inner().unwrap().downcast::<Error>().unwrap();
            assert_eq!(payload.kind(), kind);
        }
    }
}

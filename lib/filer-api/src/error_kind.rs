// This is free and unencumbered software released into the public domain.

/// The category of an [`Error`](crate::Error).
///
/// Drivers map OS error codes onto these kinds; the original code stays
/// available through [`Error::os`](crate::Error::os). Under the `std`
/// feature each kind converts to the closest `std::io::ErrorKind`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ErrorKind {
    /// The entry does not exist (`ENOENT`).
    NotFound,

    /// The entry already exists (`EEXIST`).
    AlreadyExists,

    /// A directory was expected (`ENOTDIR`).
    NotADirectory,

    /// A non-directory was expected (`EISDIR`).
    IsADirectory,

    /// A regular file was expected but the entry is a FIFO, socket, device,
    /// or other special file.
    NotRegularFile,

    /// A symlink was expected, as when reading a link target (`EINVAL`
    /// from `readlink`).
    NotASymlink,

    /// The directory is not empty (`ENOTEMPTY`).
    DirectoryNotEmpty,

    /// The OS refused access (`EACCES`, `EPERM`).
    PermissionDenied,

    /// The handle lacks the rights the operation requires. Unlike
    /// [`PermissionDenied`](Self::PermissionDenied), this is enforced by
    /// the library and the operation never reached the driver.
    RightsDenied,

    /// Resolution would have left the subtree beneath the handle, and
    /// confinement stopped it.
    EscapeBlocked,

    /// Too many symlinks were met during resolution (`ELOOP`).
    SymlinkLoop,

    /// A name or path is too long for the filesystem (`ENAMETOOLONG`).
    NameTooLong,

    /// A name or path is lexically invalid.
    InvalidName,

    /// A name is lexically valid but not acceptable in the target
    /// directory, such as a Windows reserved name.
    InvalidNameForTarget,

    /// A foreign (non-UTF-8) name was used with a handle from a different
    /// driver instance than the one that produced it.
    ForeignNameMismatch,

    /// A handle was passed to a driver instance that did not mint it.
    CrossDriver,

    /// The operation would cross filesystems or mounts (`EXDEV`).
    CrossesDevices,

    /// The filesystem or driver is read-only (`EROFS`).
    ReadOnly,

    /// The device has no space left (`ENOSPC`).
    StorageFull,

    /// A disk quota is exhausted (`EDQUOT`).
    QuotaExceeded,

    /// The driver, filesystem, or platform does not support the operation.
    Unsupported,

    /// The entry is in use (`EBUSY`, or a Windows sharing violation).
    Busy,

    /// A network file handle is stale (`ESTALE`).
    Stale,

    /// An earlier sync on this handle failed, so the state of data written
    /// since the last successful sync is unknown. Retrying does not help;
    /// recover from your own copy or log.
    Poisoned,

    /// A chained operation did not run because an earlier one failed.
    Canceled,

    /// The operation was interrupted.
    ///
    /// Drivers restart system calls that fail with `EINTR` themselves, so
    /// this never reports a signal interrupting a call; it is reserved for
    /// interruptions a driver cannot retry transparently.
    Interrupted,

    /// The operation timed out.
    TimedOut,

    /// A read reached the end of the file before filling its buffer.
    UnexpectedEof,

    /// A walk found that a reopened ancestor directory is no longer the
    /// directory it recorded.
    Moved,

    /// Data read from the filesystem is malformed.
    Corrupt,

    /// A low-level I/O error (`EIO`).
    Io,

    /// Any other error.
    Other,
}

impl core::fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::NotFound => "not found",
            Self::AlreadyExists => "already exists",
            Self::NotADirectory => "not a directory",
            Self::IsADirectory => "is a directory",
            Self::NotRegularFile => "not a regular file",
            Self::NotASymlink => "not a symlink",
            Self::DirectoryNotEmpty => "directory not empty",
            Self::PermissionDenied => "permission denied",
            Self::RightsDenied => "rights denied",
            Self::EscapeBlocked => "escape blocked",
            Self::SymlinkLoop => "too many symlinks",
            Self::NameTooLong => "name too long",
            Self::InvalidName => "invalid name",
            Self::InvalidNameForTarget => "invalid name for target",
            Self::ForeignNameMismatch => "foreign name from another driver",
            Self::CrossDriver => "handle from another driver",
            Self::CrossesDevices => "crosses devices",
            Self::ReadOnly => "read-only",
            Self::StorageFull => "storage full",
            Self::QuotaExceeded => "quota exceeded",
            Self::Unsupported => "unsupported",
            Self::Busy => "busy",
            Self::Stale => "stale handle",
            Self::Poisoned => "poisoned by an earlier sync failure",
            Self::Canceled => "canceled",
            Self::Interrupted => "interrupted",
            Self::TimedOut => "timed out",
            Self::UnexpectedEof => "unexpected end of file",
            Self::Moved => "moved",
            Self::Corrupt => "corrupt",
            Self::Io => "I/O error",
            Self::Other => "other error",
        })
    }
}

#[cfg(feature = "std")]
impl From<ErrorKind> for std::io::ErrorKind {
    /// Maps to the closest `std::io::ErrorKind`. Kinds without a stable
    /// counterpart, such as [`ErrorKind::SymlinkLoop`], map to `Other`.
    fn from(kind: ErrorKind) -> Self {
        use std::io::ErrorKind as Io;
        match kind {
            ErrorKind::NotFound => Io::NotFound,
            ErrorKind::AlreadyExists => Io::AlreadyExists,
            ErrorKind::NotADirectory => Io::NotADirectory,
            ErrorKind::IsADirectory => Io::IsADirectory,
            ErrorKind::NotRegularFile => Io::InvalidInput,
            ErrorKind::NotASymlink => Io::InvalidInput,
            ErrorKind::DirectoryNotEmpty => Io::DirectoryNotEmpty,
            ErrorKind::PermissionDenied => Io::PermissionDenied,
            ErrorKind::RightsDenied => Io::PermissionDenied,
            ErrorKind::EscapeBlocked => Io::PermissionDenied,
            ErrorKind::NameTooLong => Io::InvalidFilename,
            ErrorKind::InvalidName => Io::InvalidFilename,
            ErrorKind::InvalidNameForTarget => Io::InvalidFilename,
            ErrorKind::ForeignNameMismatch => Io::InvalidInput,
            ErrorKind::CrossDriver => Io::InvalidInput,
            ErrorKind::CrossesDevices => Io::CrossesDevices,
            ErrorKind::ReadOnly => Io::ReadOnlyFilesystem,
            ErrorKind::StorageFull => Io::StorageFull,
            ErrorKind::QuotaExceeded => Io::QuotaExceeded,
            ErrorKind::Unsupported => Io::Unsupported,
            ErrorKind::Busy => Io::ResourceBusy,
            ErrorKind::Stale => Io::StaleNetworkFileHandle,
            ErrorKind::Interrupted => Io::Interrupted,
            ErrorKind::TimedOut => Io::TimedOut,
            ErrorKind::UnexpectedEof => Io::UnexpectedEof,
            ErrorKind::Corrupt => Io::InvalidData,
            ErrorKind::SymlinkLoop
            | ErrorKind::Poisoned
            | ErrorKind::Canceled
            | ErrorKind::Moved
            | ErrorKind::Io
            | ErrorKind::Other => Io::Other,
        }
    }
}

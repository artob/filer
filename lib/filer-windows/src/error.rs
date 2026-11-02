// This is free and unencumbered software released into the public domain.

use crate::sys;
use filer_api::{Error, ErrorKind, OpKind, RawOsError};
use windows_sys::Win32::Foundation::{
    ERROR_ACCESS_DENIED, ERROR_ALREADY_EXISTS, ERROR_BAD_NET_NAME, ERROR_BAD_NETPATH,
    ERROR_BAD_PATHNAME, ERROR_BUSY, ERROR_CALL_NOT_IMPLEMENTED, ERROR_CANT_RESOLVE_FILENAME,
    ERROR_CRC, ERROR_DIR_NOT_EMPTY, ERROR_DIRECTORY, ERROR_DISK_CORRUPT, ERROR_DISK_FULL,
    ERROR_DISK_QUOTA_EXCEEDED, ERROR_FILE_CORRUPT, ERROR_FILE_EXISTS, ERROR_FILE_NOT_FOUND,
    ERROR_FILENAME_EXCED_RANGE, ERROR_HANDLE_DISK_FULL, ERROR_INVALID_DRIVE,
    ERROR_INVALID_FUNCTION, ERROR_INVALID_NAME, ERROR_IO_DEVICE, ERROR_LOCK_VIOLATION,
    ERROR_NOT_A_REPARSE_POINT, ERROR_NOT_SAME_DEVICE, ERROR_NOT_SUPPORTED, ERROR_OPERATION_ABORTED,
    ERROR_PATH_NOT_FOUND, ERROR_PRIVILEGE_NOT_HELD, ERROR_SEM_TIMEOUT, ERROR_SHARING_VIOLATION,
    ERROR_TIMEOUT, ERROR_USER_MAPPED_FILE, ERROR_WRITE_PROTECT,
};
use windows_sys::Win32::Foundation::{
    NTSTATUS, STATUS_DELETE_PENDING, STATUS_FILE_IS_A_DIRECTORY, STATUS_NOT_A_DIRECTORY,
    STATUS_OBJECT_NAME_INVALID,
};

/// Windows system error codes and the kinds they map to.
const KINDS: &[(u32, ErrorKind)] = &[
    (ERROR_FILE_NOT_FOUND, ErrorKind::NotFound),
    (ERROR_PATH_NOT_FOUND, ErrorKind::NotFound),
    (ERROR_INVALID_DRIVE, ErrorKind::NotFound),
    (ERROR_BAD_NETPATH, ErrorKind::NotFound),
    (ERROR_BAD_NET_NAME, ErrorKind::NotFound),
    (ERROR_FILE_EXISTS, ErrorKind::AlreadyExists),
    (ERROR_ALREADY_EXISTS, ErrorKind::AlreadyExists),
    (ERROR_DIRECTORY, ErrorKind::NotADirectory),
    (ERROR_DIR_NOT_EMPTY, ErrorKind::DirectoryNotEmpty),
    (ERROR_ACCESS_DENIED, ErrorKind::PermissionDenied),
    (ERROR_PRIVILEGE_NOT_HELD, ErrorKind::PermissionDenied),
    (ERROR_CANT_RESOLVE_FILENAME, ErrorKind::SymlinkLoop),
    (ERROR_NOT_A_REPARSE_POINT, ErrorKind::NotASymlink),
    (ERROR_FILENAME_EXCED_RANGE, ErrorKind::NameTooLong),
    (ERROR_INVALID_NAME, ErrorKind::InvalidNameForTarget),
    (ERROR_BAD_PATHNAME, ErrorKind::InvalidNameForTarget),
    (ERROR_NOT_SAME_DEVICE, ErrorKind::CrossesDevices),
    (ERROR_WRITE_PROTECT, ErrorKind::ReadOnly),
    (ERROR_DISK_FULL, ErrorKind::StorageFull),
    (ERROR_HANDLE_DISK_FULL, ErrorKind::StorageFull),
    (ERROR_DISK_QUOTA_EXCEEDED, ErrorKind::QuotaExceeded),
    (ERROR_NOT_SUPPORTED, ErrorKind::Unsupported),
    (ERROR_INVALID_FUNCTION, ErrorKind::Unsupported),
    (ERROR_CALL_NOT_IMPLEMENTED, ErrorKind::Unsupported),
    (ERROR_SHARING_VIOLATION, ErrorKind::Busy),
    (ERROR_LOCK_VIOLATION, ErrorKind::Busy),
    (ERROR_BUSY, ErrorKind::Busy),
    (ERROR_USER_MAPPED_FILE, ErrorKind::Busy),
    (ERROR_TIMEOUT, ErrorKind::TimedOut),
    (ERROR_SEM_TIMEOUT, ErrorKind::TimedOut),
    (ERROR_IO_DEVICE, ErrorKind::Io),
    (ERROR_CRC, ErrorKind::Io),
    (ERROR_FILE_CORRUPT, ErrorKind::Corrupt),
    (ERROR_DISK_CORRUPT, ErrorKind::Corrupt),
    // Synchronous I/O canceled by another thread, which no retry undoes.
    (ERROR_OPERATION_ABORTED, ErrorKind::Interrupted),
];

/// Converts a Windows system error code from `op` into an [`Error`],
/// keeping the code.
pub(crate) fn win32(code: u32, op: OpKind) -> Error {
    Error::new(kind(code), op).with_os(RawOsError::Windows(code))
}

/// Returns the kind of the Windows system error code `code`.
fn kind(code: u32) -> ErrorKind {
    KINDS
        .iter()
        .find(|(c, _)| *c == code)
        .map_or(ErrorKind::Other, |(_, kind)| *kind)
}

/// `NTSTATUS` values that their Windows system error codes misstate (a
/// file opened as a directory, for one, is `ERROR_ACCESS_DENIED`), and
/// the kinds they map to.
const STATUS_KINDS: &[(NTSTATUS, ErrorKind)] = &[
    (STATUS_FILE_IS_A_DIRECTORY, ErrorKind::IsADirectory),
    (STATUS_NOT_A_DIRECTORY, ErrorKind::NotADirectory),
    (STATUS_OBJECT_NAME_INVALID, ErrorKind::InvalidNameForTarget),
    // Deleted, but still open somewhere: gone as far as opening goes.
    (STATUS_DELETE_PENDING, ErrorKind::NotFound),
];

/// Converts an `NTSTATUS` from `op` into an [`Error`], keeping the Windows
/// system error code it stands for.
pub(crate) fn nt(status: NTSTATUS, op: OpKind) -> Error {
    let code = sys::win32(status);
    let kind = STATUS_KINDS
        .iter()
        .find(|(s, _)| *s == status)
        .map_or_else(|| kind(code), |(_, kind)| *kind);
    Error::new(kind, op).with_os(RawOsError::Windows(code))
}

/// Converts an error from the standard library, which keeps the system
/// error code, into an [`Error`].
pub(crate) fn io(error: &std::io::Error, op: OpKind) -> Error {
    match error.raw_os_error() {
        Some(code) => win32(code.cast_unsigned(), op),
        None => Error::new(ErrorKind::Other, op),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_codes_and_keeps_them() {
        let e = win32(ERROR_FILE_NOT_FOUND, OpKind::Open);
        assert_eq!(e.kind(), ErrorKind::NotFound);
        assert_eq!(e.os(), Some(RawOsError::Windows(ERROR_FILE_NOT_FOUND)));
        assert_eq!(
            win32(ERROR_SHARING_VIOLATION, OpKind::Open).kind(),
            ErrorKind::Busy
        );
        assert_eq!(win32(0xFFFF, OpKind::Read).kind(), ErrorKind::Other);
    }

    #[test]
    fn maps_statuses_by_their_codes_or_themselves() {
        let e = nt(STATUS_FILE_IS_A_DIRECTORY, OpKind::Open);
        assert_eq!(e.kind(), ErrorKind::IsADirectory);
        assert_eq!(e.os(), Some(RawOsError::Windows(ERROR_ACCESS_DENIED)));
        let e = nt(
            windows_sys::Win32::Foundation::STATUS_OBJECT_NAME_NOT_FOUND,
            OpKind::Open,
        );
        assert_eq!(e.kind(), ErrorKind::NotFound);
        assert_eq!(e.os(), Some(RawOsError::Windows(ERROR_FILE_NOT_FOUND)));
    }
}

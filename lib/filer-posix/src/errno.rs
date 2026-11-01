// This is free and unencumbered software released into the public domain.

use filer_api::{Error, ErrorKind, OpKind, RawOsError};
use rustix::io::Errno;

/// Errno values and the kinds they map to. A table rather than a `match`,
/// because some values coincide on some platforms (such as `ENOTSUP` and
/// `EOPNOTSUPP` on Linux), which a `match` would reject as unreachable.
const KINDS: &[(Errno, ErrorKind)] = &[
    (Errno::NOENT, ErrorKind::NotFound),
    (Errno::EXIST, ErrorKind::AlreadyExists),
    (Errno::NOTDIR, ErrorKind::NotADirectory),
    (Errno::ISDIR, ErrorKind::IsADirectory),
    (Errno::NOTEMPTY, ErrorKind::DirectoryNotEmpty),
    (Errno::ACCESS, ErrorKind::PermissionDenied),
    (Errno::PERM, ErrorKind::PermissionDenied),
    (Errno::LOOP, ErrorKind::SymlinkLoop),
    (Errno::NAMETOOLONG, ErrorKind::NameTooLong),
    (Errno::XDEV, ErrorKind::CrossesDevices),
    (Errno::ROFS, ErrorKind::ReadOnly),
    (Errno::NOSPC, ErrorKind::StorageFull),
    (Errno::DQUOT, ErrorKind::QuotaExceeded),
    (Errno::NOTSUP, ErrorKind::Unsupported),
    (Errno::OPNOTSUPP, ErrorKind::Unsupported),
    (Errno::NOSYS, ErrorKind::Unsupported),
    (Errno::BUSY, ErrorKind::Busy),
    (Errno::STALE, ErrorKind::Stale),
    (Errno::TIMEDOUT, ErrorKind::TimedOut),
    (Errno::IO, ErrorKind::Io),
    (Errno::ILSEQ, ErrorKind::InvalidNameForTarget),
];

/// Converts an errno from `op` into an [`Error`], keeping the raw code.
///
/// Never called with `EINTR`: every system call goes through [`retry`] or,
/// for directory streams, reopens them, so interruptions never surface.
pub(crate) fn error(errno: Errno, op: OpKind) -> Error {
    debug_assert_ne!(errno, Errno::INTR, "EINTR must be retried, not reported");
    let kind = KINDS
        .iter()
        .find(|(e, _)| *e == errno)
        .map_or(ErrorKind::Other, |(_, kind)| *kind);
    Error::new(kind, op).with_os(RawOsError::Errno(errno.raw_os_error()))
}

/// Runs `call` until it does not fail with `EINTR`. Every system call in
/// this crate goes through here, so an interrupted call is restarted, never
/// reported.
pub(crate) fn retry<T>(mut call: impl FnMut() -> rustix::io::Result<T>) -> rustix::io::Result<T> {
    loop {
        match call() {
            Err(Errno::INTR) => {},
            result => return result,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_errnos_and_keeps_the_code() {
        let e = error(Errno::NOENT, OpKind::Open);
        assert_eq!(e.kind(), ErrorKind::NotFound);
        assert_eq!(e.os(), Some(RawOsError::Errno(Errno::NOENT.raw_os_error())));
        assert_eq!(
            error(Errno::OPNOTSUPP, OpKind::Read).kind(),
            ErrorKind::Unsupported
        );
        assert_eq!(error(Errno::FAULT, OpKind::Read).kind(), ErrorKind::Other);
    }

    #[test]
    fn retries_interrupted_calls() {
        let mut calls = 0;
        let result = retry(|| {
            calls += 1;
            if calls < 3 {
                Err(Errno::INTR)
            } else {
                Ok(calls)
            }
        });
        assert_eq!(result, Ok(3));
        assert_eq!(retry(|| Err::<(), _>(Errno::NOENT)), Err(Errno::NOENT));
    }
}

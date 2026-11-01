// This is free and unencumbered software released into the public domain.

//! Kernel-confined path resolution with Linux `openat2` (5.6+).

use crate::errno::retry;
use filer_api::{HandleKind, Resolve, Symlinks, path::RelPath};
use rustix::{
    fd::{BorrowedFd, OwnedFd},
    fs::{Mode, OFlags, ResolveFlags},
    io::Errno,
};

/// How many times an `EAGAIN` (a rename racing the lookup) is retried
/// before falling back to the userspace walk.
const AGAIN: usize = 8;

/// What a kernel-confined open produced.
pub(crate) enum Opened {
    /// The open succeeded.
    Fd(OwnedFd),

    /// The open failed, and the error is the right one to report.
    Failed(Errno),

    /// `openat2` is not available: an old kernel, or a seccomp filter
    /// (`ENOSYS` or `EPERM`). The driver should stop trying it.
    Unavailable,

    /// The kernel's answer is not precise enough: `EXDEV` means either an
    /// escape or a mount crossing, and `EAGAIN` kept recurring. The
    /// userspace walk should answer instead.
    Inconclusive,
}

/// Opens `path` beneath `dir` with `openat2`, confined per `resolve`,
/// following a final symlink as the policy allows.
///
/// Directories open with `O_DIRECTORY`; files open with `O_NONBLOCK`, so
/// that a FIFO cannot block the open, for the caller to check and clear.
pub(crate) fn open(
    dir: BorrowedFd<'_>,
    path: &RelPath,
    kind: HandleKind,
    resolve: Resolve,
) -> Opened {
    let mut flags = ResolveFlags::NO_MAGICLINKS;
    flags |= match resolve.symlinks {
        Symlinks::Deny => ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS,
        Symlinks::InRoot => ResolveFlags::IN_ROOT,
        _ => ResolveFlags::BENEATH,
    };
    if !resolve.cross_mounts {
        flags |= ResolveFlags::NO_XDEV;
    }
    let mut oflags = OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOCTTY;
    oflags |= match kind {
        HandleKind::Dir => OFlags::DIRECTORY,
        _ => OFlags::NONBLOCK,
    };
    let path = path.as_str();
    for _ in 0..AGAIN {
        match retry(|| rustix::fs::openat2(dir, path, oflags, Mode::empty(), flags)) {
            Ok(fd) => return Opened::Fd(fd),
            Err(Errno::AGAIN) => {},
            Err(Errno::NOSYS | Errno::PERM) => return Opened::Unavailable,
            Err(Errno::XDEV) => return Opened::Inconclusive,
            Err(errno) => return Opened::Failed(errno),
        }
    }
    Opened::Inconclusive
}

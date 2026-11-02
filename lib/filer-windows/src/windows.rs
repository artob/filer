// This is free and unencumbered software released into the public domain.

use crate::{error, node::DirPayload};
use core::future::Future;
use filer_api::{
    AmbientAuthority, Completion, Dir, Driver, DriverId, DriverKey, Error, ErrorKind, Handle,
    HandleKind, Op, OpKind, Output, Spawner, run_on,
};
use std::{
    fs::{File, OpenOptions},
    os::windows::fs::OpenOptionsExt,
    path::Path,
    sync::Arc,
};
use windows_sys::Win32::Storage::FileSystem::{
    FILE_FLAG_BACKUP_SEMANTICS, FILE_LIST_DIRECTORY, FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE,
    FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_TRAVERSE, SYNCHRONIZE,
};

/// The access a directory handle holds: to list it, to open entries
/// relative to it, and to read its attributes.
pub(crate) const DIR_ACCESS: u32 =
    FILE_LIST_DIRECTORY | FILE_TRAVERSE | FILE_READ_ATTRIBUTES | SYNCHRONIZE;

/// Every share mode, so that a Filer handle never stops another program
/// from reading, writing, renaming, or deleting a file.
pub(crate) const SHARE_ALL: u32 = FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE;

/// The Windows driver, over [`windows-sys`](https://docs.rs/windows-sys).
///
/// Roots are opened with `CreateFileW` and `FILE_FLAG_BACKUP_SEMANTICS`,
/// through the standard library, which adds the `\\?\` prefix that long
/// paths need. Every handle is opened with all share modes, including
/// `FILE_SHARE_DELETE`, so that it never stops another program from
/// renaming or deleting a file.
///
/// Windows has no `EINTR`: synchronous I/O is interrupted only when
/// another thread cancels it, which surfaces as
/// [`ErrorKind::Interrupted`].
///
/// # Examples
///
/// ```
/// use filer_api::ambient_authority;
/// use filer_windows::Windows;
///
/// let windows = Windows::blocking();
/// let tmp = windows.open_ambient(std::env::temp_dir(), ambient_authority())?;
/// # Ok::<(), filer_api::Error>(())
/// ```
#[derive(Clone)]
pub struct Windows {
    key: Arc<DriverKey>,
    /// Where operations run: on a spawner's threads, or, if `None`, inside
    /// `poll`.
    spawner: Option<Arc<dyn Spawner>>,
}

impl Windows {
    /// Returns a driver that performs each operation synchronously when its
    /// future is first polled, **blocking the calling thread** until the
    /// system calls return.
    ///
    /// That suits command-line tools, tests, and single-threaded programs.
    /// On a multi-threaded async runtime, a slow call (a cold disk, a hung
    /// network share) stalls every task sharing the thread; use
    /// [`thread_pool`](Self::thread_pool) there.
    #[must_use]
    pub fn blocking() -> Self {
        Self {
            key: Arc::new(DriverKey::new()),
            spawner: None,
        }
    }

    /// Returns a driver that performs each operation as a job on
    /// `spawner`, such as a [`DefaultPool`](filer_api::DefaultPool), so
    /// that the system calls block a worker thread rather than the async
    /// executor.
    ///
    /// The job is submitted when the operation's future is first polled.
    /// Once submitted, it runs to completion even if the future is
    /// dropped. If the job panics, the operation fails with
    /// [`ErrorKind::Other`].
    ///
    /// ```
    /// use filer_api::DefaultPool;
    /// use filer_windows::Windows;
    ///
    /// let windows = Windows::thread_pool(DefaultPool::new());
    /// ```
    #[must_use]
    pub fn thread_pool(spawner: impl Spawner) -> Self {
        Self {
            spawner: Some(Arc::new(spawner)),
            ..Self::blocking()
        }
    }

    /// Opens the directory at the native `path` as a root for this driver.
    ///
    /// This is the one place native paths enter, so it requires the
    /// [`AmbientAuthority`] token. The handle's error label is `path`.
    ///
    /// # Errors
    ///
    /// Any error opening `path`, and
    /// [`NotADirectory`](ErrorKind::NotADirectory) if it is not a
    /// directory.
    pub fn open_ambient_dir(
        &self,
        path: impl AsRef<Path>,
        authority: AmbientAuthority,
    ) -> Result<Handle, Error> {
        let _ = authority;
        let path = path.as_ref();
        let op = OpKind::Open;
        let label = |error: Error| error.with_label(&path.to_string_lossy());
        let file = OpenOptions::new()
            .access_mode(DIR_ACCESS)
            .share_mode(SHARE_ALL)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
            .open(path)
            .map_err(|e| label(error::io(&e, op)))?;
        let metadata = file.metadata().map_err(|e| label(error::io(&e, op)))?;
        if !metadata.is_dir() {
            return Err(label(Error::new(ErrorKind::NotADirectory, op)));
        }
        Ok(self.dir_handle(Arc::new(file)))
    }

    /// Opens the directory at the native `path` as a root [`Dir`], with all
    /// rights and `path` as its label, for the frontend.
    ///
    /// # Errors
    ///
    /// As for [`open_ambient_dir`](Self::open_ambient_dir).
    pub fn open_ambient(
        &self,
        path: impl AsRef<Path>,
        authority: AmbientAuthority,
    ) -> Result<Dir<Self>, Error> {
        let path = path.as_ref();
        let handle = self.open_ambient_dir(path, authority)?;
        let dir = Dir::new(self.clone(), handle)?;
        Ok(dir.with_label(&path.to_string_lossy()))
    }

    fn dir_handle(&self, file: Arc<File>) -> Handle {
        self.key.handle(HandleKind::Dir, DirPayload::new(file))
    }

    /// Performs `op` synchronously.
    fn perform(&self, op: Op) -> Completion {
        let kind = op.kind();
        match op {
            Op::Close { handle } => Completion::new(self.dir(&handle, kind).map(|_| Output::Done)),
            op => op.unsupported(),
        }
    }

    /// Returns a directory handle's payload.
    fn dir<'h>(&self, handle: &'h Handle, op: OpKind) -> Result<&'h DirPayload, Error> {
        self.key
            .payload::<DirPayload>(handle)
            .ok_or_else(|| Error::new(ErrorKind::CrossDriver, op))
    }
}

impl Driver for Windows {
    fn id(&self) -> DriverId {
        self.key.id()
    }

    fn submit(&self, op: Op) -> impl Future<Output = Completion> + Send + use<> {
        let windows = self.clone();
        async move {
            let Some(spawner) = windows.spawner.clone() else {
                return windows.perform(op);
            };
            let kind = op.kind();
            match run_on(spawner, move || windows.perform(op)).await {
                Some(completion) => completion,
                // The job panicked; its buffer, if any, is lost with it.
                None => Completion::new(Err(Error::new(ErrorKind::Other, kind))),
            }
        }
    }
}

impl core::fmt::Debug for Windows {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Windows")
            .field("id", &self.key.id())
            .field("thread_pool", &self.spawner.is_some())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::task::{Context, Poll, Waker};
    use filer_api::ambient_authority;
    use std::{fs, process, string::ToString};

    fn block_on<F: Future>(future: F) -> F::Output {
        let mut future = core::pin::pin!(future);
        let mut cx = Context::from_waker(Waker::noop());
        loop {
            if let Poll::Ready(output) = future.as_mut().poll(&mut cx) {
                return output;
            }
        }
    }

    #[test]
    fn opens_and_closes_roots() {
        for windows in [
            Windows::blocking(),
            Windows::thread_pool(filer_api::DefaultPool::new()),
        ] {
            let root = windows
                .open_ambient_dir(std::env::temp_dir(), ambient_authority())
                .unwrap();
            let completion = block_on(windows.submit(Op::Close { handle: root }));
            assert!(matches!(completion.result, Ok(Output::Done)));
            let other = Windows::blocking()
                .open_ambient_dir(std::env::temp_dir(), ambient_authority())
                .unwrap();
            let completion = block_on(windows.submit(Op::Close { handle: other }));
            let error = completion.result.unwrap_err();
            assert_eq!(error.kind(), ErrorKind::CrossDriver);
        }
    }

    #[test]
    fn labels_ambient_errors() {
        let windows = Windows::blocking();
        let error = windows
            .open_ambient_dir(r"C:\nonexistent\filer", ambient_authority())
            .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::NotFound);
        assert_eq!(error.label(), Some(r"C:\nonexistent\filer"));
        assert!(error.os().is_some());
    }

    #[test]
    fn refuses_files_as_roots() {
        let path = std::env::temp_dir().join(std::format!("filer-windows-{}", process::id()));
        fs::write(&path, "").unwrap();
        let error = Windows::blocking()
            .open_ambient_dir(&path, ambient_authority())
            .unwrap_err();
        fs::remove_file(&path).unwrap();
        assert_eq!(error.kind(), ErrorKind::NotADirectory);
        let label = path.to_string_lossy().to_string();
        assert_eq!(error.label(), Some(label.as_str()));
    }

    /// A root's path may be longer than `MAX_PATH`.
    #[test]
    fn opens_long_paths() {
        let base = std::env::temp_dir().join(std::format!("filer-windows-long-{}", process::id()));
        let mut path = base.clone();
        while path.as_os_str().len() < 300 {
            path.push("a".repeat(50));
        }
        fs::create_dir_all(&path).unwrap();
        let opened = Windows::blocking().open_ambient(&path, ambient_authority());
        fs::remove_dir_all(&base).unwrap();
        opened.unwrap();
    }
}

// This is free and unencumbered software released into the public domain.

use core::future::Future;
use filer_api::{AmbientAuthority, Completion, Dir, Driver, DriverId, Error, Op};
use std::path::Path;

/// The native driver for this target.
#[cfg(unix)]
type Native = filer_posix::Posix;

/// No native driver exists for this target yet.
#[cfg(not(unix))]
type Native = crate::unsupported::Unsupported;

/// The platform's native filesystem driver.
///
/// `System` delegates every operation to the driver for the target it is
/// built for (see the [crate docs](crate)), with the same API everywhere,
/// so portable code names only `System`. Clones share one driver instance.
#[derive(Clone, Debug)]
pub struct System(Native);

impl System {
    /// Returns a driver that performs each operation synchronously when its
    /// future is first polled, **blocking the calling thread** until the
    /// system calls return.
    ///
    /// That suits command-line tools, tests, and single-threaded programs;
    /// on a multi-threaded async runtime, a slow call stalls every task
    /// sharing the thread.
    #[must_use]
    pub fn blocking() -> Self {
        Self(Native::blocking())
    }

    /// Opens the directory at the native `path` as a root [`Dir`], with all
    /// rights and `path` as its label.
    ///
    /// This is where native paths enter, so it requires the
    /// [`AmbientAuthority`] token.
    ///
    /// # Errors
    ///
    /// Any error opening `path` as a directory;
    /// [`Unsupported`](filer_api::ErrorKind::Unsupported) where no native
    /// driver exists yet.
    pub fn open_ambient(
        &self,
        path: impl AsRef<Path>,
        authority: AmbientAuthority,
    ) -> Result<Dir<Self>, Error> {
        let path = path.as_ref();
        let handle = self.0.open_ambient_dir(path, authority)?;
        let dir = Dir::new(self.clone(), handle)?;
        Ok(dir.with_label(&path.to_string_lossy()))
    }
}

impl Driver for System {
    fn id(&self) -> DriverId {
        self.0.id()
    }

    fn submit(&self, op: Op) -> impl Future<Output = Completion> + Send + use<> {
        self.0.submit(op)
    }
}

/// Opens the directory at the native `path` as a root [`Dir`] on a new
/// [blocking](System::blocking) [`System`] driver.
///
/// # Errors
///
/// As for [`System::open_ambient`].
///
/// # Examples
///
/// ```
/// use filer_system::{MetaMask, ambient_authority};
///
/// let tmp = filer_system::open_ambient(std::env::temp_dir(), ambient_authority())?;
/// # block_on(async {
/// let metadata = tmp.metadata("", MetaMask::KIND).await?;
/// assert!(metadata.kind.unwrap().is_dir());
/// # Ok::<(), filer_system::Error>(())
/// # })?;
/// # fn block_on<F: core::future::Future>(f: F) -> F::Output {
/// #     let mut f = core::pin::pin!(f);
/// #     let mut cx = core::task::Context::from_waker(core::task::Waker::noop());
/// #     loop {
/// #         if let core::task::Poll::Ready(v) = f.as_mut().poll(&mut cx) {
/// #             return v;
/// #         }
/// #     }
/// # }
/// # Ok::<(), filer_system::Error>(())
/// ```
pub fn open_ambient(
    path: impl AsRef<Path>,
    authority: AmbientAuthority,
) -> Result<Dir<System>, Error> {
    System::blocking().open_ambient(path, authority)
}

#[cfg(test)]
mod tests {
    use super::*;
    use filer_api::{ErrorKind, ambient_authority};

    #[cfg(unix)]
    fn block_on<F: Future>(future: F) -> F::Output {
        let mut future = core::pin::pin!(future);
        let mut cx = core::task::Context::from_waker(core::task::Waker::noop());
        loop {
            if let core::task::Poll::Ready(output) = future.as_mut().poll(&mut cx) {
                return output;
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn reads_files_through_the_native_driver() {
        use std::{fs, process};
        let path = std::env::temp_dir().join(std::format!("filer-system-{}", process::id()));
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join("a.txt"), "native").unwrap();
        let root = open_ambient(&path, ambient_authority()).unwrap();
        let file = block_on(root.open_file("a.txt")).unwrap();
        assert_eq!(block_on(file.read_to_end()).unwrap(), b"native");
        fs::remove_dir_all(&path).unwrap();
    }

    #[cfg(not(unix))]
    #[test]
    fn reports_unsupported_platforms() {
        let error = open_ambient(".", ambient_authority()).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Unsupported);
    }

    #[test]
    fn re_exports_the_api_and_its_macros() {
        let path: &crate::path::RelPath = crate::rel!("config/app.toml");
        let name: &crate::path::Name = crate::name!("app.toml");
        assert_eq!(path.split_last().map(|(_, last)| last), Some(name));
        let _: crate::Rights = crate::Rights::READ;
    }

    #[test]
    fn reports_missing_directories() {
        let error = open_ambient("/nonexistent/filer-system", ambient_authority()).unwrap_err();
        assert!(matches!(
            error.kind(),
            ErrorKind::NotFound | ErrorKind::Unsupported
        ));
        assert_eq!(error.label(), Some("/nonexistent/filer-system"));
    }
}

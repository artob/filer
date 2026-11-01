// This is free and unencumbered software released into the public domain.

use crate::{
    BufResult, Completion, Driver, DynDriver, EntryName, Error, ErrorKind, File, Handle,
    HandleKind, IoBufMut, LinkTarget, MetaMask, Metadata, Op, OpKind, OpenHow, Output, ReadDir,
    ReadOnly, Resolve, Rights, Target, XattrEntry,
    path::{RelPath, RelPathBuf},
    xattr,
};
use alloc::{borrow::ToOwned, sync::Arc, vec::Vec};

/// An open directory: the unit of authority.
///
/// Every path is resolved beneath a `Dir`, confined to its subtree under
/// its [`Resolve`] policy, and checked against its [`Rights`] before the
/// operation reaches the driver. There is no current directory and no
/// absolute path: code can only reach what it was handed a `Dir` for.
///
/// Paths may be [`RelPath`]s (such as [`rel!`](crate::rel) literals,
/// checked at compile time) or plain strings, checked when the method
/// runs; an invalid string fails with
/// [`InvalidName`](ErrorKind::InvalidName). Errors carry the path, relative
/// to this directory, and the directory's [label](Self::with_label).
///
/// Methods borrow the `Dir`; cloning it is a reference-count increment, so
/// to move work into a spawned task, move a clone. Dropping a returned
/// future does not cancel an operation the driver has started.
///
/// # Examples
///
/// ```
/// # use filer_api::{Dir, Driver};
/// # async fn example<D: Driver>(root: Dir<D>) -> filer_api::Result<()> {
/// let config = root.open_dir("config").await?;
/// let app = config.open_file("app.toml").await?;
/// let contents = app.read_to_end().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct Dir<D: Driver = DynDriver> {
    driver: D,
    handle: Handle,
    rights: Rights,
    resolve: Resolve,
    label: Option<Arc<str>>,
}

impl<D: Driver> Dir<D> {
    /// Wraps a directory handle minted by `driver`, with all [`Rights`] and
    /// the default [`Resolve`] policy.
    ///
    /// Drivers usually offer a more convenient way to get a root `Dir`.
    ///
    /// # Errors
    ///
    /// [`CrossDriver`](ErrorKind::CrossDriver) if `driver` did not mint
    /// `handle`, or [`NotADirectory`](ErrorKind::NotADirectory) if it is not
    /// a directory handle.
    pub fn new(driver: D, handle: Handle) -> crate::Result<Self> {
        if handle.driver() != driver.id() {
            return Err(Error::new(ErrorKind::CrossDriver, OpKind::Open));
        }
        if handle.kind() != HandleKind::Dir {
            return Err(Error::new(ErrorKind::NotADirectory, OpKind::Open));
        }
        Ok(Self {
            driver,
            handle,
            rights: Rights::all(),
            resolve: Resolve::default(),
            label: None,
        })
    }

    /// Returns the driver.
    pub fn driver(&self) -> &D {
        &self.driver
    }

    /// Returns the driver's handle for this directory.
    pub fn handle(&self) -> &Handle {
        &self.handle
    }

    /// Returns this directory's rights.
    pub fn rights(&self) -> Rights {
        self.rights
    }

    /// Returns this directory's resolution policy.
    pub fn resolve(&self) -> Resolve {
        self.resolve
    }

    /// Returns this directory's label, if any.
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// Returns a clone with at most `rights`. Rights can only be removed.
    #[must_use]
    pub fn restrict(&self, rights: Rights) -> Self {
        let mut dir = self.clone();
        dir.rights &= rights;
        dir
    }

    /// Sets the resolution policy for paths beneath this directory, such
    /// as `with_resolve(Symlinks::Deny)`. Every policy confines resolution
    /// beneath the directory; they differ in how symlinks and mount points
    /// are treated.
    #[must_use]
    pub fn with_resolve(mut self, resolve: impl Into<Resolve>) -> Self {
        self.resolve = resolve.into();
        self
    }

    /// Sets a label for errors, typically the native path the directory was
    /// opened from. Directories opened beneath this one inherit it.
    #[must_use]
    pub fn with_label(mut self, label: &str) -> Self {
        self.label = Some(Arc::from(label));
        self
    }

    /// Erases the driver's type.
    pub fn into_dyn(self) -> Dir<DynDriver> {
        Dir {
            driver: DynDriver::new(self.driver),
            handle: self.handle,
            rights: self.rights,
            resolve: self.resolve,
            label: self.label,
        }
    }

    /// Opens the directory at `path`. The result inherits this directory's
    /// rights, policy, and label. The empty path reopens this directory.
    ///
    /// # Errors
    ///
    /// Any error resolving or opening `path`, such as
    /// [`NotFound`](ErrorKind::NotFound) or
    /// [`NotADirectory`](ErrorKind::NotADirectory).
    pub async fn open_dir<P>(&self, path: P) -> crate::Result<Self>
    where
        P: TryInto<RelPathBuf>,
        P::Error: core::error::Error + Send + Sync + 'static,
    {
        let path = self.path(path, OpKind::Open)?;
        let handle = self.open(&path, HandleKind::Dir).await?;
        Ok(Self {
            driver: self.driver.clone(),
            handle,
            rights: self.rights,
            resolve: self.resolve,
            label: self.label.clone(),
        })
    }

    /// Opens the regular file at `path` for reading, following symlinks
    /// per this directory's policy.
    ///
    /// # Errors
    ///
    /// [`RightsDenied`](ErrorKind::RightsDenied) without
    /// [`Rights::READ`]; [`NotRegularFile`](ErrorKind::NotRegularFile) for
    /// FIFOs, sockets, and devices; any error resolving or opening `path`.
    pub async fn open_file<P>(&self, path: P) -> crate::Result<File<ReadOnly, D>>
    where
        P: TryInto<RelPathBuf>,
        P::Error: core::error::Error + Send + Sync + 'static,
    {
        let path = self.path(path, OpKind::Open)?;
        self.check(Rights::READ, OpKind::Open, &path)?;
        let handle = self.open(&path, HandleKind::File).await?;
        Ok(File::new(
            self.driver.clone(),
            handle,
            self.rights.for_file(),
            self.label.clone(),
        ))
    }

    /// Reads the metadata of `path`, following a final symlink per this
    /// directory's policy. The empty path means this directory.
    ///
    /// # Errors
    ///
    /// [`RightsDenied`](ErrorKind::RightsDenied) without
    /// [`Rights::READ`]; any error resolving `path`.
    pub async fn metadata<P>(&self, path: P, mask: MetaMask) -> crate::Result<Metadata>
    where
        P: TryInto<RelPathBuf>,
        P::Error: core::error::Error + Send + Sync + 'static,
    {
        self.stat(path, mask, true).await
    }

    /// Reads the metadata of `path` without following a final symlink.
    ///
    /// # Errors
    ///
    /// As for [`metadata`](Self::metadata).
    pub async fn symlink_metadata<P>(&self, path: P, mask: MetaMask) -> crate::Result<Metadata>
    where
        P: TryInto<RelPathBuf>,
        P::Error: core::error::Error + Send + Sync + 'static,
    {
        self.stat(path, mask, false).await
    }

    /// Lists this directory.
    ///
    /// # Errors
    ///
    /// [`RightsDenied`](ErrorKind::RightsDenied) without [`Rights::READ`].
    /// Errors from the driver come from [`ReadDir::next`].
    pub fn entries(&self) -> crate::Result<ReadDir<D>> {
        self.check(Rights::READ, OpKind::ReadDir, RelPath::empty())?;
        Ok(ReadDir::new(
            self.driver.clone(),
            self.handle.clone(),
            self.label.clone(),
        ))
    }

    /// Reads the target of the symlink at `path`.
    ///
    /// # Errors
    ///
    /// [`RightsDenied`](ErrorKind::RightsDenied) without [`Rights::READ`];
    /// [`NotASymlink`](ErrorKind::NotASymlink) if it is not a symlink;
    /// [`InvalidName`](ErrorKind::InvalidName) for the empty path; any
    /// error resolving its parent.
    pub async fn read_link<P>(&self, path: P) -> crate::Result<LinkTarget>
    where
        P: TryInto<RelPathBuf>,
        P::Error: core::error::Error + Send + Sync + 'static,
    {
        let op = OpKind::ReadLink;
        let path = self.path(path, op)?;
        self.check(Rights::READ, op, &path)?;
        let Some((parent, name)) = path.split_last() else {
            return Err(self.context(Error::new(ErrorKind::InvalidName, op), &path));
        };
        let at = self.parent(parent, &path).await?;
        let op = Op::ReadLink {
            at,
            name: EntryName::from(name),
        };
        match self.submit(op, &path).await? {
            Output::LinkTarget(target) => Ok(target),
            output => Err(unexpected(output, OpKind::ReadLink)),
        }
    }

    /// Reads the whole value of the extended attribute `name` of this
    /// directory, such as `"user.comment"`, or `None` if it has no such
    /// attribute.
    ///
    /// # Errors
    ///
    /// [`InvalidName`](ErrorKind::InvalidName) if `name` is not an
    /// attribute name; [`RightsDenied`](ErrorKind::RightsDenied) without
    /// [`Rights::READ`]; [`Unsupported`](ErrorKind::Unsupported) if the
    /// driver has no extended attributes;
    /// [`LimitExceeded`](ErrorKind::LimitExceeded) if the value is too
    /// large to allocate; any error from the driver.
    pub async fn xattr<N>(&self, name: N) -> crate::Result<Option<Vec<u8>>>
    where
        N: TryInto<XattrEntry>,
        N::Error: core::error::Error + Send + Sync + 'static,
    {
        let op = OpKind::GetXattr;
        let name = xattr::name(name, op).map_err(|error| self.context(error, RelPath::empty()))?;
        self.check(Rights::READ, op, RelPath::empty())?;
        xattr::read(&self.driver, &self.handle, &name)
            .await
            .map_err(|error| self.context(error, RelPath::empty()))
    }

    /// Reads as much of the value of the extended attribute `name` of this
    /// directory as fits into the spare capacity of `buf`, returning the
    /// value's whole length, or `None` if it has no such attribute. A
    /// length above the spare capacity means the value was cut short.
    ///
    /// # Errors
    ///
    /// As for [`xattr`](Self::xattr).
    ///
    /// # Panics
    ///
    /// Panics if the driver breaks its contract by not returning the
    /// buffer.
    pub async fn xattr_into<N, B>(&self, name: N, buf: B) -> BufResult<Option<u64>, B>
    where
        N: TryInto<XattrEntry>,
        N::Error: core::error::Error + Send + Sync + 'static,
        B: IoBufMut,
    {
        let op = OpKind::GetXattr;
        let name = match xattr::name(name, op) {
            Ok(name) => name,
            Err(error) => return BufResult(Err(self.context(error, RelPath::empty())), buf),
        };
        if let Err(error) = self.check(Rights::READ, op, RelPath::empty()) {
            return BufResult(Err(error), buf);
        }
        let BufResult(result, buf) = xattr::get(&self.driver, self.handle.clone(), name, buf).await;
        BufResult(
            result.map_err(|error| self.context(error, RelPath::empty())),
            buf,
        )
    }

    /// Lists the extended attributes of this directory, in no particular
    /// order.
    ///
    /// # Errors
    ///
    /// [`RightsDenied`](ErrorKind::RightsDenied) without
    /// [`Rights::READ`]; [`Unsupported`](ErrorKind::Unsupported) if the
    /// driver has no extended attributes; any error from the driver.
    pub async fn xattr_names(&self) -> crate::Result<Vec<XattrEntry>> {
        let op = OpKind::ListXattrs;
        self.check(Rights::READ, op, RelPath::empty())?;
        xattr::list(&self.driver, self.handle.clone())
            .await
            .map_err(|error| self.context(error, RelPath::empty()))
    }

    async fn stat<P>(&self, path: P, mask: MetaMask, follow: bool) -> crate::Result<Metadata>
    where
        P: TryInto<RelPathBuf>,
        P::Error: core::error::Error + Send + Sync + 'static,
    {
        let op = OpKind::Stat;
        let path = self.path(path, op)?;
        self.check(Rights::READ, op, &path)?;
        let target = match path.split_last() {
            None => Target::Handle(self.handle.clone()),
            Some((parent, name)) => Target::Entry {
                at: self.parent(parent, &path).await?,
                name: EntryName::from(name),
                follow,
                resolve: self.resolve,
            },
        };
        match self.submit(Op::Stat { target, mask }, &path).await? {
            Output::Metadata(metadata) => Ok(metadata),
            output => Err(unexpected(output, op)),
        }
    }

    /// Converts a path argument, mapping a conversion error to
    /// [`InvalidName`](ErrorKind::InvalidName).
    fn path<P>(&self, path: P, op: OpKind) -> crate::Result<RelPathBuf>
    where
        P: TryInto<RelPathBuf>,
        P::Error: core::error::Error + Send + Sync + 'static,
    {
        path.try_into().map_err(|error| {
            let error = Error::new(ErrorKind::InvalidName, op).with_source(error);
            match &self.label {
                Some(label) => error.with_label(label),
                None => error,
            }
        })
    }

    /// Fails with [`RightsDenied`](ErrorKind::RightsDenied) unless this
    /// directory has `rights`.
    fn check(&self, rights: Rights, op: OpKind, path: &RelPath) -> crate::Result<()> {
        if self.rights.contains(rights) {
            return Ok(());
        }
        Err(self.context(Error::new(ErrorKind::RightsDenied, op), path))
    }

    /// Adds this directory's label and `path` to an error.
    fn context(&self, error: Error, path: &RelPath) -> Error {
        let error = error.with_path(path);
        match &self.label {
            Some(label) => error.with_label(label),
            None => error,
        }
    }

    /// Submits `op`, adding context to its error.
    async fn submit(&self, op: Op, path: &RelPath) -> crate::Result<Output> {
        let Completion { result, .. } = self.driver.submit(op).await;
        result.map_err(|error| self.context(error, path))
    }

    async fn open(&self, path: &RelPath, kind: HandleKind) -> crate::Result<Handle> {
        let mut how = match kind {
            HandleKind::Dir => OpenHow::dir(),
            _ => OpenHow::file(),
        };
        how.resolve = self.resolve;
        let op = Op::Open {
            at: self.handle.clone(),
            path: path.to_owned(),
            how,
        };
        match self.submit(op, path).await? {
            Output::Opened(handle) => Ok(handle),
            output => Err(unexpected(output, OpKind::Open)),
        }
    }

    /// Returns a handle to the directory `parent` (this one if empty), for
    /// an operation on the entry at `path` within it.
    async fn parent(&self, parent: &RelPath, path: &RelPath) -> crate::Result<Handle> {
        if parent.is_empty() {
            return Ok(self.handle.clone());
        }
        self.open(parent, HandleKind::Dir)
            .await
            .map_err(|error| error.with_path(path))
    }
}

impl<D: Driver> core::fmt::Debug for Dir<D> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Dir")
            .field("handle", &self.handle)
            .field("rights", &self.rights)
            .field("resolve", &self.resolve)
            .field("label", &self.label)
            .finish_non_exhaustive()
    }
}

/// Reports an [`Output`] variant that does not match the operation, which
/// is a driver bug.
pub(crate) fn unexpected(output: Output, op: OpKind) -> Error {
    if cfg!(debug_assertions) {
        panic!("driver returned {output:?} for {op}");
    }
    Error::new(ErrorKind::Other, op)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BufResult, DriverId, FileKind,
        driver::tests::{Toy, block_on},
        rel,
    };
    use alloc::{vec, vec::Vec};
    use core::{
        future::Future,
        sync::atomic::{AtomicUsize, Ordering},
    };

    /// Counts the operations reaching the toy driver.
    #[derive(Clone)]
    struct Counting {
        inner: Toy,
        count: Arc<AtomicUsize>,
    }

    impl Driver for Counting {
        fn id(&self) -> DriverId {
            self.inner.id()
        }

        fn submit(&self, op: Op) -> impl Future<Output = Completion> + Send + use<> {
            self.count.fetch_add(1, Ordering::Relaxed);
            self.inner.submit(op)
        }
    }

    fn root(files: &[(&str, &[u8])]) -> (Dir<Counting>, Arc<AtomicUsize>) {
        let (toy, handle) = Toy::new(files);
        let count = Arc::new(AtomicUsize::new(0));
        let driver = Counting {
            inner: toy,
            count: count.clone(),
        };
        (Dir::new(driver, handle).unwrap(), count)
    }

    #[test]
    fn reads_whole_files_of_any_size() {
        let big: Vec<u8> = (0..20_000).map(|i| i as u8).collect();
        let (dir, _) = root(&[("big", &big), ("empty", b"")]);
        let file = block_on(dir.open_file("big")).unwrap();
        assert_eq!(block_on(file.read_to_end()).unwrap(), big);
        let empty = block_on(dir.open_file(rel!("empty"))).unwrap();
        assert_eq!(block_on(empty.read_to_end()).unwrap(), b"");
    }

    #[test]
    fn reads_exactly_or_fails_at_the_end() {
        let (dir, _) = root(&[("a", b"hello")]);
        let file = block_on(dir.open_file("a")).unwrap();
        let BufResult(result, buf) = block_on(file.read_exact_at(Vec::with_capacity(3), 1));
        result.unwrap();
        assert_eq!(buf, b"ell");
        let BufResult(result, buf) = block_on(file.read_exact_at(Vec::with_capacity(9), 2));
        assert_eq!(result.unwrap_err().kind(), ErrorKind::UnexpectedEof);
        assert_eq!(buf, b"llo");
    }

    #[test]
    fn checks_rights_before_reaching_the_driver() {
        let (dir, count) = root(&[("a", b"x")]);
        let blind = dir.restrict(Rights::all() - Rights::READ);
        let error = block_on(blind.open_file("a")).unwrap_err();
        assert_eq!(
            (error.kind(), error.path()),
            (ErrorKind::RightsDenied, Some(rel!("a")))
        );
        let error = block_on(blind.metadata("", MetaMask::all())).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::RightsDenied);
        assert_eq!(count.load(Ordering::Relaxed), 0);
        // Restriction never adds rights back.
        assert_eq!(blind.restrict(Rights::all()).rights(), blind.rights());
    }

    #[test]
    fn rejects_invalid_strings_before_reaching_the_driver() {
        let (dir, count) = root(&[]);
        let dir = dir.with_label("<root>");
        let error = block_on(dir.open_file("../etc/passwd")).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::InvalidName);
        assert_eq!(error.label(), Some("<root>"));
        assert!(core::error::Error::source(&error).is_some());
        assert_eq!(count.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn labels_driver_errors_with_the_path() {
        let (dir, _) = root(&[]);
        let dir = dir.with_label("<root>");
        let error = block_on(dir.open_file("nope")).unwrap_err();
        assert_eq!(
            alloc::string::ToString::to_string(&error),
            "open \"nope\" in <root>: not found"
        );
    }

    #[test]
    fn stats_directories_and_files() {
        let (dir, _) = root(&[("a", b"hello")]);
        let metadata = block_on(dir.metadata("", MetaMask::KIND)).unwrap();
        assert_eq!(metadata.kind, Some(FileKind::Dir));
        let file = block_on(dir.open_file("a")).unwrap();
        let metadata = block_on(file.metadata(MetaMask::LEN)).unwrap();
        assert_eq!(metadata.len, Some(5));
        assert_eq!(file.rights(), Rights::all().for_file());
    }

    #[test]
    fn ends_listings_after_an_error() {
        let (dir, count) = root(&[]);
        // The toy driver does not list directories.
        let mut entries = dir.entries().unwrap();
        let error = block_on(entries.next()).unwrap().unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Unsupported);
        assert!(block_on(entries.next()).is_none());
        assert_eq!(count.load(Ordering::Relaxed), 1);
        let blind = dir.restrict(Rights::empty());
        assert_eq!(blind.entries().unwrap_err().kind(), ErrorKind::RightsDenied);
    }

    #[test]
    fn wraps_only_matching_directory_handles() {
        let (toy, root) = Toy::new(&[("a", b"")]);
        let (other, _) = Toy::new(&[]);
        let error = Dir::new(other, root.clone()).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::CrossDriver);
        let dir = Dir::new(toy.clone(), root).unwrap();
        let file = block_on(dir.open_file("a")).unwrap();
        let error = Dir::new(toy, file.handle().clone()).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::NotADirectory);
        let _ = vec![dir.into_dyn()];
    }
}

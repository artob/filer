// This is free and unencumbered software released into the public domain.

//! The driver shell: a [`FileSystem`] a format implements, and the one
//! [`Driver`] over it, [`Mounted`], that every file system driver shares.
//!
//! A format answers questions about its nodes (what one is, what a
//! directory holds, what a file reads as); the shell does the rest of the
//! driver contract the same way for all of them: it dispatches each
//! [`Op`], keeps what an open handle needs in its payload, resolves paths
//! through the userspace [`Resolver`], refuses what a handle's kind does
//! not allow, turns names into bytes and buffers into answers, and names
//! entries in errors. A driver's public type wraps a `Mounted`.
//!
//! The shell serves reading for now. Writing (FAT's, `DESIGN.md` §17.13)
//! will add operations to [`FileSystem`], each [`Unsupported`] by default,
//! and their dispatch here.
//!
//! [`Unsupported`]: ErrorKind::Unsupported

use alloc::{sync::Arc, vec::Vec};
use core::future::Future;
use filer_api::{
    Completion, DirCookie, Driver, DriverId, DriverKey, Entry, EntryName, Error, ErrorKind,
    FileKind, Handle, HandleKind, IoBufMut, LinkTarget, MetaMask, Metadata, Op, OpKind, OpenHow,
    Output, Resolve, Resolver, Target, XattrEntry, XattrNs, XattrOp, path::RelPath,
};

/// What a file system format answers, for [`Mounted`] to serve it.
///
/// Each method gets the [`OpKind`] of the operation it serves, for its
/// errors. Nodes are what lookups find, such as an inode or a directory
/// entry; a handle keeps its node and what opening it made.
pub trait FileSystem: Send + Sync + 'static {
    /// A file, directory, symlink, or special file, as lookups find it.
    type Node: Clone + Send + Sync + 'static;

    /// What an open directory keeps to list it.
    type Dir: Send + Sync + 'static;

    /// What an open file keeps to read it.
    type File: Send + Sync + 'static;

    /// Returns the key of the driver, which makes its handles and names.
    fn key(&self) -> &DriverKey;

    /// Returns the root directory.
    ///
    /// # Errors
    ///
    /// The format's error for a missing or invalid root; any error reading
    /// the volume.
    fn root(&self, op: OpKind) -> impl Future<Output = Result<Self::Node, Error>> + Send;

    /// Returns what `node` is.
    fn kind(&self, node: &Self::Node) -> FileKind;

    /// Finds the entry `name` of directory `dir`, without following it if
    /// it is a symlink.
    ///
    /// # Errors
    ///
    /// [`NotFound`](ErrorKind::NotFound) if there is none; the format's
    /// errors; any error reading the volume.
    fn lookup(
        &self,
        dir: &Self::Node,
        name: &[u8],
        op: OpKind,
    ) -> impl Future<Output = Result<Self::Node, Error>> + Send;

    /// Returns the target of `link`, which is a symlink.
    ///
    /// # Errors
    ///
    /// The format's error for an invalid target; any error reading the
    /// volume.
    fn read_link(
        &self,
        link: &Self::Node,
        op: OpKind,
    ) -> impl Future<Output = Result<LinkTarget, Error>> + Send;

    /// Opens `dir`, which is a directory.
    ///
    /// # Errors
    ///
    /// The format's errors; any error reading the volume.
    fn open_dir(
        &self,
        dir: &Self::Node,
        op: OpKind,
    ) -> impl Future<Output = Result<Self::Dir, Error>> + Send;

    /// Opens `file`, which is a regular file.
    ///
    /// # Errors
    ///
    /// [`Unsupported`](ErrorKind::Unsupported) for a file whose contents
    /// the driver cannot read; the format's errors; any error reading the
    /// volume.
    fn open_file(
        &self,
        file: &Self::Node,
        op: OpKind,
    ) -> impl Future<Output = Result<Self::File, Error>> + Send;

    /// Reads up to `max` bytes of `file`, opened from `node`, at `off`:
    /// none at or past its end.
    ///
    /// # Errors
    ///
    /// The format's errors; any error reading the volume.
    fn read(
        &self,
        node: &Self::Node,
        file: &Self::File,
        off: u64,
        max: usize,
        op: OpKind,
    ) -> impl Future<Output = Result<Vec<u8>, Error>> + Send;

    /// Returns the metadata of `node`, which `opened` holds open if the
    /// stat is of a handle, as far as `mask` asks for what is costly.
    ///
    /// # Errors
    ///
    /// The format's errors; any error reading the volume.
    fn metadata(
        &self,
        node: &Self::Node,
        opened: Option<Opened<'_, Self>>,
        mask: MetaMask,
        op: OpKind,
    ) -> impl Future<Output = Result<Metadata, Error>> + Send;

    /// Lists up to `max` entries of `dir`, opened from `node`, from
    /// `cookie` on (from the start if `None`), without `.` and `..`, and
    /// returns them with the cookie of the next, or `None` at the end.
    ///
    /// # Errors
    ///
    /// The format's errors, as for a name that is no [`EntryName`]; any
    /// error reading the volume.
    fn read_dir(
        &self,
        node: &Self::Node,
        dir: &Self::Dir,
        cookie: Option<DirCookie>,
        max: usize,
        op: OpKind,
    ) -> impl Future<Output = Result<(Vec<Entry>, Option<DirCookie>), Error>> + Send;

    /// Lists the extended attributes of `node`.
    ///
    /// # Errors
    ///
    /// [`Unsupported`](ErrorKind::Unsupported) unless the format has
    /// extended attributes; the format's errors; any error reading the
    /// volume.
    fn xattr_names(
        &self,
        node: &Self::Node,
        op: OpKind,
    ) -> impl Future<Output = Result<Vec<XattrEntry>, Error>> + Send {
        let _ = node;
        core::future::ready(Err(Error::new(ErrorKind::Unsupported, op)))
    }

    /// Returns up to `max` bytes of the extended attribute `name` of `ns`
    /// of `node`, and its whole length, or `None` if it has none of that
    /// name. The name is as [`xattr_names`](Self::xattr_names) lists it,
    /// in bytes.
    ///
    /// # Errors
    ///
    /// [`Unsupported`](ErrorKind::Unsupported) unless the format has
    /// extended attributes; the format's errors; any error reading the
    /// volume.
    #[allow(clippy::type_complexity, reason = "a value and its length")]
    fn xattr(
        &self,
        node: &Self::Node,
        ns: XattrNs,
        name: &[u8],
        max: usize,
        op: OpKind,
    ) -> impl Future<Output = Result<Option<(Vec<u8>, u64)>, Error>> + Send {
        let _ = (node, ns, name, max);
        core::future::ready(Err(Error::new(ErrorKind::Unsupported, op)))
    }
}

/// What a handle holds open, as [`FileSystem::metadata`] gets it.
pub enum Opened<'a, F: FileSystem + ?Sized> {
    /// An open directory.
    Dir(&'a F::Dir),
    /// An open file.
    File(&'a F::File),
}

/// The private state of a handle.
enum Payload<F: FileSystem> {
    Dir { node: F::Node, dir: F::Dir },
    File { node: F::Node, file: F::File },
}

impl<F: FileSystem> Payload<F> {
    const fn node(&self) -> &F::Node {
        match self {
            Self::Dir { node, .. } | Self::File { node, .. } => node,
        }
    }

    const fn opened(&self) -> Opened<'_, F> {
        match self {
            Self::Dir { dir, .. } => Opened::Dir(dir),
            Self::File { file, .. } => Opened::File(file),
        }
    }
}

/// A [`FileSystem`] served as a read-only [`Driver`].
///
/// A driver's public type wraps one, opening the root with
/// [`root`](Self::root) and delegating [`Driver`] to it.
pub struct Mounted<F>(Arc<F>);

impl<F: FileSystem> Mounted<F> {
    /// Serves `fs`.
    #[must_use]
    pub fn new(fs: F) -> Self {
        Self(Arc::new(fs))
    }

    /// Returns the file system served.
    #[must_use]
    pub fn fs(&self) -> &F {
        &self.0
    }

    /// Opens the root directory, returning its handle.
    ///
    /// # Errors
    ///
    /// Any error of [`FileSystem::root`] or [`FileSystem::open_dir`].
    pub async fn root(&self) -> Result<Handle, Error> {
        let op = OpKind::Open;
        let node = self.0.root(op).await?;
        let dir = self.0.open_dir(&node, op).await?;
        Ok(self
            .0
            .key()
            .handle(HandleKind::Dir, Payload::<F>::Dir { node, dir }))
    }

    async fn perform(&self, op: Op) -> Completion {
        let kind = op.kind();
        match op {
            Op::Open { at, path, how } => Completion::new(
                self.open_path(&at, &path, how)
                    .await
                    .map_err(|error| error.with_path(&*path)),
            ),
            Op::Close { handle } => {
                Completion::new(self.payload(&handle, kind).map(|_| Output::Done))
            },
            Op::Read { file, mut buf, off } => {
                let result = self.read(&file, &mut buf, off, kind).await;
                Completion::new(result.map(Output::Read)).with_buf(buf)
            },
            Op::Stat { target, mask } => {
                let result = match &target {
                    Target::Handle(handle) => match self.payload(handle, kind) {
                        Ok(payload) => {
                            let opened = Some(payload.opened());
                            self.0.metadata(payload.node(), opened, mask, kind).await
                        },
                        Err(error) => Err(error),
                    },
                    Target::Entry {
                        at,
                        name,
                        follow,
                        resolve,
                    } => match self.entry(at, name, *follow, *resolve, kind).await {
                        Ok(node) => self.0.metadata(&node, None, mask, kind).await,
                        Err(error) => Err(error),
                    }
                    .map_err(|error| error.with_entry_name(name)),
                };
                Completion::new(result.map(Output::Metadata))
            },
            Op::ReadDir { dir, cookie, max } => {
                let result = match self.dir(&dir, kind) {
                    Ok((node, dir)) => self.0.read_dir(node, dir, cookie, max, kind).await,
                    Err(error) => Err(error),
                };
                Completion::new(result.map(|(entries, next)| Output::Entries { entries, next }))
            },
            Op::ReadLink { at, name } => {
                let result = match self
                    .entry(&at, &name, false, Resolve::default(), kind)
                    .await
                {
                    Ok(node) => Walker(&*self.0).link(&node, kind).await,
                    Err(error) => Err(error),
                };
                Completion::new(
                    result
                        .map(Output::LinkTarget)
                        .map_err(|error| error.with_entry_name(&name)),
                )
            },
            Op::Xattr(XattrOp::Get {
                handle,
                name,
                mut buf,
            }) => {
                let result = self.xattr(&handle, &name, &mut buf, kind).await;
                Completion::new(result.map(Output::Xattr)).with_buf(buf)
            },
            Op::Xattr(XattrOp::List { handle }) => {
                let result = match self.payload(&handle, kind) {
                    Ok(payload) => self.0.xattr_names(payload.node(), kind).await,
                    Err(error) => Err(error),
                };
                Completion::new(result.map(Output::XattrNames))
            },
            op => op.unsupported(),
        }
    }

    fn payload<'h>(&self, handle: &'h Handle, op: OpKind) -> Result<&'h Payload<F>, Error> {
        self.0
            .key()
            .payload::<Payload<F>>(handle)
            .ok_or_else(|| Error::new(ErrorKind::CrossDriver, op))
    }

    fn dir<'h>(&self, handle: &'h Handle, op: OpKind) -> Result<(&'h F::Node, &'h F::Dir), Error> {
        match self.payload(handle, op)? {
            Payload::Dir { node, dir } => Ok((node, dir)),
            Payload::File { .. } => Err(Error::new(ErrorKind::NotADirectory, op)),
        }
    }

    async fn open_path(&self, at: &Handle, path: &RelPath, how: OpenHow) -> Result<Output, Error> {
        let op = OpKind::Open;
        let (dir, _) = self.dir(at, op)?;
        let node = Walker(&*self.0)
            .resolve(dir, path.as_str().as_bytes(), how.resolve, true, op)
            .await?;
        let payload = match (self.0.kind(&node), how.kind) {
            (FileKind::Dir, HandleKind::Dir) => {
                let dir = self.0.open_dir(&node, op).await?;
                Payload::<F>::Dir { node, dir }
            },
            (FileKind::File, HandleKind::File) => {
                let file = self.0.open_file(&node, op).await?;
                Payload::File { node, file }
            },
            (FileKind::Dir, _) => return Err(Error::new(ErrorKind::IsADirectory, op)),
            (_, HandleKind::Dir) => return Err(Error::new(ErrorKind::NotADirectory, op)),
            // FIFOs, sockets, and devices.
            (_, _) => return Err(Error::new(ErrorKind::NotRegularFile, op)),
        };
        Ok(Output::Opened(self.0.key().handle(how.kind, payload)))
    }

    async fn read(
        &self,
        file: &Handle,
        buf: &mut impl IoBufMut,
        off: u64,
        op: OpKind,
    ) -> Result<usize, Error> {
        let (node, file) = match self.payload(file, op)? {
            Payload::File { node, file } => (node, file),
            Payload::Dir { .. } => return Err(Error::new(ErrorKind::IsADirectory, op)),
        };
        let bytes = self.0.read(node, file, off, buf.spare(), op).await?;
        Ok(buf.fill_from(&bytes))
    }

    /// Reads as much of the extended attribute `name` of `handle` as fits
    /// into `buf`, returning its whole length.
    async fn xattr(
        &self,
        handle: &Handle,
        name: &XattrEntry,
        buf: &mut impl IoBufMut,
        op: OpKind,
    ) -> Result<Option<u64>, Error> {
        let node = self.payload(handle, op)?.node();
        let (ns, wanted) = match name {
            XattrEntry::Foreign { name, .. } if name.origin() != self.0.key().id() => {
                return Err(Error::new(ErrorKind::ForeignNameMismatch, op));
            },
            XattrEntry::Utf8(name) => (name.ns(), name.name().as_bytes()),
            XattrEntry::Foreign { ns, name } => (*ns, name.as_bytes()),
        };
        let found = self.0.xattr(node, ns, wanted, buf.spare(), op).await?;
        Ok(found.map(|(value, len)| {
            buf.fill_from(&value);
            len
        }))
    }

    /// Resolves the entry `name` of the directory `at` holds.
    async fn entry(
        &self,
        at: &Handle,
        name: &EntryName,
        follow: bool,
        resolve: Resolve,
        op: OpKind,
    ) -> Result<F::Node, Error> {
        let (dir, _) = self.dir(at, op)?;
        let bytes = match name {
            EntryName::Utf8(name) => name.as_str().as_bytes(),
            EntryName::Foreign(name) if name.origin() == self.0.key().id() => name.as_bytes(),
            EntryName::Foreign(_) => return Err(Error::new(ErrorKind::ForeignNameMismatch, op)),
        };
        Walker(&*self.0)
            .resolve(dir, bytes, resolve, follow, op)
            .await
    }
}

impl<F: FileSystem> Driver for Mounted<F> {
    fn id(&self) -> DriverId {
        self.0.key().id()
    }

    fn submit(&self, op: Op) -> impl Future<Output = Completion> + Send + use<F> {
        let mounted = self.clone();
        async move { mounted.perform(op).await }
    }
}

impl<F> Clone for Mounted<F> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<F: FileSystem> core::fmt::Debug for Mounted<F> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Mounted")
            .field("id", &self.0.key().id())
            .finish_non_exhaustive()
    }
}

/// The [`Resolver`] primitives over a file system. A private type, so
/// that nodes cannot be resolved from without a handle.
struct Walker<'a, F>(&'a F);

impl<F: FileSystem> Walker<'_, F> {
    /// Returns the target of `node`, if it is a symlink.
    async fn link(&self, node: &F::Node, op: OpKind) -> Result<LinkTarget, Error> {
        if self.0.kind(node) != FileKind::Symlink {
            return Err(Error::new(ErrorKind::NotASymlink, op));
        }
        self.0.read_link(node, op).await
    }
}

impl<F: FileSystem> Resolver for Walker<'_, F> {
    type Node = F::Node;

    fn lookup(
        &self,
        dir: &F::Node,
        name: &[u8],
        op: OpKind,
    ) -> impl Future<Output = Result<F::Node, Error>> + Send {
        self.0.lookup(dir, name, op)
    }

    fn kind(&self, node: &F::Node) -> FileKind {
        self.0.kind(node)
    }

    async fn read_link(&self, dir: &F::Node, name: &[u8], op: OpKind) -> Result<LinkTarget, Error> {
        let node = self.0.lookup(dir, name, op).await?;
        self.link(&node, op).await
    }
}

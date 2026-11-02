// This is free and unencumbered software released into the public domain.

use crate::{
    Options, XfsError, attr,
    dir::Directory,
    file_map::FileMap,
    volume::{Node, Volume, Walker},
};
use alloc::{sync::Arc, vec::Vec};
use core::future::Future;
use filer_api::{
    Completion, Dir, DirCookie, Driver, DriverId, Entry, EntryName, Error, ErrorKind, FileId,
    FileKind, Handle, HandleKind, IoBufMut, Metadata, Op, OpKind, OpenHow, Output, ReadAt, Resolve,
    Resolver, Rights, Target, XattrEntry, XattrOp, path::RelPath,
};

/// A read-only driver for an XFS volume.
///
/// It reads a bare volume's data device, with no partition table, from
/// any [`ReadAt`] source: a disk image [`File`](filer_api::File) of
/// another driver, bytes in memory, or a block device. Opening reads and
/// validates the superblock and the root directory; inodes, directories,
/// and data are read as operations need them.
///
/// Names are bytes, matched exactly; those that are not UTF-8 are
/// [`ForeignName`](filer_api::ForeignName)s. Listings are in directory
/// order, without `.` and `..`. A file's id is its inode number, so hard
/// links share it. Times are UTC, at 1 ns. Metadata reports the flags
/// immutable and append-only. Extended attributes are read in the `user`,
/// `trusted`, `security`, and `system` namespaces; POSIX ACLs read as
/// Linux's `getxattr` returns them, as `system` attributes converted and
/// as `trusted` ones as they are. Symlinks are confined like any other;
/// FIFOs, sockets, and devices are what they say, and do not open. Writing
/// is out of scope: everything but reading is
/// [`Unsupported`](ErrorKind::Unsupported).
///
/// # Examples
///
/// ```no_run
/// use filer_xfs::Xfs;
///
/// # async fn example(image: Vec<u8>) -> filer_api::Result<()> {
/// let root = Xfs::open(image).await?;
/// let file = root.open_file("etc/hostname").await?;
/// let text = file.read_to_end().await?;
/// # Ok(())
/// # }
/// ```
pub struct Xfs<S>(Arc<Volume<S>>);

/// The private state of a handle.
enum Payload {
    Dir { node: Node, dir: Directory },
    File { node: Node, map: FileMap },
}

impl Payload {
    const fn node(&self) -> &Node {
        match self {
            Self::Dir { node, .. } | Self::File { node, .. } => node,
        }
    }
}

impl<S: ReadAt + 'static> Xfs<S> {
    /// Opens the volume in `source` and returns its root directory, with
    /// [`Rights::READ`]. A volume whose log was not left clean is refused;
    /// [`open_with`](Self::open_with) can open it anyway.
    ///
    /// # Errors
    ///
    /// As [`open_with`](Self::open_with) with the default [`Options`].
    pub async fn open(source: S) -> Result<Dir<Self>, Error> {
        Self::open_with(source, Options::new()).await
    }

    /// Opens the volume in `source` with `options` and returns its root
    /// directory, with [`Rights::READ`].
    ///
    /// # Errors
    ///
    /// [`Corrupt`](ErrorKind::Corrupt), with an [`XfsError`] as its source,
    /// if the superblock or the root directory is invalid, or the source is
    /// shorter than the volume; [`Unsupported`](ErrorKind::Unsupported) if
    /// the volume is of an unknown version or uses features this driver
    /// does not read, or, unless `options` allow it, if its log needs
    /// recovery or is on another device; any error reading the source.
    pub async fn open_with(source: S, options: Options) -> Result<Dir<Self>, Error> {
        let op = OpKind::Open;
        let volume = Volume::open(source, options).await?;
        let node = volume.root(op).await?;
        let dir = volume.directory(&node, op)?;
        let root = volume
            .key
            .handle(HandleKind::Dir, Payload::Dir { node, dir });
        Ok(Dir::new(Self(Arc::new(volume)), root)?.restrict(Rights::READ))
    }
}

impl<S: ReadAt> Xfs<S> {
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
                let result = self.read(&file, &mut buf, off).await;
                Completion::new(result.map(Output::Read)).with_buf(buf)
            },
            Op::Stat { target, .. } => {
                let result = match &target {
                    Target::Handle(handle) => self
                        .payload(handle, kind)
                        .map(|payload| self.metadata(payload.node())),
                    Target::Entry {
                        at,
                        name,
                        follow,
                        resolve,
                    } => self
                        .entry(at, name, *follow, *resolve, kind)
                        .await
                        .map(|node| self.metadata(&node))
                        .map_err(|error| with_name(error, name)),
                };
                Completion::new(result.map(Output::Metadata))
            },
            Op::ReadDir { dir, cookie, max } => {
                Completion::new(self.read_dir(&dir, cookie, max, kind).await)
            },
            Op::ReadLink { at, name } => {
                let result = match self
                    .entry(&at, &name, false, Resolve::default(), kind)
                    .await
                {
                    Ok(node) => self.0.link(&node, kind).await,
                    Err(error) => Err(error),
                };
                Completion::new(
                    result
                        .map(Output::LinkTarget)
                        .map_err(|error| with_name(error, &name)),
                )
            },
            Op::Xattr(XattrOp::Get {
                handle,
                name,
                mut buf,
            }) => {
                let result = self.xattr(&handle, &name, &mut buf).await;
                Completion::new(result.map(Output::Xattr)).with_buf(buf)
            },
            Op::Xattr(XattrOp::List { handle }) => {
                Completion::new(self.xattr_names(&handle).await.map(Output::XattrNames))
            },
            op => op.unsupported(),
        }
    }

    fn payload<'h>(&self, handle: &'h Handle, op: OpKind) -> Result<&'h Payload, Error> {
        self.0
            .key
            .payload::<Payload>(handle)
            .ok_or_else(|| Error::new(ErrorKind::CrossDriver, op))
    }

    fn dir<'h>(&self, handle: &'h Handle, op: OpKind) -> Result<(&'h Node, &'h Directory), Error> {
        match self.payload(handle, op)? {
            Payload::Dir { node, dir } => Ok((node, dir)),
            Payload::File { .. } => Err(Error::new(ErrorKind::NotADirectory, op)),
        }
    }

    async fn open_path(&self, at: &Handle, path: &RelPath, how: OpenHow) -> Result<Output, Error> {
        let op = OpKind::Open;
        let (dir, _) = self.dir(at, op)?;
        let node = Walker(&self.0)
            .resolve(dir, path.as_str().as_bytes(), how.resolve, true, op)
            .await?;
        let payload = match (node.kind(), how.kind) {
            (FileKind::Dir, HandleKind::Dir) => {
                let dir = self.0.directory(&node, op)?;
                Payload::Dir { node, dir }
            },
            (FileKind::File, HandleKind::File) => {
                let map = self.0.map(&node, op)?;
                Payload::File { node, map }
            },
            (FileKind::Dir, _) => return Err(Error::new(ErrorKind::IsADirectory, op)),
            (_, HandleKind::Dir) => return Err(Error::new(ErrorKind::NotADirectory, op)),
            // FIFOs, sockets, and devices.
            (_, _) => return Err(Error::new(ErrorKind::NotRegularFile, op)),
        };
        Ok(Output::Opened(self.0.key.handle(how.kind, payload)))
    }

    async fn read(&self, file: &Handle, buf: &mut impl IoBufMut, off: u64) -> Result<usize, Error> {
        let op = OpKind::Read;
        let (node, map) = match self.payload(file, op)? {
            Payload::File { node, map } => (node, map),
            Payload::Dir { .. } => return Err(Error::new(ErrorKind::IsADirectory, op)),
        };
        let volume = &self.0;
        let size = node.inode().size();
        let bytes = map
            .read(&volume.source, &volume.sb, size, off, buf.spare(), op)
            .await?;
        buf.fill(&mut |spare| {
            let n = spare.len().min(bytes.len());
            let (Some(to), Some(from)) = (spare.get_mut(..n), bytes.get(..n)) else {
                return Ok(0);
            };
            to.copy_from_slice(from);
            Ok(n)
        })
    }

    /// Reads as much of the extended attribute `name` of `handle` as fits
    /// into `buf`, returning its whole length.
    async fn xattr(
        &self,
        handle: &Handle,
        name: &XattrEntry,
        buf: &mut impl IoBufMut,
    ) -> Result<Option<u64>, Error> {
        let op = OpKind::GetXattr;
        let node = self.payload(handle, op)?.node();
        let (ns, wanted) = match name {
            XattrEntry::Foreign { name, .. } if name.origin() != self.0.key.id() => {
                return Err(Error::new(ErrorKind::ForeignNameMismatch, op));
            },
            XattrEntry::Utf8(name) => (name.ns(), name.name().as_bytes()),
            XattrEntry::Foreign { ns, name } => (*ns, name.as_bytes()),
        };
        let volume = &self.0;
        let xattrs = attr::read(&volume.source, &volume.sb, node.inode(), op).await?;
        let Some(found) = xattrs
            .into_iter()
            .find(|xattr| xattr.ns == ns && xattr.name == wanted)
        else {
            return Ok(None);
        };
        let value = found.value;
        buf.fill(&mut |spare| {
            let n = spare.len().min(value.len());
            let (Some(to), Some(from)) = (spare.get_mut(..n), value.get(..n)) else {
                return Ok(0);
            };
            to.copy_from_slice(from);
            Ok(n)
        })?;
        Ok(Some(u64::try_from(value.len()).unwrap_or(u64::MAX)))
    }

    /// Lists the extended attributes of `handle`.
    async fn xattr_names(&self, handle: &Handle) -> Result<Vec<XattrEntry>, Error> {
        let op = OpKind::ListXattrs;
        let node = self.payload(handle, op)?.node();
        let volume = &self.0;
        let bad = || {
            XfsError::BadXattr {
                inode: node.inode().number(),
            }
            .into_error(op)
        };
        attr::read(&volume.source, &volume.sb, node.inode(), op)
            .await?
            .iter()
            .map(|xattr| {
                volume
                    .key
                    .xattr_entry(xattr.ns, &xattr.name)
                    .map_err(|_| bad())
            })
            .collect()
    }

    async fn entry(
        &self,
        at: &Handle,
        name: &EntryName,
        follow: bool,
        resolve: Resolve,
        op: OpKind,
    ) -> Result<Node, Error> {
        let (dir, _) = self.dir(at, op)?;
        let bytes = match name {
            EntryName::Utf8(name) => name.as_str().as_bytes(),
            EntryName::Foreign(name) if name.origin() == self.0.key.id() => name.as_bytes(),
            EntryName::Foreign(_) => return Err(Error::new(ErrorKind::ForeignNameMismatch, op)),
        };
        Walker(&self.0)
            .resolve(dir, bytes, resolve, follow, op)
            .await
    }

    /// Returns the id of inode `number`: the volume's, and the number.
    fn id(&self, number: u64) -> FileId {
        FileId::new(self.0.sb.volume_id(), u128::from(number))
    }

    /// Returns the metadata of `node`.
    fn metadata(&self, node: &Node) -> Metadata {
        let inode = node.inode();
        let times = inode.times();
        let mut metadata = Metadata::default();
        metadata.kind = Some(inode.kind());
        metadata.id = Some(self.id(inode.number()));
        metadata.len = Some(inode.size());
        metadata.modified = Some(times.modified);
        metadata.changed = Some(times.changed);
        metadata.accessed = Some(times.accessed);
        metadata.created = times.created;
        metadata.attrs = Some(inode.attrs());
        metadata
    }

    async fn read_dir(
        &self,
        handle: &Handle,
        cookie: Option<DirCookie>,
        max: usize,
        op: OpKind,
    ) -> Result<Output, Error> {
        let (_, dir) = self.dir(handle, op)?;
        let volume = &self.0;
        // The cookie is the offset of the next entry, or below it.
        let mut position = cookie.map_or(0, DirCookie::get);
        let first = dir.block_of(&volume.sb, position);
        let mut entries = Vec::new();
        for index in first..dir.blocks(&volume.sb) {
            for found in dir.block(&volume.source, &volume.sb, index, op).await? {
                if found.offset < position {
                    continue;
                }
                if entries.len() >= max {
                    return Ok(Output::Entries {
                        entries,
                        next: Some(DirCookie::new(position)),
                    });
                }
                // Offsets ascend, so the next entry is past this one.
                position = found.offset.saturating_add(1);
                if found.is_dot() {
                    continue;
                }
                let name = volume.key.entry_name(&found.name).map_err(|_| {
                    XfsError::BadDirEntry {
                        dir: dir.inode(),
                        offset: found.offset,
                    }
                    .into_error(op)
                })?;
                let mut listed = Entry::new(name);
                listed.kind = found.kind;
                listed.id = Some(self.id(found.inode));
                entries.push(listed);
            }
        }
        Ok(Output::Entries {
            entries,
            next: None,
        })
    }
}

impl<S: ReadAt + 'static> Driver for Xfs<S> {
    fn id(&self) -> DriverId {
        self.0.key.id()
    }

    fn submit(&self, op: Op) -> impl Future<Output = Completion> + Send + use<S> {
        let xfs = self.clone();
        async move { xfs.perform(op).await }
    }
}

impl<S> Clone for Xfs<S> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<S> core::fmt::Debug for Xfs<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Xfs")
            .field("id", &self.0.key.id())
            .field("volume", &self.0.sb.volume_id())
            .finish_non_exhaustive()
    }
}

/// Adds an entry's name to an error, if it is UTF-8.
fn with_name(error: Error, name: &EntryName) -> Error {
    match name.as_name() {
        Some(name) => error.with_path(name),
        None => error,
    }
}

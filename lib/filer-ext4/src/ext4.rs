// This is free and unencumbered software released into the public domain.

use crate::{
    Ext4Error,
    block_map::BlockMap,
    dir::Directory,
    volume::{Node, Volume, Walker},
};
use alloc::{sync::Arc, vec::Vec};
use core::future::Future;
use filer_api::{
    Completion, Dir, DirCookie, Driver, DriverId, Entry, EntryName, Error, ErrorKind, FileId,
    FileKind, Handle, HandleKind, IoBufMut, Metadata, Op, OpKind, OpenHow, Output, ReadAt, Resolve,
    Resolver, Rights, Target, path::RelPath,
};

/// A read-only driver for an ext2, ext3, or ext4 volume.
///
/// It reads a bare volume, with no partition table, from any [`ReadAt`]
/// source: a disk image [`File`](filer_api::File) of another driver,
/// bytes in memory, or a block device. Opening reads and validates the
/// superblock and the root directory; inodes, directories, and data are
/// read as operations need them.
///
/// Names are bytes, matched exactly; those that are not UTF-8 are
/// [`ForeignName`](filer_api::ForeignName)s. Listings are in directory
/// order, without `.` and `..`. A file's id is its inode number, so hard
/// links share it. Times are UTC, at 1 ns where the inode has room for
/// the extra fields, else at 1 s, and include the creation time where it
/// is recorded. Symlinks are confined like any other; FIFOs, sockets, and
/// devices are what they say, and do not open. Writing is out of scope:
/// everything but reading is [`Unsupported`](ErrorKind::Unsupported).
///
/// # Examples
///
/// ```no_run
/// use filer_ext4::Ext4;
///
/// # async fn example(image: Vec<u8>) -> filer_api::Result<()> {
/// let root = Ext4::open(image).await?;
/// let file = root.open_file("etc/hostname").await?;
/// let text = file.read_to_end().await?;
/// # Ok(())
/// # }
/// ```
pub struct Ext4<S>(Arc<Volume<S>>);

/// The private state of a handle.
enum Payload {
    Dir { node: Node, dir: Directory },
    File { node: Node, map: BlockMap },
}

impl Payload {
    const fn node(&self) -> &Node {
        match self {
            Self::Dir { node, .. } | Self::File { node, .. } => node,
        }
    }
}

impl<S: ReadAt + 'static> Ext4<S> {
    /// Opens the volume in `source` and returns its root directory, with
    /// [`Rights::READ`].
    ///
    /// # Errors
    ///
    /// [`Corrupt`](ErrorKind::Corrupt), with an
    /// [`Ext4Error`](crate::Ext4Error) as its source, if the superblock or
    /// the root directory is invalid, or the source is shorter than the
    /// volume; [`Unsupported`](ErrorKind::Unsupported) if the volume is of
    /// an unknown revision, uses incompatible features this driver does
    /// not read, or its journal needs recovery; any error reading the
    /// source.
    pub async fn open(source: S) -> Result<Dir<Self>, Error> {
        let op = OpKind::Open;
        let volume = Volume::open(source).await?;
        let node = volume.root(op).await?;
        let dir = volume.directory(&node, op)?;
        let root = volume
            .key
            .handle(HandleKind::Dir, Payload::Dir { node, dir });
        Ok(Dir::new(Self(Arc::new(volume)), root)?.restrict(Rights::READ))
    }
}

impl<S: ReadAt> Ext4<S> {
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
    fn id(&self, number: u32) -> FileId {
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
        // The cookie is the offset of the next entry.
        let mut position = cookie.map_or(0, DirCookie::get);
        let first = position
            .checked_div(u64::from(volume.sb.block_size()))
            .unwrap_or(0);
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
                position = found.next;
                if found.is_dot() {
                    continue;
                }
                let name = volume.key.entry_name(&found.name).map_err(|_| {
                    Ext4Error::BadDirEntry {
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

impl<S: ReadAt + 'static> Driver for Ext4<S> {
    fn id(&self) -> DriverId {
        self.0.key.id()
    }

    fn submit(&self, op: Op) -> impl Future<Output = Completion> + Send + use<S> {
        let ext4 = self.clone();
        async move { ext4.perform(op).await }
    }
}

impl<S> Clone for Ext4<S> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<S> core::fmt::Debug for Ext4<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Ext4")
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

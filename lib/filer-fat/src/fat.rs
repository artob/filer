// This is free and unencumbered software released into the public domain.

use crate::{
    dir_entry::Body,
    fat_time::{ONE_DAY, TEN_MILLIS, TWO_SECONDS},
    geometry,
    volume::{DirStream, Extent, FileData, Found, Node, Volume, Walker},
};
use alloc::{sync::Arc, vec::Vec};
use core::future::Future;
use filer_api::{
    Completion, Dir, DirCookie, Driver, DriverId, DriverKey, Entry, EntryName, Error, ErrorKind,
    FileId, FileKind, Handle, HandleKind, IoBufMut, LinkTarget, Metadata, Op, OpKind, OpenHow,
    Output, ReadAt, Resolve, Resolver, Rights, Target, path::RelPath,
};

/// A read-only driver for a FAT12, FAT16, or FAT32 volume.
///
/// It reads a bare volume, with no partition table, from any [`ReadAt`]
/// source: a disk image [`File`](filer_api::File) of another driver, bytes
/// in memory, or a block device. Opening reads and validates the boot
/// sector; directories and cluster chains are read as operations need
/// them, and a file's chain is read and validated once, when it is opened.
///
/// Names are matched case-insensitively, as FAT preserves case but does
/// not distinguish it, against either an entry's long name or its short
/// (8.3) name. Listings show the long name if there is one. Times are
/// local with no time zone, and are reported as if UTC.
///
/// Writing is planned. Until then, everything but reading is
/// [`Unsupported`](ErrorKind::Unsupported).
///
/// # Examples
///
/// ```no_run
/// use filer_fat::Fat;
///
/// # async fn example(image: Vec<u8>) -> filer_api::Result<()> {
/// let root = Fat::open(image).await?;
/// let file = root.open_file("README.TXT").await?;
/// let text = file.read_to_end().await?;
/// # Ok(())
/// # }
/// ```
pub struct Fat<S>(Arc<Volume<S>>);

/// The private state of a handle.
enum Payload {
    Dir { node: Node, extent: Extent },
    File { node: Node, data: FileData },
}

impl Payload {
    const fn node(&self) -> &Node {
        match self {
            Self::Dir { node, .. } | Self::File { node, .. } => node,
        }
    }
}

impl<S: ReadAt + 'static> Fat<S> {
    /// Opens the volume in `source` and returns its root directory, with
    /// [`Rights::READ`].
    ///
    /// # Errors
    ///
    /// [`Corrupt`](ErrorKind::Corrupt), with a [`FatError`](crate::FatError)
    /// as its source, if the boot sector is invalid or the source is
    /// shorter than the volume; any error reading the source.
    pub async fn open(source: S) -> Result<Dir<Self>, Error> {
        let geometry = geometry::read(&source).await?;
        let volume = Volume {
            key: DriverKey::new(),
            geometry,
            source,
        };
        let op = OpKind::Open;
        let extent = volume.extent(&Node::Root, op).await?;
        let root = volume.key.handle(
            HandleKind::Dir,
            Payload::Dir {
                node: Node::Root,
                extent,
            },
        );
        Ok(Dir::new(Self(Arc::new(volume)), root)?.restrict(Rights::READ))
    }
}

impl<S: ReadAt> Fat<S> {
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
                        .map(|payload| self.metadata(payload.node(), Some(payload))),
                    Target::Entry {
                        at, name, resolve, ..
                    } => self
                        .entry(at, name, *resolve, kind)
                        .await
                        .map(|node| self.metadata(&node, None))
                        .map_err(|error| with_name(error, name)),
                };
                Completion::new(result.map(Output::Metadata))
            },
            Op::ReadDir { dir, cookie, max } => {
                Completion::new(self.read_dir(&dir, cookie, max, kind).await)
            },
            Op::ReadLink { at, name } => {
                let result = self
                    .entry(&at, &name, Resolve::default(), kind)
                    .await
                    .and_then(|_| Err::<LinkTarget, _>(Error::new(ErrorKind::NotASymlink, kind)));
                Completion::new(
                    result
                        .map(Output::LinkTarget)
                        .map_err(|error| with_name(error, &name)),
                )
            },
            _ => Completion::new(Err(Error::new(ErrorKind::Unsupported, kind))),
        }
    }

    fn payload<'h>(&self, handle: &'h Handle, op: OpKind) -> Result<&'h Payload, Error> {
        self.0
            .key
            .payload::<Payload>(handle)
            .ok_or_else(|| Error::new(ErrorKind::CrossDriver, op))
    }

    fn dir<'h>(&self, handle: &'h Handle, op: OpKind) -> Result<(&'h Node, &'h Extent), Error> {
        match self.payload(handle, op)? {
            Payload::Dir { node, extent } => Ok((node, extent)),
            Payload::File { .. } => Err(Error::new(ErrorKind::NotADirectory, op)),
        }
    }

    async fn open_path(&self, at: &Handle, path: &RelPath, how: OpenHow) -> Result<Output, Error> {
        let op = OpKind::Open;
        let (dir, _) = self.dir(at, op)?;
        let walker = Walker(&self.0);
        let node = walker
            .resolve(dir, path.as_str().as_bytes(), how.resolve, true, op)
            .await?;
        let payload = match (node.kind(), how.kind) {
            (FileKind::Dir, HandleKind::Dir) => {
                let extent = self.0.extent(&node, op).await?;
                Payload::Dir { node, extent }
            },
            (FileKind::File, HandleKind::File) => {
                let Node::Entry(found) = &node else {
                    return Err(Error::new(ErrorKind::IsADirectory, op));
                };
                let data = self.0.file_chain(found, op).await?;
                Payload::File { node, data }
            },
            (FileKind::Dir, _) => return Err(Error::new(ErrorKind::IsADirectory, op)),
            (_, _) => return Err(Error::new(ErrorKind::NotADirectory, op)),
        };
        Ok(Output::Opened(self.0.key.handle(how.kind, payload)))
    }

    async fn read(&self, file: &Handle, buf: &mut impl IoBufMut, off: u64) -> Result<usize, Error> {
        let op = OpKind::Read;
        let data = match self.payload(file, op)? {
            Payload::File { data, .. } => data,
            Payload::Dir { .. } => return Err(Error::new(ErrorKind::IsADirectory, op)),
        };
        let bytes = self.0.read_file(data, off, buf.spare(), op).await?;
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
        resolve: Resolve,
        op: OpKind,
    ) -> Result<Node, Error> {
        let (dir, _) = self.dir(at, op)?;
        let bytes = match name {
            EntryName::Utf8(name) => name.as_str().as_bytes(),
            EntryName::Foreign(name) if name.origin() == self.0.key.id() => name.as_bytes(),
            EntryName::Foreign(_) => return Err(Error::new(ErrorKind::ForeignNameMismatch, op)),
        };
        Walker(&self.0).resolve(dir, bytes, resolve, true, op).await
    }

    fn metadata(&self, node: &Node, payload: Option<&Payload>) -> Metadata {
        let mut metadata = Metadata::default();
        metadata.kind = Some(node.kind());
        metadata.id = Some(FileId::new(0, node.id()));
        if let Node::Entry(found) = node {
            let Found { entry, .. } = &**found;
            metadata.len = match (payload, entry.body()) {
                (Some(Payload::File { data, .. }), _) => Some(data.size()),
                (_, Body::File { size, .. }) => Some(u64::from(size.get())),
                (_, Body::EmptyFile) => Some(0),
                (_, Body::Dir(_)) => None,
            };
            metadata.modified = entry.modified().to_timestamp(0, TWO_SECONDS);
            metadata.created = entry.created().to_timestamp(0, TEN_MILLIS);
            metadata.accessed = entry.accessed().to_timestamp(0, ONE_DAY);
        }
        metadata
    }

    async fn read_dir(
        &self,
        dir: &Handle,
        cookie: Option<DirCookie>,
        max: usize,
        op: OpKind,
    ) -> Result<Output, Error> {
        let (_, extent) = self.dir(dir, op)?;
        let slot = cookie.map_or(Some(0), |cookie| u32::try_from(cookie.get()).ok());
        let Some(slot) = slot else {
            return Ok(Output::Entries {
                entries: Vec::new(),
                next: None,
            });
        };
        let mut stream = DirStream::new(&self.0, extent, slot);
        let mut entries = Vec::new();
        while entries.len() < max {
            let Some(dir_entry) = stream.next(op).await? else {
                return Ok(Output::Entries {
                    entries,
                    next: None,
                });
            };
            let name = self
                .0
                .key
                .entry_name(&dir_entry.name())
                .map_err(|error| Error::new(ErrorKind::Corrupt, op).with_source(error))?;
            let node = Node::Entry(Arc::new(Found {
                dir: extent.id(),
                entry: dir_entry,
            }));
            let mut entry = Entry::new(name);
            entry.kind = Some(node.kind());
            entry.id = Some(FileId::new(0, node.id()));
            entries.push(entry);
        }
        let next = Some(DirCookie::new(u64::from(stream.slot())));
        Ok(Output::Entries { entries, next })
    }
}

impl<S: ReadAt + 'static> Driver for Fat<S> {
    fn id(&self) -> DriverId {
        self.0.key.id()
    }

    fn submit(&self, op: Op) -> impl Future<Output = Completion> + Send + use<S> {
        let fat = self.clone();
        async move { fat.perform(op).await }
    }
}

impl<S> Clone for Fat<S> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<S> core::fmt::Debug for Fat<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Fat")
            .field("id", &self.0.key.id())
            .field("type", &self.0.geometry.fat_type())
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

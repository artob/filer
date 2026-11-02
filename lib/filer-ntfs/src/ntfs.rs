// This is free and unencumbered software released into the public domain.

use crate::{
    NtfsError, Options,
    file_name::FileName,
    index::Index,
    reparse,
    stream::Stream,
    time,
    volume::{
        FIRST_USER_RECORD, Node, StandardInformation, Volume, Walker, is_valid_name, listed_name,
    },
    wtf8,
};
use alloc::{sync::Arc, vec::Vec};
use core::future::Future;
use filer_api::{
    Completion, Dir, DirCookie, Driver, DriverId, Entry, EntryName, Error, ErrorKind, FileId,
    FileKind, Handle, HandleKind, IoBufMut, LinkTarget, Metadata, Op, OpKind, OpenHow, Output,
    ReadAt, Resolve, Resolver, Rights, Target, XattrEntry, XattrNs, XattrOp, path::RelPath,
};

/// A read-only driver for an NTFS volume.
///
/// It reads a bare volume, with no partition table, from any [`ReadAt`]
/// source: a disk image [`File`](filer_api::File) of another driver,
/// bytes in memory, or a block device. Opening reads and validates the
/// boot sector, the MFT's own record, `$Volume`, and `$UpCase`; files and
/// directories are read as operations need them.
///
/// Names are matched exactly, else case-insensitively by the volume's own
/// `$UpCase` table, as on Windows. Listings show each name of a file once,
/// skipping the DOS (8.3) names that shadow long ones, and hide the
/// system files (`$MFT`, `$LogFile`, and the rest). A file's id is its MFT
/// record number, so hard links share it. Times are UTC, at 100 ns.
///
/// Named data streams (alternate data streams) are extended attributes
/// in the `user` namespace, as ntfs-3g has them: `file:stream` is
/// `user.stream` of `file`, matched as names are. Metadata reports file
/// flags: hidden, system, archive, sparse, compressed, offline,
/// encrypted, and recall on access as dataless.
///
/// Compressed files (LZNT1) read as any other. Symbolic links, junctions,
/// and WSL symlinks are symlinks, confined like any other: a target that
/// is an absolute path on a drive (`C:\\path`) is taken from the root of
/// the volume, assumed to be that drive. Reparse points of other tags,
/// such as cloud placeholders, read as
/// [`Unsupported`](ErrorKind::Unsupported). Interix special files, as
/// ntfs-3g makes FIFOs and sockets, and as it and Services for UNIX made
/// symlinks and devices, are what they say; since only a system file's
/// contents tell, listings report no kind for system files. Writing is
/// out of scope:
/// everything but reading is [`Unsupported`](ErrorKind::Unsupported), as
/// are encrypted files.
///
/// # Examples
///
/// ```no_run
/// use filer_ntfs::Ntfs;
///
/// # async fn example(image: Vec<u8>) -> filer_api::Result<()> {
/// let root = Ntfs::open(image).await?;
/// let file = root.open_file("Documents/notes.txt").await?;
/// let text = file.read_to_end().await?;
/// # Ok(())
/// # }
/// ```
pub struct Ntfs<S>(Arc<Volume<S>>);

/// The private state of a handle.
enum Payload {
    Dir { node: Node, index: Index },
    File { node: Node, data: Stream },
}

impl Payload {
    const fn node(&self) -> &Node {
        match self {
            Self::Dir { node, .. } | Self::File { node, .. } => node,
        }
    }
}

impl<S: ReadAt + 'static> Ntfs<S> {
    /// Opens the volume in `source` and returns its root directory, with
    /// [`Rights::READ`], with the default [`Options`].
    ///
    /// # Errors
    ///
    /// [`Corrupt`](ErrorKind::Corrupt), with an [`NtfsError`] as its
    /// source, if the boot sector, the MFT, `$Volume`, `$UpCase`, or the
    /// root directory is invalid, or the source is shorter than the
    /// volume; [`Unsupported`](ErrorKind::Unsupported) if the volume is
    /// dirty or of an unknown version; any error reading the source.
    pub async fn open(source: S) -> Result<Dir<Self>, Error> {
        Self::open_with(source, Options::new()).await
    }

    /// Opens the volume in `source` as [`open`](Self::open) does, reading
    /// it per `options`.
    ///
    /// # Errors
    ///
    /// As for [`open`](Self::open).
    pub async fn open_with(source: S, options: Options) -> Result<Dir<Self>, Error> {
        let op = OpKind::Open;
        let volume = Volume::open(source, options).await?;
        let node = volume.root(op).await?;
        let index = volume.index(&node, op)?;
        let root = volume
            .key
            .handle(HandleKind::Dir, Payload::Dir { node, index });
        Ok(Dir::new(Self(Arc::new(volume)), root)?.restrict(Rights::READ))
    }
}

impl<S: ReadAt> Ntfs<S> {
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
                        .and_then(|payload| self.metadata(payload.node(), Some(payload), kind)),
                    Target::Entry {
                        at,
                        name,
                        follow,
                        resolve,
                    } => match self.entry(at, name, *follow, *resolve, kind).await {
                        Ok(node) => self.metadata(&node, None, kind),
                        Err(error) => Err(error),
                    }
                    .map_err(|error| with_name(error, name)),
                };
                Completion::new(result.map(Output::Metadata))
            },
            Op::ReadDir { dir, cookie, max } => {
                Completion::new(self.read_dir(&dir, cookie, max, kind).await)
            },
            Op::ReadLink { at, name } => {
                let result = self
                    .entry(&at, &name, false, Resolve::default(), kind)
                    .await
                    .and_then(|node| match node.link() {
                        Some(target) => Ok(LinkTarget::from_bytes(target.to_vec())),
                        None => Err(Error::new(ErrorKind::NotASymlink, kind)),
                    });
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
                Completion::new(self.xattr_names(&handle).map(Output::XattrNames))
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

    fn dir<'h>(&self, handle: &'h Handle, op: OpKind) -> Result<(&'h Node, &'h Index), Error> {
        match self.payload(handle, op)? {
            Payload::Dir { node, index } => Ok((node, index)),
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
                let index = self.0.index(&node, op)?;
                Payload::Dir { node, index }
            },
            (FileKind::File, HandleKind::File) if node.is_opaque() => {
                return Err(Error::new(ErrorKind::Unsupported, op));
            },
            (FileKind::File, HandleKind::File) => {
                let data = self.0.data(&node, op)?;
                Payload::File { node, data }
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
        let data = match self.payload(file, op)? {
            Payload::File { data, .. } => data,
            Payload::Dir { .. } => return Err(Error::new(ErrorKind::IsADirectory, op)),
        };
        let volume = &self.0;
        let bytes = data
            .read(&volume.source, &volume.geometry, off, buf.spare(), op)
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

    /// Reads as much of the extended attribute `name` of `handle`, a named
    /// data stream, as fits into `buf`, returning its whole length.
    async fn xattr(
        &self,
        handle: &Handle,
        name: &XattrEntry,
        buf: &mut impl IoBufMut,
    ) -> Result<Option<u64>, Error> {
        let op = OpKind::GetXattr;
        let node = self.payload(handle, op)?.node();
        let id = self.0.key.id();
        let units = match name {
            XattrEntry::Foreign { name, .. } if name.origin() != id => {
                return Err(Error::new(ErrorKind::ForeignNameMismatch, op));
            },
            XattrEntry::Utf8(name) if name.ns() == XattrNs::User => {
                Some(name.name().encode_utf16().collect())
            },
            XattrEntry::Foreign {
                ns: XattrNs::User,
                name,
            } => wtf8::decode(name.as_bytes()),
            // Streams are only in the `user` namespace.
            _ => None,
        };
        let Some(units) = units else {
            return Ok(None);
        };
        let Some(stream) = self.0.stream(node, &units, op)? else {
            return Ok(None);
        };
        let len = stream.len();
        let want = usize::try_from(len).unwrap_or(usize::MAX).min(buf.spare());
        let volume = &self.0;
        let bytes = stream
            .read_exact(&volume.source, &volume.geometry, 0, want, op)
            .await?;
        buf.fill(&mut |spare| {
            let n = spare.len().min(bytes.len());
            let (Some(to), Some(from)) = (spare.get_mut(..n), bytes.get(..n)) else {
                return Ok(0);
            };
            to.copy_from_slice(from);
            Ok(n)
        })?;
        Ok(Some(len))
    }

    /// Lists the extended attributes of `handle`: its named data streams.
    fn xattr_names(&self, handle: &Handle) -> Result<Vec<XattrEntry>, Error> {
        let op = OpKind::ListXattrs;
        let node = self.payload(handle, op)?.node();
        let bad = || {
            NtfsError::BadStreamName {
                record: node.number(),
            }
            .into_error(op)
        };
        self.0
            .streams(node, op)?
            .iter()
            .map(|units| {
                let raw = wtf8::encode(units);
                self.0
                    .key
                    .xattr_entry(XattrNs::User, &raw)
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

    /// Returns the id of the file with record `number`: the volume serial
    /// number, and the record.
    fn id(&self, number: u64) -> FileId {
        FileId::new(self.0.geometry.serial(), u128::from(number))
    }

    /// Returns the metadata of `node`, opened as `payload` if it is a
    /// handle's.
    fn metadata(
        &self,
        node: &Node,
        payload: Option<&Payload>,
        op: OpKind,
    ) -> Result<Metadata, Error> {
        let information =
            StandardInformation::of(node.file()).map_err(|error| error.into_error(op))?;
        let mut metadata = Metadata::default();
        metadata.kind = Some(node.kind());
        metadata.id = Some(self.id(node.number()));
        metadata.len = match (payload, node.kind(), node.link()) {
            (Some(Payload::File { data, .. }), _, _) => Some(data.len()),
            (_, _, Some(target)) => u64::try_from(target.len()).ok(),
            (_, FileKind::Dir, _) => None,
            (_, _, None) => Some(self.0.data(node, op)?.len()),
        };
        metadata.created = time::timestamp(information.created);
        metadata.modified = time::timestamp(information.modified);
        metadata.changed = time::timestamp(information.changed);
        metadata.accessed = time::timestamp(information.accessed);
        metadata.attrs = Some(information.flags());
        Ok(metadata)
    }

    async fn read_dir(
        &self,
        dir: &Handle,
        cookie: Option<DirCookie>,
        max: usize,
        op: OpKind,
    ) -> Result<Output, Error> {
        let (node, index) = self.dir(dir, op)?;
        let volume = &self.0;
        let skip = cookie.map_or(0, DirCookie::get);
        let mut seen = 0u64;
        let mut entries = Vec::new();
        let mut walk = index.walk();
        while entries.len() < max {
            let Some((file, entry)) = walk
                .next(index, &volume.source, &volume.geometry, op)
                .await?
            else {
                return Ok(Output::Entries {
                    entries,
                    next: None,
                });
            };
            if file.record < FIRST_USER_RECORD {
                continue;
            }
            let Some(name) = listed_name(&entry) else {
                continue;
            };
            seen = seen.saturating_add(1);
            if seen <= skip {
                continue;
            }
            let bad = || {
                NtfsError::BadFileName {
                    record: node.number(),
                }
                .into_error(op)
            };
            if !is_valid_name(&name) {
                return Err(bad());
            }
            let name = volume.key.entry_name(&name).map_err(|_| bad())?;
            let mut listed = Entry::new(name);
            listed.kind = kind_of(&entry);
            listed.id = Some(self.id(file.record));
            entries.push(listed);
        }
        Ok(Output::Entries {
            entries,
            next: Some(DirCookie::new(seen)),
        })
    }
}

/// Returns the kind of the file an index entry names, by the attributes
/// and reparse tag its `$FILE_NAME` carries, or `None` for a system file,
/// which only its contents tell from an Interix special file.
fn kind_of(entry: &FileName) -> Option<FileKind> {
    let attributes = entry.attributes;
    if attributes & FileName::REPARSE_POINT != 0 && reparse::is_link(entry.reparse_tag) {
        Some(FileKind::Symlink)
    } else if attributes & FileName::DIRECTORY != 0 {
        Some(FileKind::Dir)
    } else if attributes & FileName::REPARSE_POINT == 0 && attributes & FileName::SYSTEM != 0 {
        None
    } else {
        Some(FileKind::File)
    }
}

impl<S: ReadAt + 'static> Driver for Ntfs<S> {
    fn id(&self) -> DriverId {
        self.0.key.id()
    }

    fn submit(&self, op: Op) -> impl Future<Output = Completion> + Send + use<S> {
        let ntfs = self.clone();
        async move { ntfs.perform(op).await }
    }
}

impl<S> Clone for Ntfs<S> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<S> core::fmt::Debug for Ntfs<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Ntfs")
            .field("id", &self.0.key.id())
            .field("serial", &self.0.geometry.serial())
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

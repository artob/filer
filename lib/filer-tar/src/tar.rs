// This is free and unencumbered software released into the public domain.

use crate::{
    Limits,
    index::{Member, index},
};
use alloc::{sync::Arc, vec::Vec};
use core::future::Future;
use filer_api::{
    BufResult, Completion, Dir, DirCookie, Driver, DriverId, DriverKey, Entry, EntryName, Error,
    ErrorKind, FileId, Handle, HandleKind, IoBufMut, LinkTarget, Metadata, Op, OpKind, OpenHow,
    Output, ReadAt, Resolve, Resolver, Rights, Target,
    path::RelPath,
    tree::{Ino, Node, ROOT, Tree},
};

/// A read-only driver over a TAR archive.
///
/// [`open`](Self::open) reads the archive's headers once, from any
/// [`ReadAt`] source, and builds an index; afterwards, reading a member
/// reads its bytes directly from the source, since TAR stores them
/// contiguously. The archive's root directory comes back as a [`Dir`]
/// with only [`Rights::READ`].
///
/// It reads POSIX ustar headers (including the name prefix), PAX extended
/// headers (`path`, `linkpath`, `size`, `mtime`), and GNU long names and
/// base-256 numbers. Global PAX headers are ignored, GNU sparse members are
/// [`Unsupported`](ErrorKind::Unsupported), and, as POSIX requires, other
/// unknown member types are read as regular files.
///
/// The archive is untrusted. Opening it fails, rather than exposing a
/// member, if any member name is absolute, contains `..`, or contains NUL
/// ([`InvalidName`](ErrorKind::InvalidName)); if a header is malformed or
/// truncated ([`Corrupt`](ErrorKind::Corrupt)); or if a [`Limits`] value
/// is exceeded ([`LimitExceeded`](ErrorKind::LimitExceeded)). The error's
/// source names the offending header's offset. Directories implied by
/// member names are created; of duplicate members, the later wins; a
/// hardlink shares its target's node, and so its [`FileId`]. Symlinks in
/// the archive resolve within it, confined like any other path.
///
/// Metadata has kind, length, id, and, for regular files, the modification
/// time. Listings are sorted by name bytes.
///
/// # Examples
///
/// ```
/// use filer_tar::Tar;
///
/// # filer_spec::block_on(async {
/// let archive: &'static [u8] = include_bytes!("../tests/data/hello.tar");
/// let root = Tar::open(archive).await?;
/// let file = root.open_file("hello/world.txt").await?;
/// assert_eq!(file.read_to_end().await?, b"Hello, world!\n");
/// # Ok::<(), filer_api::Error>(())
/// # }).unwrap();
/// ```
pub struct Tar<S> {
    key: Arc<DriverKey>,
    tree: Arc<Tree<Member>>,
    source: Arc<S>,
}

impl<S: ReadAt + 'static> Tar<S> {
    /// Indexes the archive in `source` with the default [`Limits`] and
    /// returns its root directory.
    ///
    /// # Errors
    ///
    /// See the [type docs](Self); also any error reading `source`.
    pub async fn open(source: S) -> Result<Dir<Self>, Error> {
        Self::open_with(source, Limits::default()).await
    }

    /// Indexes the archive in `source` with `limits` and returns its root
    /// directory.
    ///
    /// # Errors
    ///
    /// See the [type docs](Self); also any error reading `source`.
    pub async fn open_with(source: S, limits: Limits) -> Result<Dir<Self>, Error> {
        let tree = index(&source, &limits).await?;
        let tar = Self {
            key: Arc::new(DriverKey::new()),
            tree: Arc::new(tree),
            source: Arc::new(source),
        };
        let root = tar.key.handle(HandleKind::Dir, ROOT);
        Ok(Dir::new(tar, root)?.restrict(Rights::READ))
    }

    async fn perform(&self, op: Op) -> Completion {
        let kind = op.kind();
        let error = |error| Error::new(error, kind);
        match op {
            Op::Open { at, path, how } => Completion::new(
                self.open_entry(&at, &path, how)
                    .map_err(|error| error.with_path(&*path)),
            ),
            Op::Close { handle } => {
                Completion::new(self.ino(&handle).map(|_| Output::Done).map_err(error))
            },
            Op::Read { file, mut buf, off } => {
                let result = match self.member(&file) {
                    Ok(member) => self.read(member, &mut buf, off).await,
                    Err(kind) => Err(error(kind)),
                };
                Completion::new(result.map(Output::Read)).with_buf(buf)
            },
            Op::Stat { target, .. } => {
                let ino = match &target {
                    Target::Handle(handle) => self.ino(handle).map_err(error),
                    Target::Entry {
                        at,
                        name,
                        follow,
                        resolve,
                    } => self
                        .entry(at, name, *follow, *resolve, kind)
                        .map_err(|error| with_name(error, name)),
                };
                Completion::new(ino.map(|ino| Output::Metadata(self.metadata(ino))))
            },
            Op::ReadDir { dir, cookie, max } => {
                Completion::new(self.read_dir(&dir, cookie, max).map_err(error))
            },
            Op::ReadLink { at, name } => {
                let result = self
                    .entry(&at, &name, false, Resolve::default(), kind)
                    .and_then(|ino| match self.tree.node(ino) {
                        Node::Symlink(target) => {
                            Ok(Output::LinkTarget(LinkTarget::from_bytes(target.to_vec())))
                        },
                        _ => Err(error(ErrorKind::NotASymlink)),
                    });
                Completion::new(result.map_err(|error| with_name(error, &name)))
            },
            op => Completion::new(Err(Error::new(ErrorKind::Unsupported, op.kind()))),
        }
    }

    /// Reads from `member` at `off` into the spare capacity of `buf`,
    /// never past the member's end.
    async fn read(
        &self,
        member: Member,
        buf: &mut impl IoBufMut,
        off: u64,
    ) -> Result<usize, Error> {
        let remaining = member.size.saturating_sub(off);
        let wanted = usize::try_from(remaining).map_or(buf.spare(), |r| r.min(buf.spare()));
        if wanted == 0 {
            return Ok(0);
        }
        // A bounded intermediate buffer keeps the source from reading past
        // the member into the next header.
        let at = member.offset + off;
        let BufResult(result, data) = self
            .source
            .read_exact_at(Vec::with_capacity(wanted), at)
            .await;
        result.map_err(|error| match error.kind() {
            ErrorKind::UnexpectedEof => Error::new(ErrorKind::Corrupt, OpKind::Read),
            _ => error,
        })?;
        buf.fill(&mut |spare| {
            spare[..data.len()].copy_from_slice(&data);
            Ok(data.len())
        })
    }

    /// Returns the inode a handle refers to, if this archive minted it.
    fn ino(&self, handle: &Handle) -> Result<Ino, ErrorKind> {
        self.key
            .payload::<Ino>(handle)
            .copied()
            .ok_or(ErrorKind::CrossDriver)
    }

    fn dir(&self, handle: &Handle) -> Result<Ino, ErrorKind> {
        let ino = self.ino(handle)?;
        match self.tree.node(ino) {
            Node::Dir(_) => Ok(ino),
            _ => Err(ErrorKind::NotADirectory),
        }
    }

    fn member(&self, handle: &Handle) -> Result<Member, ErrorKind> {
        match self.tree.node(self.ino(handle)?) {
            Node::File(member) => Ok(*member),
            Node::Dir(_) => Err(ErrorKind::IsADirectory),
            _ => Err(ErrorKind::NotRegularFile),
        }
    }

    fn open_entry(&self, at: &Handle, path: &RelPath, how: OpenHow) -> Result<Output, Error> {
        let op = OpKind::Open;
        let dir = self.dir(at).map_err(|error| Error::new(error, op))?;
        let ino = self
            .tree
            .resolve(&dir, path.as_str().as_bytes(), how.resolve, true, op)?;
        let mismatch = match (self.tree.node(ino), how.kind) {
            (Node::Dir(_), HandleKind::Dir) | (Node::File(_), HandleKind::File) => None,
            (Node::Dir(_), HandleKind::File) => Some(ErrorKind::IsADirectory),
            (_, HandleKind::Dir) => Some(ErrorKind::NotADirectory),
            (_, HandleKind::File) => Some(ErrorKind::NotRegularFile),
            _ => Some(ErrorKind::Unsupported),
        };
        match mismatch {
            None => Ok(Output::Opened(self.key.handle(how.kind, ino))),
            Some(error) => Err(Error::new(error, op)),
        }
    }

    fn entry(
        &self,
        at: &Handle,
        name: &EntryName,
        follow: bool,
        resolve: Resolve,
        op: OpKind,
    ) -> Result<Ino, Error> {
        let dir = self.dir(at).map_err(|error| Error::new(error, op))?;
        let bytes = match name {
            EntryName::Utf8(name) => name.as_str().as_bytes(),
            EntryName::Foreign(name) if name.origin() == self.key.id() => name.as_bytes(),
            EntryName::Foreign(_) => return Err(Error::new(ErrorKind::ForeignNameMismatch, op)),
        };
        self.tree.resolve(&dir, bytes, resolve, follow, op)
    }

    fn metadata(&self, ino: Ino) -> Metadata {
        let mut metadata = Metadata::default();
        let node = self.tree.node(ino);
        metadata.kind = Some(node.kind());
        metadata.len = Some(match node {
            Node::Dir(entries) => entries.len() as u64,
            Node::File(member) => member.size,
            Node::Symlink(target) => target.len() as u64,
            Node::Special(_) => 0,
        });
        if let Node::File(member) = node {
            metadata.modified = member.modified;
        }
        metadata.id = Some(FileId::new(0, ino as u128));
        metadata
    }

    fn read_dir(
        &self,
        dir: &Handle,
        cookie: Option<DirCookie>,
        max: usize,
    ) -> Result<Output, ErrorKind> {
        let children = self
            .tree
            .entries(self.dir(dir)?)
            .expect("`dir` returns directories");
        let start = cookie.map_or(0, |cookie| {
            usize::try_from(cookie.get()).unwrap_or(usize::MAX)
        });
        let mut entries = Vec::new();
        for (name, &ino) in children.iter().skip(start).take(max) {
            let name = self
                .key
                .entry_name(name)
                .expect("archive names are checked when indexed");
            let mut entry = Entry::new(name);
            entry.kind = Some(self.tree.node(ino).kind());
            entry.id = Some(FileId::new(0, ino as u128));
            entries.push(entry);
        }
        let end = start.saturating_add(entries.len());
        let next = (end < children.len()).then(|| DirCookie::new(end as u64));
        Ok(Output::Entries { entries, next })
    }
}

impl<S: ReadAt + 'static> Driver for Tar<S> {
    fn id(&self) -> DriverId {
        self.key.id()
    }

    fn submit(&self, op: Op) -> impl Future<Output = Completion> + Send + use<S> {
        let tar = self.clone();
        async move { tar.perform(op).await }
    }
}

impl<S> Clone for Tar<S> {
    fn clone(&self) -> Self {
        Self {
            key: self.key.clone(),
            tree: self.tree.clone(),
            source: self.source.clone(),
        }
    }
}

impl<S> core::fmt::Debug for Tar<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Tar")
            .field("id", &self.key.id())
            .field("nodes", &self.tree.node_count())
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

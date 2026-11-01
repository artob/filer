// This is free and unencumbered software released into the public domain.

use crate::HeapBuilder;
use alloc::{collections::BTreeMap, sync::Arc, vec::Vec};
use core::future::Future;
use filer_api::{
    BlockingResolver, Completion, Dir, DirCookie, Driver, DriverId, DriverKey, Entry, EntryName,
    Error, ErrorKind, FileId, FileKind, Handle, HandleKind, IoBufMut, LinkTarget, Metadata, Op,
    OpKind, OpenHow, Output, Resolve, Target, XattrEntry, XattrName, XattrOp,
    path::RelPath,
    tree::{Ino, Node, ROOT, Tree},
};

/// Extended attributes, by inode.
pub(crate) type Xattrs = BTreeMap<Ino, BTreeMap<XattrName, Vec<u8>>>;

/// An in-memory filesystem driver.
///
/// A heap is built once by a [`HeapBuilder`] and is read-only so far.
/// Clones share the same contents and the same [`DriverId`]. Directory
/// listings are sorted by the bytes of each name, and [`DirCookie`]s are
/// entry indexes.
///
/// Paths resolve through the generic [`BlockingResolver`], confined
/// beneath the directory handle they are resolved against, following
/// symlinks per the operation's [`Resolve`] policy. Following a single entry, as a
/// following stat does, uses the default policy.
///
/// Metadata has kind, length, and id; no timestamps yet. File ids use
/// volume 0 and the inode number. Extended attributes, in any namespace,
/// are set by the builder.
///
/// # Examples
///
/// ```
/// use filer_heap::Heap;
///
/// let heap = Heap::builder()
///     .file("config/app.toml", "debug = true")
///     .symlink("current", "config")
///     .build();
/// let root = heap.root_dir();
/// # block_on(async {
/// let file = root.open_file("current/app.toml").await?;
/// assert_eq!(file.read_to_end().await?, b"debug = true");
/// # Ok::<(), filer_api::Error>(())
/// # }).unwrap();
/// # fn block_on<F: core::future::Future>(f: F) -> F::Output {
/// #     let mut f = core::pin::pin!(f);
/// #     let mut cx = core::task::Context::from_waker(core::task::Waker::noop());
/// #     loop {
/// #         if let core::task::Poll::Ready(v) = f.as_mut().poll(&mut cx) {
/// #             return v;
/// #         }
/// #     }
/// # }
/// ```
#[derive(Clone, Debug)]
pub struct Heap {
    key: Arc<DriverKey>,
    tree: Arc<Tree<Vec<u8>>>,
    xattrs: Arc<Xattrs>,
}

impl Heap {
    /// Returns a builder for a new heap.
    pub fn builder() -> HeapBuilder {
        HeapBuilder::new()
    }

    pub(crate) fn new(tree: Tree<Vec<u8>>, xattrs: Xattrs) -> Self {
        Self {
            key: Arc::new(DriverKey::new()),
            tree: Arc::new(tree),
            xattrs: Arc::new(xattrs),
        }
    }

    /// Returns a new handle to the root directory.
    #[must_use]
    pub fn root(&self) -> Handle {
        self.key.handle(HandleKind::Dir, ROOT)
    }

    /// Returns the root directory, with all rights, for the frontend.
    #[must_use]
    pub fn root_dir(&self) -> Dir<Self> {
        Dir::new(self.clone(), self.root()).expect("the root is a directory of this heap")
    }

    /// Performs `op` synchronously.
    fn perform(&self, op: Op) -> Completion {
        let kind = op.kind();
        let error = |error| Error::new(error, kind);
        match op {
            Op::Open { at, path, how } => Completion::new(
                self.open(&at, &path, how)
                    .map_err(|error| error.with_path(&*path)),
            ),
            Op::Close { handle } => {
                Completion::new(self.ino(&handle).map(|_| Output::Done).map_err(error))
            },
            Op::Read { file, mut buf, off } => {
                let result = self
                    .ino(&file)
                    .and_then(|ino| self.contents(ino))
                    .map_err(error)
                    .and_then(|data| {
                        let start = usize::try_from(off).unwrap_or(usize::MAX);
                        let data = data.get(start..).unwrap_or_default();
                        buf.fill(&mut |spare| {
                            let n = spare.len().min(data.len());
                            spare[..n].copy_from_slice(&data[..n]);
                            Ok(n)
                        })
                    })
                    .map(Output::Read);
                Completion::new(result).with_buf(buf)
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
            Op::Xattr(XattrOp::Get {
                handle,
                name,
                mut buf,
            }) => {
                let result = self
                    .xattr(&handle, &name)
                    .map_err(error)
                    .and_then(|value| {
                        let Some(value) = value else {
                            return Ok(None);
                        };
                        buf.fill(&mut |spare| {
                            let n = spare.len().min(value.len());
                            spare[..n].copy_from_slice(&value[..n]);
                            Ok(n)
                        })?;
                        Ok(Some(value.len() as u64))
                    })
                    .map(Output::Xattr);
                Completion::new(result).with_buf(buf)
            },
            Op::Xattr(XattrOp::List { handle }) => {
                let result = self.ino(&handle).map(|ino| {
                    let names = self.xattrs.get(&ino).into_iter().flat_map(BTreeMap::keys);
                    Output::XattrNames(names.map(XattrEntry::from).collect())
                });
                Completion::new(result.map_err(error))
            },
            op => op.unsupported(),
        }
    }

    /// Returns the inode a handle refers to, if this heap minted it.
    fn ino(&self, handle: &Handle) -> Result<Ino, ErrorKind> {
        self.key
            .payload::<Ino>(handle)
            .copied()
            .ok_or(ErrorKind::CrossDriver)
    }

    /// Returns the value of an extended attribute of a handle's inode.
    fn xattr(&self, handle: &Handle, name: &XattrEntry) -> Result<Option<&[u8]>, ErrorKind> {
        let ino = self.ino(handle)?;
        let name = match name {
            XattrEntry::Utf8(name) => name,
            // The heap's names are all UTF-8, so it has none of its own.
            XattrEntry::Foreign { name, .. } if name.origin() == self.key.id() => return Ok(None),
            XattrEntry::Foreign { .. } => return Err(ErrorKind::ForeignNameMismatch),
        };
        let xattrs = self.xattrs.get(&ino);
        Ok(xattrs
            .and_then(|xattrs| xattrs.get(name))
            .map(Vec::as_slice))
    }

    /// Returns the inode of a directory handle.
    fn dir(&self, handle: &Handle) -> Result<Ino, ErrorKind> {
        let ino = self.ino(handle)?;
        match self.tree.node(ino) {
            Node::Dir(_) => Ok(ino),
            _ => Err(ErrorKind::NotADirectory),
        }
    }

    /// Returns a regular file's contents.
    fn contents(&self, ino: Ino) -> Result<&[u8], ErrorKind> {
        match self.tree.node(ino) {
            Node::File(data) => Ok(data),
            Node::Dir(_) => Err(ErrorKind::IsADirectory),
            Node::Symlink(_) | Node::Special(_) => Err(ErrorKind::NotRegularFile),
        }
    }

    fn open(&self, at: &Handle, path: &RelPath, how: OpenHow) -> Result<Output, Error> {
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

    /// Looks up one entry of a directory, following it under `resolve` if
    /// it is a symlink and `follow` is set.
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
        let (kind, len) = match self.tree.node(ino) {
            Node::Dir(entries) => (FileKind::Dir, entries.len()),
            Node::File(data) => (FileKind::File, data.len()),
            Node::Symlink(target) => (FileKind::Symlink, target.len()),
            Node::Special(kind) => (*kind, 0),
        };
        metadata.kind = Some(kind);
        metadata.len = Some(len as u64);
        metadata.id = Some(Self::file_id(ino));
        metadata
    }

    fn file_id(ino: Ino) -> FileId {
        FileId::new(0, ino as u128)
    }

    fn read_dir(
        &self,
        dir: &Handle,
        cookie: Option<DirCookie>,
        max: usize,
    ) -> Result<Output, ErrorKind> {
        let Node::Dir(children) = self.tree.node(self.dir(dir)?) else {
            unreachable!("`dir` returns directories");
        };
        let start = cookie.map_or(0, |cookie| {
            usize::try_from(cookie.get()).unwrap_or(usize::MAX)
        });
        let mut entries = Vec::new();
        for (name, &ino) in children.iter().skip(start).take(max) {
            let name = self
                .key
                .entry_name(name)
                .expect("heap names are valid entry names");
            let mut entry = Entry::new(name);
            entry.kind = self.metadata(ino).kind;
            entry.id = Some(Self::file_id(ino));
            entries.push(entry);
        }
        let end = start.saturating_add(entries.len());
        let next = (end < children.len()).then(|| DirCookie::new(end as u64));
        Ok(Output::Entries { entries, next })
    }
}

impl Driver for Heap {
    fn id(&self) -> DriverId {
        self.key.id()
    }

    fn submit(&self, op: Op) -> impl Future<Output = Completion> + Send + use<> {
        let heap = self.clone();
        async move { heap.perform(op) }
    }
}

/// Adds an entry's name to an error, if it is UTF-8.
fn with_name(error: Error, name: &EntryName) -> Error {
    match name.as_name() {
        Some(name) => error.with_path(name),
        None => error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::borrow::ToOwned;
    use alloc::{
        string::{String, ToString},
        vec,
    };
    use core::task::{Context, Poll, Waker};
    use filer_api::{ErasedBufMut, MetaMask, rel};

    fn block_on<F: Future>(future: F) -> F::Output {
        let mut future = core::pin::pin!(future);
        let mut cx = Context::from_waker(Waker::noop());
        loop {
            if let Poll::Ready(output) = future.as_mut().poll(&mut cx) {
                return output;
            }
        }
    }

    fn fixture() -> Heap {
        Heap::builder()
            .file("a/b/c.txt", "contents")
            .file("a/z.txt", "")
            .dir("a/empty")
            .symlink("a/link", "b/c.txt")
            .build()
    }

    fn open(heap: &Heap, at: &Handle, path: &RelPath, how: OpenHow) -> Result<Handle, Error> {
        let completion = block_on(heap.submit(Op::Open {
            at: at.clone(),
            path: path.to_owned(),
            how,
        }));
        match completion.result? {
            Output::Opened(handle) => Ok(handle),
            output => panic!("unexpected {output:?}"),
        }
    }

    fn read(heap: &Heap, file: &Handle, off: u64, capacity: usize) -> Result<Vec<u8>, Error> {
        let completion = block_on(heap.submit(Op::Read {
            file: file.clone(),
            buf: ErasedBufMut::new(Vec::<u8>::with_capacity(capacity)),
            off,
        }));
        let buf: Vec<u8> = completion.buf.unwrap().downcast().unwrap();
        completion.result.map(|_| buf)
    }

    fn stat(heap: &Heap, target: Target) -> Result<Metadata, Error> {
        let completion = block_on(heap.submit(Op::Stat {
            target,
            mask: MetaMask::all(),
        }));
        match completion.result? {
            Output::Metadata(metadata) => Ok(metadata),
            output => panic!("unexpected {output:?}"),
        }
    }

    fn list(heap: &Heap, dir: &Handle, max: usize) -> Vec<(String, Option<FileKind>)> {
        let mut names = Vec::new();
        let mut cookie = None;
        loop {
            let completion = block_on(heap.submit(Op::ReadDir {
                dir: dir.clone(),
                cookie,
                max,
            }));
            let Ok(Output::Entries { entries, next }) = completion.result else {
                panic!("read_dir failed");
            };
            names.extend(entries.into_iter().map(|e| (e.name.to_string(), e.kind)));
            match next {
                Some(next) => cookie = Some(next),
                None => return names,
            }
        }
    }

    #[test]
    fn lists_directories_in_batches() {
        let heap = fixture();
        let a = open(&heap, &heap.root(), rel!("a"), OpenHow::dir()).unwrap();
        let expected = vec![
            ("b".to_string(), Some(FileKind::Dir)),
            ("empty".to_string(), Some(FileKind::Dir)),
            ("link".to_string(), Some(FileKind::Symlink)),
            ("z.txt".to_string(), Some(FileKind::File)),
        ];
        assert_eq!(list(&heap, &a, 100), expected);
        assert_eq!(list(&heap, &a, 1), expected);
        let empty = open(&heap, &a, rel!("empty"), OpenHow::dir()).unwrap();
        assert_eq!(list(&heap, &empty, 1), vec![]);
    }

    #[test]
    fn rejects_handles_and_names_from_other_drivers() {
        let (heap, other) = (fixture(), fixture());
        let error = open(&heap, &other.root(), rel!("a"), OpenHow::dir()).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::CrossDriver);
        let foreign = DriverKey::new().entry_name(b"\xFF").unwrap();
        let error = stat(
            &heap,
            Target::Entry {
                at: heap.root(),
                name: foreign,
                follow: false,
                resolve: Resolve::default(),
            },
        )
        .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::ForeignNameMismatch);
    }

    #[test]
    fn closes_and_rejects_reads_of_directories() {
        let heap = fixture();
        let root = heap.root();
        let close = block_on(heap.submit(Op::Close {
            handle: root.clone(),
        }));
        assert!(matches!(close.result, Ok(Output::Done)));
        assert_eq!(
            read(&heap, &root, 0, 4).unwrap_err().kind(),
            ErrorKind::IsADirectory
        );
    }
}

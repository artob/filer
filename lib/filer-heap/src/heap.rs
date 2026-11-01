// This is free and unencumbered software released into the public domain.

use crate::{
    HeapBuilder,
    node::{Ino, Node, ROOT, Tree},
};
use alloc::{sync::Arc, vec::Vec};
use core::future::Future;
use filer_api::{
    Completion, DirCookie, Driver, DriverId, DriverKey, Entry, EntryName, Error, ErrorKind, FileId,
    FileKind, Handle, HandleKind, IoBufMut, LinkTarget, Metadata, Op, OpKind, OpenHow, Output,
    Resolve, Resolver, Target, path::RelPath,
};

/// An in-memory filesystem driver.
///
/// A heap is built once by a [`HeapBuilder`] and is read-only so far.
/// Clones share the same contents and the same [`DriverId`]. Directory
/// listings are sorted by the bytes of each name, and [`DirCookie`]s are
/// entry indexes.
///
/// Paths resolve through the generic [`Resolver`], confined beneath the
/// directory handle they are resolved against, following symlinks per the
/// operation's [`Resolve`] policy. Following a single entry, as a
/// following stat does, uses the default policy.
///
/// Metadata has kind, length, and id; no timestamps yet. File ids use
/// volume 0 and the inode number.
///
/// # Examples
///
/// ```
/// use filer_api::{Driver, IoBufMut, Op, OpenHow, Output, rel};
/// use filer_heap::Heap;
///
/// let heap = Heap::builder().file("hello.txt", "Hello!").build();
/// let opened = block_on(heap.submit(Op::Open {
///     at: heap.root(),
///     path: rel!("hello.txt").to_owned(),
///     how: OpenHow::file(),
/// }));
/// let Ok(Output::Opened(file)) = opened.result else { panic!() };
///
/// let buf = filer_api::ErasedBufMut::new(Vec::<u8>::with_capacity(64));
/// let read = block_on(heap.submit(Op::Read { file, buf, off: 0 }));
/// let buf: Vec<u8> = read.buf.unwrap().downcast().unwrap();
/// assert_eq!(buf, b"Hello!");
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
    tree: Arc<Tree>,
}

impl Heap {
    /// Returns a builder for a new heap.
    pub fn builder() -> HeapBuilder {
        HeapBuilder::new()
    }

    pub(crate) fn new(tree: Tree) -> Self {
        Self {
            key: Arc::new(DriverKey::new()),
            tree: Arc::new(tree),
        }
    }

    /// Returns a new handle to the root directory.
    #[must_use]
    pub fn root(&self) -> Handle {
        self.key.handle(HandleKind::Dir, ROOT)
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
                    Target::Entry { at, name, follow } => self
                        .entry(at, name, *follow, kind)
                        .map_err(|error| with_name(error, name)),
                };
                Completion::new(ino.map(|ino| Output::Metadata(self.metadata(ino))))
            },
            Op::ReadDir { dir, cookie, max } => {
                Completion::new(self.read_dir(&dir, cookie, max).map_err(error))
            },
            Op::ReadLink { at, name } => {
                let result = self.entry(&at, &name, false, kind).and_then(|ino| {
                    match &self.tree.nodes[ino] {
                        Node::Symlink(target) => {
                            Ok(Output::LinkTarget(LinkTarget::Utf8(target.clone())))
                        },
                        _ => Err(error(ErrorKind::NotASymlink)),
                    }
                });
                Completion::new(result.map_err(|error| with_name(error, &name)))
            },
            op => Completion::new(Err(Error::new(ErrorKind::Unsupported, op.kind()))),
        }
    }

    /// Returns the inode a handle refers to, if this heap minted it.
    fn ino(&self, handle: &Handle) -> Result<Ino, ErrorKind> {
        self.key
            .payload::<Ino>(handle)
            .copied()
            .ok_or(ErrorKind::CrossDriver)
    }

    /// Returns the inode of a directory handle.
    fn dir(&self, handle: &Handle) -> Result<Ino, ErrorKind> {
        let ino = self.ino(handle)?;
        match self.tree.nodes[ino] {
            Node::Dir(_) => Ok(ino),
            _ => Err(ErrorKind::NotADirectory),
        }
    }

    /// Returns a regular file's contents.
    fn contents(&self, ino: Ino) -> Result<&[u8], ErrorKind> {
        match &self.tree.nodes[ino] {
            Node::File(data) => Ok(data),
            Node::Dir(_) => Err(ErrorKind::IsADirectory),
            Node::Symlink(_) => Err(ErrorKind::NotRegularFile),
        }
    }

    fn open(&self, at: &Handle, path: &RelPath, how: OpenHow) -> Result<Output, Error> {
        let op = OpKind::Open;
        let dir = self.dir(at).map_err(|error| Error::new(error, op))?;
        let ino = self
            .tree
            .resolve(&dir, path.as_str().as_bytes(), how.resolve, true, op)?;
        let mismatch = match (&self.tree.nodes[ino], how.kind) {
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

    /// Looks up one entry of a directory, following it if it is a symlink
    /// and `follow` is set.
    ///
    /// Operations naming a single entry carry no resolution policy yet, so
    /// following uses the default, [`Symlinks::Beneath`].
    fn entry(&self, at: &Handle, name: &EntryName, follow: bool, op: OpKind) -> Result<Ino, Error> {
        let dir = self.dir(at).map_err(|error| Error::new(error, op))?;
        let bytes = match name {
            EntryName::Utf8(name) => name.as_str().as_bytes(),
            EntryName::Foreign(name) if name.origin() == self.key.id() => name.as_bytes(),
            EntryName::Foreign(_) => return Err(Error::new(ErrorKind::ForeignNameMismatch, op)),
        };
        self.tree
            .resolve(&dir, bytes, Resolve::default(), follow, op)
    }

    fn metadata(&self, ino: Ino) -> Metadata {
        let mut metadata = Metadata::default();
        let (kind, len) = match &self.tree.nodes[ino] {
            Node::Dir(entries) => (FileKind::Dir, entries.len()),
            Node::File(data) => (FileKind::File, data.len()),
            Node::Symlink(target) => (FileKind::Symlink, target.len()),
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
        let Node::Dir(children) = &self.tree.nodes[self.dir(dir)?] else {
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
    use filer_api::{ErasedBufMut, MetaMask, Symlinks, name, rel};

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
    fn opens_and_reads_files() {
        let heap = fixture();
        let file = open(&heap, &heap.root(), rel!("a/b/c.txt"), OpenHow::file()).unwrap();
        assert_eq!(read(&heap, &file, 0, 64).unwrap(), b"contents");
        assert_eq!(read(&heap, &file, 3, 2).unwrap(), b"te");
        assert_eq!(read(&heap, &file, 8, 4).unwrap(), b"");
        assert_eq!(read(&heap, &file, u64::MAX, 4).unwrap(), b"");
    }

    #[test]
    fn opens_relative_to_subdirectories() {
        let heap = fixture();
        let a = open(&heap, &heap.root(), rel!("a"), OpenHow::dir()).unwrap();
        let file = open(&heap, &a, rel!("b/c.txt"), OpenHow::file()).unwrap();
        assert_eq!(read(&heap, &file, 0, 64).unwrap(), b"contents");
        let again = open(&heap, &a, RelPath::empty(), OpenHow::dir()).unwrap();
        assert_eq!(heap.ino(&again), heap.ino(&a));
    }

    #[test]
    fn reports_open_errors() {
        let heap = fixture();
        let root = heap.root();
        let kind = |path, how| open(&heap, &root, path, how).unwrap_err().kind();
        assert_eq!(kind(rel!("nope"), OpenHow::file()), ErrorKind::NotFound);
        assert_eq!(kind(rel!("a"), OpenHow::file()), ErrorKind::IsADirectory);
        assert_eq!(
            kind(rel!("a/z.txt"), OpenHow::dir()),
            ErrorKind::NotADirectory
        );
        assert_eq!(
            kind(rel!("a/z.txt/x"), OpenHow::file()),
            ErrorKind::NotADirectory
        );
        let error = open(&heap, &root, rel!("a/nope"), OpenHow::file()).unwrap_err();
        assert_eq!(
            (error.op(), error.path()),
            (OpKind::Open, Some(rel!("a/nope")))
        );
    }

    #[test]
    fn follows_symlinks_beneath_the_handle() {
        let heap = Heap::builder()
            .file("a/b/c.txt", "contents")
            .symlink("a/link", "b/c.txt")
            .symlink("a/up", "../a/b/c.txt")
            .symlink("a/abs", "/a/b/c.txt")
            .build();
        let root = heap.root();
        let a = open(&heap, &root, rel!("a"), OpenHow::dir()).unwrap();
        let how = |symlinks| {
            let mut how = OpenHow::file();
            how.resolve.symlinks = symlinks;
            how
        };
        let contents = |at: &Handle, path, symlinks| {
            let file = open(&heap, at, path, how(symlinks))?;
            read(&heap, &file, 0, 64)
        };
        let kind = |at, path, symlinks| contents(at, path, symlinks).unwrap_err().kind();

        assert_eq!(
            contents(&root, rel!("a/link"), Symlinks::Beneath).unwrap(),
            b"contents"
        );
        // `..` stays beneath the root, but not beneath `a`.
        assert_eq!(
            contents(&root, rel!("a/up"), Symlinks::Beneath).unwrap(),
            b"contents"
        );
        assert_eq!(
            kind(&a, rel!("up"), Symlinks::Beneath),
            ErrorKind::EscapeBlocked
        );
        // Absolute targets escape, unless the handle is treated as the root.
        assert_eq!(
            kind(&root, rel!("a/abs"), Symlinks::Beneath),
            ErrorKind::EscapeBlocked
        );
        assert_eq!(
            contents(&root, rel!("a/abs"), Symlinks::InRoot).unwrap(),
            b"contents"
        );
        assert_eq!(
            kind(&root, rel!("a/link"), Symlinks::Deny),
            ErrorKind::SymlinkLoop
        );

        let stat_link = |follow| {
            stat(
                &heap,
                Target::Entry {
                    at: a.clone(),
                    name: EntryName::from(name!("link")),
                    follow,
                },
            )
            .unwrap()
            .kind
        };
        assert_eq!(stat_link(true), Some(FileKind::File));
        assert_eq!(stat_link(false), Some(FileKind::Symlink));
    }

    #[test]
    fn stats_handles_and_entries() {
        let heap = fixture();
        let a = open(&heap, &heap.root(), rel!("a"), OpenHow::dir()).unwrap();
        let dir = stat(&heap, Target::Handle(a.clone())).unwrap();
        assert_eq!((dir.kind, dir.len), (Some(FileKind::Dir), Some(4)));
        let entry = |name: &str, follow| Target::Entry {
            at: a.clone(),
            name: EntryName::Utf8(name.parse().unwrap()),
            follow,
        };
        let file = stat(&heap, entry("z.txt", true)).unwrap();
        assert_eq!((file.kind, file.len), (Some(FileKind::File), Some(0)));
        let link = stat(&heap, entry("link", false)).unwrap();
        assert_eq!((link.kind, link.len), (Some(FileKind::Symlink), Some(7)));
        let error = stat(&heap, entry("nope", false)).unwrap_err();
        assert_eq!(
            (error.kind(), error.path()),
            (ErrorKind::NotFound, Some(rel!("nope")))
        );
        assert_ne!(dir.id, file.id);
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
    fn reads_links() {
        let heap = fixture();
        let a = open(&heap, &heap.root(), rel!("a"), OpenHow::dir()).unwrap();
        let read_link = |name| {
            block_on(heap.submit(Op::ReadLink {
                at: a.clone(),
                name: EntryName::from(name),
            }))
            .result
        };
        let Ok(Output::LinkTarget(target)) = read_link(name!("link")) else {
            panic!("read_link failed");
        };
        assert_eq!(target.as_str(), Some("b/c.txt"));
        let error = read_link(name!("z.txt")).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::NotASymlink);
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

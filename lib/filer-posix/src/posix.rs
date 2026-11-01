// This is free and unencumbered software released into the public domain.

use crate::{
    errno::{error, retry},
    node::{DirPayload, FilePayload, Listing, Node},
    stat,
};
use core::future::Future;
use filer_api::{
    AmbientAuthority, Completion, Dir, DirCookie, Driver, DriverId, DriverKey, Entry, EntryName,
    Error, ErrorKind, FileId, FileKind, Handle, HandleKind, IoBufMut, LinkTarget, Op, OpKind,
    OpenHow, Output, Resolve, Resolver, Target, path::RelPath,
};
use rustix::{
    fd::{AsFd, BorrowedFd, OwnedFd},
    fs::{AtFlags, DirEntry, Mode, OFlags},
    io::Errno,
};
use std::{path::Path, sync::Arc, vec::Vec};

/// The POSIX driver, over [`rustix`](https://docs.rs/rustix).
///
/// Paths resolve through the generic [`Resolver`], one component at a
/// time: each directory is opened with `O_DIRECTORY | O_NOFOLLOW` relative
/// to the previous one, and symlinks are read and followed by the resolver
/// under the operation's [`Resolve`] policy, so resolution stays beneath
/// the handle even if entries are renamed concurrently. Files are opened
/// with `O_NOFOLLOW`, rejecting anything but regular files, and every
/// descriptor is close-on-exec.
///
/// Directory listings keep their stream open in the handle between
/// batches; a [`DirCookie`] is the number of entries listed so far, and an
/// unexpected cookie restarts the listing and skips that many. Listed ids
/// combine the directory's device with `d_ino`, which for a mount point is
/// the inode underneath, not the mounted root.
///
/// Metadata comes from `fstat` and `fstatat`: kind, length, modification,
/// change, and access times, and id; no birth times yet.
///
/// System calls interrupted by a signal (`EINTR`) are restarted internally,
/// so [`ErrorKind::Interrupted`] never surfaces from this driver.
///
/// # Examples
///
/// ```
/// use filer_api::{MetaMask, ambient_authority};
/// use filer_posix::Posix;
///
/// let posix = Posix::blocking();
/// let tmp = posix.open_ambient(std::env::temp_dir(), ambient_authority())?;
/// # block_on(async {
/// let metadata = tmp.metadata("", MetaMask::KIND).await?;
/// assert!(metadata.kind.unwrap().is_dir());
/// # Ok::<(), filer_api::Error>(())
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
/// # Ok::<(), filer_api::Error>(())
/// ```
#[derive(Clone, Debug)]
pub struct Posix {
    key: Arc<DriverKey>,
}

impl Posix {
    /// Returns a driver that performs each operation synchronously when its
    /// future is first polled, **blocking the calling thread** until the
    /// system calls return.
    ///
    /// That suits command-line tools, tests, and single-threaded programs.
    /// On a multi-threaded async runtime, a slow call (a cold disk, a hung
    /// network mount) stalls every task sharing the thread; a thread-pool
    /// executor will cover that case.
    #[must_use]
    pub fn blocking() -> Self {
        Self {
            key: Arc::new(DriverKey::new()),
        }
    }

    /// Opens the directory at the native `path` as a root for this driver.
    ///
    /// This is the one place native paths enter, so it requires the
    /// [`AmbientAuthority`] token. The handle's error label is `path`.
    ///
    /// # Errors
    ///
    /// Any error opening `path` as a directory.
    pub fn open_ambient_dir(
        &self,
        path: impl AsRef<Path>,
        authority: AmbientAuthority,
    ) -> Result<Handle, Error> {
        let _ = authority;
        let path = path.as_ref();
        let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC;
        retry(|| rustix::fs::open(path, flags, Mode::empty()))
            .map(|fd| self.dir_handle(Arc::new(fd)))
            .map_err(|errno| error(errno, OpKind::Open).with_label(&path.to_string_lossy()))
    }

    /// Opens the directory at the native `path` as a root [`Dir`], with all
    /// rights and `path` as its label, for the frontend.
    ///
    /// # Errors
    ///
    /// As for [`open_ambient_dir`](Self::open_ambient_dir).
    pub fn open_ambient(
        &self,
        path: impl AsRef<Path>,
        authority: AmbientAuthority,
    ) -> Result<Dir<Self>, Error> {
        let path = path.as_ref();
        let handle = self.open_ambient_dir(path, authority)?;
        let dir = Dir::new(self.clone(), handle)?;
        Ok(dir.with_label(&path.to_string_lossy()))
    }

    fn dir_handle(&self, fd: Arc<OwnedFd>) -> Handle {
        self.key.handle(HandleKind::Dir, DirPayload::new(fd))
    }

    /// Performs `op` synchronously.
    fn perform(&self, op: Op) -> Completion {
        let kind = op.kind();
        match op {
            Op::Open { at, path, how } => {
                Completion::new(self.open(&at, &path, how).map_err(|e| e.with_path(&*path)))
            },
            Op::Close { handle } => Completion::new(self.fd(&handle, kind).map(|_| Output::Done)),
            Op::Read { file, mut buf, off } => {
                let result = self.file(&file, kind).and_then(|fd| {
                    buf.fill(&mut |spare| {
                        retry(|| rustix::io::pread(fd, &mut *spare, off))
                            .map_err(|errno| error(errno, kind))
                    })
                });
                Completion::new(result.map(Output::Read)).with_buf(buf)
            },
            Op::Stat { target, .. } => {
                let result = match &target {
                    Target::Handle(handle) => self
                        .fd(handle, kind)
                        .and_then(|fd| retry(|| rustix::fs::fstat(fd)).map_err(|e| error(e, kind))),
                    Target::Entry {
                        at,
                        name,
                        follow,
                        resolve,
                    } => self
                        .entry(at, name, *follow, *resolve, kind)
                        .and_then(|node| stat_node(&node, kind))
                        .map_err(|e| with_name(e, name)),
                };
                Completion::new(result.map(|s| Output::Metadata(stat::metadata(&s))))
            },
            Op::ReadDir { dir, cookie, max } => Completion::new(self.read_dir(&dir, cookie, max)),
            Op::ReadLink { at, name } => {
                let result = self.dir(&at, kind).and_then(|dir| {
                    let bytes = self.name_bytes(&name, kind)?;
                    read_link(dir.fd.as_fd(), bytes, kind)
                });
                Completion::new(
                    result
                        .map(Output::LinkTarget)
                        .map_err(|e| with_name(e, &name)),
                )
            },
            op => Completion::new(Err(Error::new(ErrorKind::Unsupported, op.kind()))),
        }
    }

    /// Returns a directory handle's payload.
    fn dir<'h>(&self, handle: &'h Handle, op: OpKind) -> Result<&'h DirPayload, Error> {
        match self.key.payload::<DirPayload>(handle) {
            Some(dir) => Ok(dir),
            None if self.key.payload::<FilePayload>(handle).is_some() => {
                Err(Error::new(ErrorKind::NotADirectory, op))
            },
            None => Err(Error::new(ErrorKind::CrossDriver, op)),
        }
    }

    /// Returns a file handle's descriptor.
    fn file<'h>(&self, handle: &'h Handle, op: OpKind) -> Result<BorrowedFd<'h>, Error> {
        match self.key.payload::<FilePayload>(handle) {
            Some(file) => Ok(file.fd.as_fd()),
            None if self.key.payload::<DirPayload>(handle).is_some() => {
                Err(Error::new(ErrorKind::IsADirectory, op))
            },
            None => Err(Error::new(ErrorKind::CrossDriver, op)),
        }
    }

    /// Returns any handle's descriptor.
    fn fd<'h>(&self, handle: &'h Handle, op: OpKind) -> Result<BorrowedFd<'h>, Error> {
        match self.key.payload::<DirPayload>(handle) {
            Some(dir) => Ok(dir.fd.as_fd()),
            None => self.file(handle, op),
        }
    }

    /// Returns the bytes of an entry name, refusing foreign names from
    /// other drivers.
    fn name_bytes<'n>(&self, name: &'n EntryName, op: OpKind) -> Result<&'n [u8], Error> {
        match name {
            EntryName::Utf8(name) => Ok(name.as_str().as_bytes()),
            EntryName::Foreign(name) if name.origin() == self.key.id() => Ok(name.as_bytes()),
            EntryName::Foreign(_) => Err(Error::new(ErrorKind::ForeignNameMismatch, op)),
        }
    }

    fn open(&self, at: &Handle, path: &RelPath, how: OpenHow) -> Result<Output, Error> {
        let op = OpKind::Open;
        let start = Node::Dir(self.dir(at, op)?.fd.clone());
        let node = Walker.resolve(&start, path.as_str().as_bytes(), how.resolve, true, op)?;
        match (node, how.kind) {
            (Node::Dir(fd), HandleKind::Dir) => Ok(Output::Opened(self.dir_handle(fd))),
            (Node::Dir(_), HandleKind::File) => Err(Error::new(ErrorKind::IsADirectory, op)),
            (Node::Entry { .. }, HandleKind::Dir) => Err(Error::new(ErrorKind::NotADirectory, op)),
            (Node::Entry { dir, name, kind }, HandleKind::File) => {
                if kind != FileKind::File {
                    return Err(Error::new(ErrorKind::NotRegularFile, op));
                }
                let fd = open_file(dir.as_fd(), &name, op)?;
                Ok(Output::Opened(
                    self.key.handle(HandleKind::File, FilePayload { fd }),
                ))
            },
            _ => Err(Error::new(ErrorKind::Unsupported, op)),
        }
    }

    /// Resolves a single entry of a directory, following it under
    /// `resolve` if it is a symlink and `follow` is set.
    fn entry(
        &self,
        at: &Handle,
        name: &EntryName,
        follow: bool,
        resolve: Resolve,
        op: OpKind,
    ) -> Result<Node, Error> {
        let start = Node::Dir(self.dir(at, op)?.fd.clone());
        let bytes = self.name_bytes(name, op)?;
        Walker.resolve(&start, bytes, resolve, follow, op)
    }

    fn read_dir(
        &self,
        handle: &Handle,
        cookie: Option<DirCookie>,
        max: usize,
    ) -> Result<Output, Error> {
        let op = OpKind::ReadDir;
        let dir = self.dir(handle, op)?;
        let device = retry(|| rustix::fs::fstat(&dir.fd)).map_err(|e| error(e, op))?;
        let device = stat::device(&device);
        let start = cookie.map_or(0, DirCookie::get);
        let mut listing = dir
            .listing
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        // Continue the cached stream if it is where the caller left off;
        // otherwise start over and skip to the cookie.
        let mut current = match listing.take() {
            Some(current) if current.next == start => current,
            _ => open_listing(&dir.fd, start).map_err(|e| error(e, op))?,
        };
        let mut entries = Vec::new();
        let mut done = false;
        while entries.len() < max {
            let Some(raw) = next_entry(&mut current, &dir.fd).map_err(|e| error(e, op))? else {
                done = true;
                break;
            };
            let name = self
                .key
                .entry_name(raw.file_name().to_bytes())
                .map_err(|_| Error::new(ErrorKind::Corrupt, op))?;
            let mut entry = Entry::new(name);
            let kind = raw.file_type();
            entry.kind = (kind != rustix::fs::FileType::Unknown).then(|| stat::file_kind(kind));
            entry.id = Some(FileId::new(device, u128::from(raw.ino())));
            entries.push(entry);
            current.next += 1;
        }
        let next = (!done).then(|| DirCookie::new(current.next));
        if !done {
            *listing = Some(current);
        }
        Ok(Output::Entries { entries, next })
    }
}

impl Driver for Posix {
    fn id(&self) -> DriverId {
        self.key.id()
    }

    fn submit(&self, op: Op) -> impl Future<Output = Completion> + Send + use<> {
        let posix = self.clone();
        async move { posix.perform(op) }
    }
}

/// The [`Resolver`] primitives over `rustix`. A private type, so that
/// callers cannot resolve from descriptors they were never handed.
pub(crate) struct Walker;

impl Resolver for Walker {
    type Node = Node;

    fn lookup(&self, dir: &Node, name: &[u8], op: OpKind) -> Result<Node, Error> {
        let Node::Dir(dir) = dir else {
            return Err(Error::new(ErrorKind::NotADirectory, op));
        };
        let stat = retry(|| rustix::fs::statat(dir, name, AtFlags::SYMLINK_NOFOLLOW))
            .map_err(|e| error(e, op))?;
        let kind = stat::kind(&stat);
        if kind != FileKind::Dir {
            return Ok(Node::Entry {
                dir: dir.clone(),
                name: name.to_vec(),
                kind,
            });
        }
        // `O_NOFOLLOW` fails rather than follows if the entry was replaced
        // by a symlink since the `statat`.
        let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
        let fd = retry(|| rustix::fs::openat(dir, name, flags, Mode::empty()))
            .map_err(|e| error(e, op))?;
        Ok(Node::Dir(Arc::new(fd)))
    }

    fn kind(&self, node: &Node) -> FileKind {
        match node {
            Node::Dir(_) => FileKind::Dir,
            Node::Entry { kind, .. } => *kind,
        }
    }

    fn read_link(&self, dir: &Node, name: &[u8], op: OpKind) -> Result<LinkTarget, Error> {
        let Node::Dir(dir) = dir else {
            return Err(Error::new(ErrorKind::NotADirectory, op));
        };
        read_link(dir.as_fd(), name, op)
    }

    fn crosses_mount(&self, dir: &Node, child: &Node) -> bool {
        let device = |node| stat_node(node, OpKind::Stat).map(|s| stat::device(&s));
        match (device(dir), device(child)) {
            (Ok(a), Ok(b)) => a != b,
            // If either cannot be stat-ed, the next step reports the error.
            _ => false,
        }
    }
}

/// Returns the metadata of a resolution node, without following it.
fn stat_node(node: &Node, op: OpKind) -> Result<rustix::fs::Stat, Error> {
    match node {
        Node::Dir(fd) => retry(|| rustix::fs::fstat(fd)).map_err(|e| error(e, op)),
        Node::Entry { dir, name, .. } => {
            retry(|| rustix::fs::statat(dir, name.as_slice(), AtFlags::SYMLINK_NOFOLLOW))
                .map_err(|e| error(e, op))
        },
    }
}

/// Opens the regular file `name` in `dir` for reading.
///
/// `O_NONBLOCK` keeps a FIFO swapped in since resolution from blocking the
/// open; anything but a regular file is then rejected, and the flag is
/// cleared.
fn open_file(dir: BorrowedFd<'_>, name: &[u8], op: OpKind) -> Result<OwnedFd, Error> {
    let flags = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC;
    let fd =
        retry(|| rustix::fs::openat(dir, name, flags, Mode::empty())).map_err(|e| error(e, op))?;
    let stat = retry(|| rustix::fs::fstat(&fd)).map_err(|e| error(e, op))?;
    if stat::kind(&stat) != FileKind::File {
        return Err(Error::new(ErrorKind::NotRegularFile, op));
    }
    let flags = retry(|| rustix::fs::fcntl_getfl(&fd)).map_err(|e| error(e, op))?;
    retry(|| rustix::fs::fcntl_setfl(&fd, flags - OFlags::NONBLOCK)).map_err(|e| error(e, op))?;
    Ok(fd)
}

/// Reads the target of the symlink `name` in `dir`.
fn read_link(dir: BorrowedFd<'_>, name: &[u8], op: OpKind) -> Result<LinkTarget, Error> {
    match retry(|| rustix::fs::readlinkat(dir, name, Vec::new())) {
        Ok(target) => Ok(LinkTarget::from_bytes(target.into_bytes())),
        Err(Errno::INVAL) => Err(Error::new(ErrorKind::NotASymlink, op)),
        Err(errno) => Err(error(errno, op)),
    }
}

/// Opens a new stream on the directory `fd`, positioned after `position`
/// entries (not counting `.` and `..`).
///
/// A stream stops for good after an error, so an `EINTR` while skipping
/// restarts from the beginning rather than retrying the read.
fn open_listing(fd: &OwnedFd, position: u64) -> rustix::io::Result<Listing> {
    'restart: loop {
        let mut listing = Listing {
            next: 0,
            dir: retry(|| rustix::fs::Dir::read_from(fd))?,
        };
        while listing.next < position {
            match next_raw(&mut listing.dir) {
                Ok(Some(_)) => listing.next += 1,
                Ok(None) => break,
                Err(Errno::INTR) => continue 'restart,
                Err(errno) => return Err(errno),
            }
        }
        return Ok(listing);
    }
}

/// Returns the next entry of a listing, skipping `.` and `..`.
///
/// On `EINTR`, the stream is reopened at the same position, since a stream
/// stops for good after an error.
fn next_entry(listing: &mut Listing, fd: &OwnedFd) -> rustix::io::Result<Option<DirEntry>> {
    loop {
        match next_raw(&mut listing.dir) {
            Err(Errno::INTR) => *listing = open_listing(fd, listing.next)?,
            result => return result,
        }
    }
}

/// Returns the next entry of a stream, skipping `.` and `..`.
fn next_raw(dir: &mut rustix::fs::Dir) -> rustix::io::Result<Option<DirEntry>> {
    loop {
        match dir.read() {
            None => return Ok(None),
            Some(Err(errno)) => return Err(errno),
            Some(Ok(entry)) => match entry.file_name().to_bytes() {
                b"." | b".." => {},
                _ => return Ok(Some(entry)),
            },
        }
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
    use core::task::{Context, Poll, Waker};
    use filer_api::{ErasedBufMut, MetaMask, Symlinks, ambient_authority, name, rel};
    use std::{
        borrow::ToOwned,
        fs,
        os::unix::fs::symlink,
        path::PathBuf,
        process,
        string::{String, ToString},
        sync::atomic::{AtomicUsize, Ordering},
    };

    fn block_on<F: Future>(future: F) -> F::Output {
        let mut future = core::pin::pin!(future);
        let mut cx = Context::from_waker(Waker::noop());
        loop {
            if let Poll::Ready(output) = future.as_mut().poll(&mut cx) {
                return output;
            }
        }
    }

    /// A directory under the system temporary directory, removed on drop.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let n = NEXT.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(std::format!("filer-posix-{}-{n}", process::id()));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// A fixture: `a/b/c.txt`, `a/z.txt`, and links in `a`, under `top`,
    /// with `outside.txt` beside `top`.
    fn fixture() -> (TempDir, Posix, Handle) {
        let temp = TempDir::new();
        let top = temp.0.join("top");
        fs::create_dir_all(top.join("a/b")).unwrap();
        fs::write(top.join("a/b/c.txt"), "contents").unwrap();
        fs::write(top.join("a/z.txt"), "").unwrap();
        fs::write(temp.0.join("outside.txt"), "outside").unwrap();
        symlink("b/c.txt", top.join("a/link")).unwrap();
        symlink("../a/b/c.txt", top.join("a/up")).unwrap();
        symlink("/a/b/c.txt", top.join("a/abs")).unwrap();
        symlink("../../outside.txt", top.join("a/escape")).unwrap();
        let posix = Posix::blocking();
        let root = posix.open_ambient_dir(&top, ambient_authority()).unwrap();
        (temp, posix, root)
    }

    fn open(posix: &Posix, at: &Handle, path: &RelPath, how: OpenHow) -> Result<Handle, Error> {
        let completion = block_on(posix.submit(Op::Open {
            at: at.clone(),
            path: path.to_owned(),
            how,
        }));
        match completion.result? {
            Output::Opened(handle) => Ok(handle),
            output => panic!("unexpected {output:?}"),
        }
    }

    fn read(posix: &Posix, file: &Handle, off: u64, capacity: usize) -> Result<Vec<u8>, Error> {
        let completion = block_on(posix.submit(Op::Read {
            file: file.clone(),
            buf: ErasedBufMut::new(Vec::<u8>::with_capacity(capacity)),
            off,
        }));
        let buf: Vec<u8> = completion.buf.unwrap().downcast().unwrap();
        completion.result.map(|_| buf)
    }

    fn stat(posix: &Posix, target: Target) -> Result<filer_api::Metadata, Error> {
        let completion = block_on(posix.submit(Op::Stat {
            target,
            mask: MetaMask::all(),
        }));
        match completion.result? {
            Output::Metadata(metadata) => Ok(metadata),
            output => panic!("unexpected {output:?}"),
        }
    }

    fn list(posix: &Posix, dir: &Handle, max: usize) -> Vec<(String, Option<FileKind>)> {
        let mut names = Vec::new();
        let mut cookie = None;
        loop {
            let completion = block_on(posix.submit(Op::ReadDir {
                dir: dir.clone(),
                cookie,
                max,
            }));
            let Ok(Output::Entries { entries, next }) = completion.result else {
                panic!("read_dir failed: {:?}", completion.result);
            };
            names.extend(entries.into_iter().map(|e| (e.name.to_string(), e.kind)));
            match next {
                Some(next) => cookie = Some(next),
                None => break,
            }
        }
        names.sort_by(|a, b| a.0.cmp(&b.0));
        names
    }

    fn file_how(symlinks: Symlinks) -> OpenHow {
        let mut how = OpenHow::file();
        how.resolve.symlinks = symlinks;
        how
    }

    #[test]
    fn opens_and_reads_files() {
        let (_temp, posix, root) = fixture();
        let file = open(&posix, &root, rel!("a/b/c.txt"), OpenHow::file()).unwrap();
        assert_eq!(read(&posix, &file, 0, 64).unwrap(), b"contents");
        assert_eq!(read(&posix, &file, 3, 2).unwrap(), b"te");
        assert_eq!(read(&posix, &file, 100, 4).unwrap(), b"");
        let a = open(&posix, &root, rel!("a"), OpenHow::dir()).unwrap();
        let file = open(&posix, &a, rel!("b/c.txt"), OpenHow::file()).unwrap();
        assert_eq!(read(&posix, &file, 0, 64).unwrap(), b"contents");
    }

    #[test]
    fn reports_open_errors_with_os_codes() {
        let (_temp, posix, root) = fixture();
        let error = open(&posix, &root, rel!("a/nope"), OpenHow::file()).unwrap_err();
        assert_eq!(
            (error.kind(), error.path()),
            (ErrorKind::NotFound, Some(rel!("a/nope")))
        );
        assert!(error.os().is_some());
        let kind = |path, how| open(&posix, &root, path, how).unwrap_err().kind();
        assert_eq!(kind(rel!("a"), OpenHow::file()), ErrorKind::IsADirectory);
        assert_eq!(
            kind(rel!("a/z.txt"), OpenHow::dir()),
            ErrorKind::NotADirectory
        );
        assert_eq!(
            kind(rel!("a/z.txt/x"), OpenHow::file()),
            ErrorKind::NotADirectory
        );
    }

    #[test]
    fn confines_symlinks() {
        let (_temp, posix, root) = fixture();
        let contents = |at: &Handle, path, symlinks| {
            read(&posix, &open(&posix, at, path, file_how(symlinks))?, 0, 64)
        };
        let kind = |at, path, symlinks| contents(at, path, symlinks).unwrap_err().kind();
        let a = open(&posix, &root, rel!("a"), OpenHow::dir()).unwrap();

        assert_eq!(
            contents(&root, rel!("a/link"), Symlinks::Beneath).unwrap(),
            b"contents"
        );
        assert_eq!(
            contents(&root, rel!("a/up"), Symlinks::Beneath).unwrap(),
            b"contents"
        );
        assert_eq!(
            kind(&a, rel!("up"), Symlinks::Beneath),
            ErrorKind::EscapeBlocked
        );
        // The real `/a/b/c.txt` does not exist; `InRoot` finds ours.
        assert_eq!(
            kind(&root, rel!("a/abs"), Symlinks::Beneath),
            ErrorKind::EscapeBlocked
        );
        assert_eq!(
            contents(&root, rel!("a/abs"), Symlinks::InRoot).unwrap(),
            b"contents"
        );
        // `outside.txt` exists beside the root, but is out of reach.
        assert_eq!(
            kind(&root, rel!("a/escape"), Symlinks::Beneath),
            ErrorKind::EscapeBlocked
        );
        assert_eq!(
            kind(&root, rel!("a/escape"), Symlinks::InRoot),
            ErrorKind::NotFound
        );
        assert_eq!(
            kind(&root, rel!("a/link"), Symlinks::Deny),
            ErrorKind::SymlinkLoop
        );
    }

    #[test]
    fn stats_handles_and_entries() {
        let (_temp, posix, root) = fixture();
        let a = open(&posix, &root, rel!("a"), OpenHow::dir()).unwrap();
        let dir = stat(&posix, Target::Handle(a.clone())).unwrap();
        assert_eq!(dir.kind, Some(FileKind::Dir));
        assert!(dir.modified.is_some() && dir.id.is_some());
        let entry = |name: &str, follow| Target::Entry {
            at: a.clone(),
            name: EntryName::Utf8(name.parse().unwrap()),
            follow,
            resolve: Resolve::default(),
        };
        let link = stat(&posix, entry("link", false)).unwrap();
        assert_eq!((link.kind, link.len), (Some(FileKind::Symlink), Some(7)));
        let file = stat(&posix, entry("link", true)).unwrap();
        assert_eq!((file.kind, file.len), (Some(FileKind::File), Some(8)));
        let opened = open(&posix, &root, rel!("a/b/c.txt"), OpenHow::file()).unwrap();
        assert_eq!(stat(&posix, Target::Handle(opened)).unwrap().id, file.id);
        let error = stat(&posix, entry("nope", false)).unwrap_err();
        assert_eq!(
            (error.kind(), error.path()),
            (ErrorKind::NotFound, Some(rel!("nope")))
        );
    }

    #[test]
    fn lists_directories_in_batches() {
        let (temp, posix, root) = fixture();
        let many = temp.0.join("top/many");
        fs::create_dir(&many).unwrap();
        for i in 0..10 {
            fs::write(many.join(std::format!("f{i}")), "").unwrap();
        }
        let dir = open(&posix, &root, rel!("many"), OpenHow::dir()).unwrap();
        let expected: Vec<_> = (0..10)
            .map(|i| (std::format!("f{i}"), Some(FileKind::File)))
            .collect();
        assert_eq!(list(&posix, &dir, 100), expected);
        assert_eq!(list(&posix, &dir, 3), expected);
        assert_eq!(list(&posix, &dir, 1), expected);
        let a = open(&posix, &root, rel!("a"), OpenHow::dir()).unwrap();
        let names: Vec<String> = list(&posix, &a, 2).into_iter().map(|(n, _)| n).collect();
        assert_eq!(names, ["abs", "b", "escape", "link", "up", "z.txt"]);
    }

    #[test]
    fn restarts_listings_from_any_cookie() {
        let (_temp, posix, root) = fixture();
        let a = open(&posix, &root, rel!("a"), OpenHow::dir()).unwrap();
        let batch = |cookie| {
            let completion = block_on(posix.submit(Op::ReadDir {
                dir: a.clone(),
                cookie,
                max: 2,
            }));
            match completion.result {
                Ok(Output::Entries { entries, next }) => (entries.len(), next),
                result => panic!("{result:?}"),
            }
        };
        let (_, second) = batch(None);
        let (n, _) = batch(second);
        // Rereading the second batch restarts the stream and skips to it.
        assert_eq!(batch(second).0, n);
        assert_eq!(batch(Some(DirCookie::new(1000))), (0, None));
    }

    #[test]
    fn reads_links() {
        let (_temp, posix, root) = fixture();
        let a = open(&posix, &root, rel!("a"), OpenHow::dir()).unwrap();
        let read_link = |name| {
            block_on(posix.submit(Op::ReadLink {
                at: a.clone(),
                name: EntryName::from(name),
            }))
            .result
        };
        let Ok(Output::LinkTarget(target)) = read_link(name!("abs")) else {
            panic!("read_link failed");
        };
        assert_eq!(target.as_str(), Some("/a/b/c.txt"));
        assert_eq!(
            read_link(name!("z.txt")).unwrap_err().kind(),
            ErrorKind::NotASymlink
        );
    }

    #[test]
    fn refuses_special_files() {
        let (temp, posix, root) = fixture();
        let fifo = temp.0.join("top/fifo");
        let status = process::Command::new("mkfifo").arg(&fifo).status().unwrap();
        assert!(status.success());
        let error = open(&posix, &root, rel!("fifo"), OpenHow::file()).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::NotRegularFile);
    }

    #[test]
    fn rejects_handles_from_other_drivers() {
        let (_temp, _posix, root) = fixture();
        let other = Posix::blocking();
        let error = open(&other, &root, rel!("a"), OpenHow::dir()).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::CrossDriver);
    }

    #[test]
    fn labels_ambient_errors() {
        let posix = Posix::blocking();
        let error = posix
            .open_ambient_dir("/nonexistent/filer", ambient_authority())
            .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::NotFound);
        assert_eq!(error.label(), Some("/nonexistent/filer"));
    }

    /// Linux filesystems accept any bytes in names; APFS rejects invalid
    /// UTF-8, so this runs on Linux only.
    #[cfg(target_os = "linux")]
    #[test]
    fn lists_and_stats_foreign_names() {
        use std::{ffi::OsStr, os::unix::ffi::OsStrExt};
        let (temp, posix, root) = fixture();
        let raw = b"caf\xE9";
        fs::write(temp.0.join("top").join(OsStr::from_bytes(raw)), "x").unwrap();
        let completion = block_on(posix.submit(Op::ReadDir {
            dir: root.clone(),
            cookie: None,
            max: 100,
        }));
        let Ok(Output::Entries { entries, .. }) = completion.result else {
            panic!("read_dir failed");
        };
        let foreign = entries.into_iter().find(|e| e.name.is_foreign()).unwrap();
        assert_eq!(foreign.name.to_string(), "caf\\xE9");
        let metadata = stat(
            &posix,
            Target::Entry {
                at: root,
                name: foreign.name,
                follow: false,
                resolve: Resolve::default(),
            },
        )
        .unwrap();
        assert_eq!(metadata.len, Some(1));
    }
}

#[cfg(test)]
mod frontend_tests {
    use super::*;
    use core::task::{Context, Poll, Waker};
    use filer_api::{MetaMask, ambient_authority};
    use std::{fs, process, string::ToString};

    fn block_on<F: Future>(future: F) -> F::Output {
        let mut future = core::pin::pin!(future);
        let mut cx = Context::from_waker(Waker::noop());
        loop {
            if let Poll::Ready(output) = future.as_mut().poll(&mut cx) {
                return output;
            }
        }
    }

    #[test]
    fn serves_the_frontend() {
        let path =
            std::env::temp_dir().join(std::format!("filer-posix-frontend-{}", process::id()));
        fs::create_dir_all(path.join("sub")).unwrap();
        fs::write(path.join("sub/file.txt"), "hello").unwrap();
        let root = Posix::blocking()
            .open_ambient(&path, ambient_authority())
            .unwrap();
        let file = block_on(root.open_file("sub/file.txt")).unwrap();
        assert_eq!(block_on(file.read_to_end()).unwrap(), b"hello");
        let metadata = block_on(root.metadata("sub", MetaMask::KIND)).unwrap();
        assert_eq!(metadata.kind, Some(FileKind::Dir));
        let error = block_on(root.open_file("nope")).unwrap_err();
        let label = path.to_string_lossy().to_string();
        assert_eq!(error.label(), Some(label.as_str()));
        fs::remove_dir_all(&path).unwrap();
    }
}

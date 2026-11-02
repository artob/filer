// This is free and unencumbered software released into the public domain.

use crate::{
    error,
    node::{self, DirPayload, FilePayload, Node},
    sys,
};
use core::future::Future;
use filer_api::{
    AmbientAuthority, BlockingResolver, Completion, Dir, Driver, DriverId, DriverKey, EntryName,
    Error, ErrorKind, FileKind, Handle, HandleKind, LinkTarget, Op, OpKind, OpenHow, Output,
    Spawner, path::RelPath, run_on,
};
use filer_fs::{reparse::Reparse, wtf8};
use std::{
    fs::{File, OpenOptions},
    os::windows::fs::OpenOptionsExt,
    path::Path,
    sync::Arc,
    vec::Vec,
};
use windows_sys::{
    Wdk::Storage::FileSystem::{
        FILE_DIRECTORY_FILE, FILE_NON_DIRECTORY_FILE, FILE_OPEN_REPARSE_POINT,
    },
    Win32::Storage::FileSystem::{
        FILE_FLAG_BACKUP_SEMANTICS, FILE_LIST_DIRECTORY, FILE_READ_ATTRIBUTES, FILE_READ_DATA,
        FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_TRAVERSE, SYNCHRONIZE,
    },
};

/// The access a directory handle holds: to list it, to open entries
/// relative to it, and to read its attributes.
pub(crate) const DIR_ACCESS: u32 =
    FILE_LIST_DIRECTORY | FILE_TRAVERSE | FILE_READ_ATTRIBUTES | SYNCHRONIZE;

/// The access a file handle holds: to read its data and attributes.
const FILE_ACCESS: u32 = FILE_READ_DATA | FILE_READ_ATTRIBUTES | SYNCHRONIZE;

/// The access entries are looked up with: to read their attributes, which
/// needs no permission on the file itself.
const LOOKUP_ACCESS: u32 = FILE_READ_ATTRIBUTES | SYNCHRONIZE;

/// Every share mode, so that a Filer handle never stops another program
/// from reading, writing, renaming, or deleting a file.
pub(crate) const SHARE_ALL: u32 = FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE;

/// The Windows driver, over [`windows-sys`](https://docs.rs/windows-sys).
///
/// Roots are opened with `CreateFileW` and `FILE_FLAG_BACKUP_SEMANTICS`,
/// through the standard library, which adds the `\\?\` prefix that long
/// paths need. Every handle is opened with all share modes, including
/// `FILE_SHARE_DELETE`, so that it never stops another program from
/// renaming or deleting a file.
///
/// Windows cannot confine resolution itself, so paths resolve through the
/// generic [`BlockingResolver`], one component at a time: each entry is
/// opened with `NtCreateFile` relative to its directory's handle, with
/// `FILE_OPEN_REPARSE_POINT`, so that links are opened rather than
/// followed, and names match without regard to case, as in Win32.
/// Symbolic links, junctions, and WSL symlinks are read and followed by
/// the resolver under the policy, their targets read as `filer-ntfs`
/// reads them ([`filer_fs::reparse`]): `/`-separated, with an absolute
/// path on a drive taken from the root. Directories and files are then
/// reopened from the entry's handle, never by name again, so resolution
/// stays beneath the handle even if entries are renamed concurrently.
/// Names holding `\` or `:`, which Windows would read as another
/// component or a stream, are
/// [`InvalidNameForTarget`](ErrorKind::InvalidNameForTarget).
///
/// Other name surrogates are [`FileKind::Other`], and opening one as a file
/// is [`NotRegularFile`](ErrorKind::NotRegularFile). Other reparse points,
/// such as cloud placeholders and compressed files, are the files or
/// directories they stand for, opened through the filters that own them.
///
/// Windows has no `EINTR`: synchronous I/O is interrupted only when
/// another thread cancels it, which surfaces as
/// [`ErrorKind::Interrupted`].
///
/// # Examples
///
/// ```
/// use filer_api::ambient_authority;
/// use filer_windows::Windows;
///
/// let windows = Windows::blocking();
/// let tmp = windows.open_ambient(std::env::temp_dir(), ambient_authority())?;
/// # Ok::<(), filer_api::Error>(())
/// ```
#[derive(Clone)]
pub struct Windows {
    key: Arc<DriverKey>,
    /// Where operations run: on a spawner's threads, or, if `None`, inside
    /// `poll`.
    spawner: Option<Arc<dyn Spawner>>,
}

impl Windows {
    /// Returns a driver that performs each operation synchronously when its
    /// future is first polled, **blocking the calling thread** until the
    /// system calls return.
    ///
    /// That suits command-line tools, tests, and single-threaded programs.
    /// On a multi-threaded async runtime, a slow call (a cold disk, a hung
    /// network share) stalls every task sharing the thread; use
    /// [`thread_pool`](Self::thread_pool) there.
    #[must_use]
    pub fn blocking() -> Self {
        Self {
            key: Arc::new(DriverKey::new()),
            spawner: None,
        }
    }

    /// Returns a driver that performs each operation as a job on
    /// `spawner`, such as a [`DefaultPool`](filer_api::DefaultPool), so
    /// that the system calls block a worker thread rather than the async
    /// executor.
    ///
    /// The job is submitted when the operation's future is first polled.
    /// Once submitted, it runs to completion even if the future is
    /// dropped. If the job panics, the operation fails with
    /// [`ErrorKind::Other`].
    ///
    /// ```
    /// use filer_api::DefaultPool;
    /// use filer_windows::Windows;
    ///
    /// let windows = Windows::thread_pool(DefaultPool::new());
    /// ```
    #[must_use]
    pub fn thread_pool(spawner: impl Spawner) -> Self {
        Self {
            spawner: Some(Arc::new(spawner)),
            ..Self::blocking()
        }
    }

    /// Opens the directory at the native `path` as a root for this driver.
    ///
    /// This is the one place native paths enter, so it requires the
    /// [`AmbientAuthority`] token. The handle's error label is `path`.
    ///
    /// # Errors
    ///
    /// Any error opening `path`, and
    /// [`NotADirectory`](ErrorKind::NotADirectory) if it is not a
    /// directory.
    pub fn open_ambient_dir(
        &self,
        path: impl AsRef<Path>,
        authority: AmbientAuthority,
    ) -> Result<Handle, Error> {
        let _ = authority;
        let path = path.as_ref();
        let op = OpKind::Open;
        let label = |error: Error| error.with_label(&path.to_string_lossy());
        let file = OpenOptions::new()
            .access_mode(DIR_ACCESS)
            .share_mode(SHARE_ALL)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
            .open(path)
            .map_err(|e| label(error::io(&e, op)))?;
        let metadata = file.metadata().map_err(|e| label(error::io(&e, op)))?;
        if !metadata.is_dir() {
            return Err(label(Error::new(ErrorKind::NotADirectory, op)));
        }
        Ok(self.dir_handle(Arc::new(file)))
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

    fn dir_handle(&self, file: Arc<File>) -> Handle {
        self.key.handle(HandleKind::Dir, DirPayload::new(file))
    }

    /// Performs `op` synchronously.
    fn perform(&self, op: Op) -> Completion {
        let kind = op.kind();
        match op {
            Op::Open { at, path, how } => {
                Completion::new(self.open(&at, &path, how).map_err(|e| e.with_path(&*path)))
            },
            Op::Close { handle } => Completion::new(self.any(&handle, kind).map(|_| Output::Done)),
            Op::ReadLink { at, name } => {
                let result = self.dir(&at, kind).and_then(|dir| {
                    let bytes = self.name_bytes(&name, kind)?;
                    Walker.read_link(&Node::Dir(dir.file.clone()), bytes, kind)
                });
                Completion::new(
                    result
                        .map(Output::LinkTarget)
                        .map_err(|e| e.with_entry_name(&name)),
                )
            },
            op => op.unsupported(),
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

    /// Returns a file handle's file.
    fn file<'h>(&self, handle: &'h Handle, op: OpKind) -> Result<&'h File, Error> {
        match self.key.payload::<FilePayload>(handle) {
            Some(file) => Ok(&file.file),
            None if self.key.payload::<DirPayload>(handle).is_some() => {
                Err(Error::new(ErrorKind::IsADirectory, op))
            },
            None => Err(Error::new(ErrorKind::CrossDriver, op)),
        }
    }

    /// Returns any handle's file.
    fn any<'h>(&self, handle: &'h Handle, op: OpKind) -> Result<&'h File, Error> {
        match self.key.payload::<DirPayload>(handle) {
            Some(dir) => Ok(&dir.file),
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
        let start = Node::Dir(self.dir(at, op)?.file.clone());
        let node = Walker.resolve(&start, path.as_str().as_bytes(), how.resolve, true, op)?;
        match (node, how.kind) {
            (Node::Dir(file), HandleKind::Dir) => Ok(Output::Opened(self.dir_handle(file))),
            (Node::Dir(_), HandleKind::File) => Err(Error::new(ErrorKind::IsADirectory, op)),
            (Node::Entry { .. }, HandleKind::Dir) => Err(Error::new(ErrorKind::NotADirectory, op)),
            (
                Node::Entry {
                    file,
                    kind: FileKind::File,
                },
                HandleKind::File,
            ) => {
                // Without `FILE_OPEN_REPARSE_POINT`, so that a file a filter
                // owns, such as a compressed one, reads through it.
                let file = sys::reopen(&file, FILE_ACCESS, FILE_NON_DIRECTORY_FILE)
                    .map_err(|status| error::nt(status, op))?;
                Ok(Output::Opened(
                    self.key.handle(HandleKind::File, FilePayload { file }),
                ))
            },
            (Node::Entry { .. }, HandleKind::File) => {
                Err(Error::new(ErrorKind::NotRegularFile, op))
            },
            _ => Err(Error::new(ErrorKind::Unsupported, op)),
        }
    }
}

impl Driver for Windows {
    fn id(&self) -> DriverId {
        self.key.id()
    }

    fn submit(&self, op: Op) -> impl Future<Output = Completion> + Send + use<> {
        let windows = self.clone();
        async move {
            let Some(spawner) = windows.spawner.clone() else {
                return windows.perform(op);
            };
            let kind = op.kind();
            match run_on(spawner, move || windows.perform(op)).await {
                Some(completion) => completion,
                // The job panicked; its buffer, if any, is lost with it.
                None => Completion::new(Err(Error::new(ErrorKind::Other, kind))),
            }
        }
    }
}

impl core::fmt::Debug for Windows {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Windows")
            .field("id", &self.key.id())
            .field("thread_pool", &self.spawner.is_some())
            .finish()
    }
}

/// The [`BlockingResolver`] primitives over `NtCreateFile`. A private
/// type, so that callers cannot resolve from handles they were never
/// handed.
pub(crate) struct Walker;

impl Walker {
    /// Opens the entry `name` of `dir` to read its attributes, without
    /// following it if it is a reparse point.
    fn open_entry(dir: &File, name: &[u8], op: OpKind) -> Result<File, Error> {
        let units = units(name, op)?;
        sys::open_relative(dir, &units, LOOKUP_ACCESS, FILE_OPEN_REPARSE_POINT)
            .map_err(|status| error::nt(status, op))
    }
}

impl BlockingResolver for Walker {
    type Node = Node;

    fn lookup(&self, dir: &Node, name: &[u8], op: OpKind) -> Result<Node, Error> {
        let Node::Dir(dir) = dir else {
            return Err(Error::new(ErrorKind::NotADirectory, op));
        };
        let file = Self::open_entry(dir, name, op)?;
        let (attributes, tag) = sys::attribute_tag(&file).map_err(|code| error::win32(code, op))?;
        match node::kind(attributes, tag) {
            FileKind::Dir => {
                let dir = sys::reopen(&file, DIR_ACCESS, FILE_DIRECTORY_FILE)
                    .map_err(|status| error::nt(status, op))?;
                Ok(Node::Dir(Arc::new(dir)))
            },
            kind => Ok(Node::Entry {
                file: Arc::new(file),
                kind,
            }),
        }
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
        link_target(&Self::open_entry(dir, name, op)?, op)
    }

    fn crosses_mount(&self, dir: &Node, child: &Node) -> bool {
        match (sys::id(dir.file()), sys::id(child.file())) {
            (Ok((a, _)), Ok((b, _))) => a != b,
            // If either cannot be read, the next step reports the error.
            _ => false,
        }
    }
}

/// Returns the UTF-16 of an entry name, which is UTF-8 or, for names this
/// driver listed, WTF-8. Names holding `\` or `:` are refused: Windows
/// would read them as more than one component, or as a stream.
fn units(name: &[u8], op: OpKind) -> Result<Vec<u16>, Error> {
    let invalid = || Error::new(ErrorKind::InvalidNameForTarget, op);
    if name.iter().any(|&byte| matches!(byte, b'\\' | b':')) {
        return Err(invalid());
    }
    wtf8::decode(name).ok_or_else(invalid)
}

/// Returns the target of the link `file` is open on.
fn link_target(file: &File, op: OpKind) -> Result<LinkTarget, Error> {
    let data = sys::reparse_point(file).map_err(|code| error::win32(code, op))?;
    match Reparse::parse(&data) {
        Some(Reparse::Link(target)) => Ok(LinkTarget::from_bytes(target.into_vec())),
        Some(Reparse::Other(_)) => Err(Error::new(ErrorKind::NotASymlink, op)),
        None => Err(Error::new(ErrorKind::Corrupt, op)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::task::{Context, Poll, Waker};
    use filer_api::{
        Resolve, Symlinks, ambient_authority,
        path::{NameBuf, RelPathBuf},
    };
    use std::{
        fs,
        os::windows::fs::{symlink_dir, symlink_file},
        path::PathBuf,
        process::{self, Command, Stdio},
        string::ToString,
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
            let path =
                std::env::temp_dir().join(std::format!("filer-windows-{}-{n}", process::id()));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// Makes a junction at `link` to the directory `target`.
    fn junction(link: &Path, target: &Path) {
        let status = Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .stdout(Stdio::null())
            .status()
            .unwrap();
        assert!(status.success());
    }

    /// A fixture: `a/b/c.txt` and `a/z.txt` under `top`, with
    /// `outside.txt` beside `top`; in `a`, symbolic links to files and to a
    /// directory, and junctions to `a/b` and to `top`'s parent.
    fn fixture() -> (TempDir, Windows, Handle) {
        let temp = TempDir::new();
        let top = temp.0.join("top");
        fs::create_dir_all(top.join("a/b")).unwrap();
        fs::write(top.join("a/b/c.txt"), "contents").unwrap();
        fs::write(top.join("a/z.txt"), "").unwrap();
        fs::write(temp.0.join("outside.txt"), "outside").unwrap();
        for (link, target) in [
            ("link", "b/c.txt"),
            ("backslash", r"b\c.txt"),
            ("up", "../a/b/c.txt"),
            ("abs", "/a/b/c.txt"),
            ("escape", "../../outside.txt"),
            ("loop", "loop"),
        ] {
            symlink_file(target, top.join("a").join(link)).unwrap();
        }
        symlink_dir("b", top.join("a/dirlink")).unwrap();
        junction(&top.join(r"a\junction"), &top.join(r"a\b"));
        junction(&top.join(r"a\out"), &temp.0);
        let windows = Windows::blocking();
        let root = windows.open_ambient_dir(&top, ambient_authority()).unwrap();
        (temp, windows, root)
    }

    fn open(windows: &Windows, at: &Handle, path: &str, how: OpenHow) -> Result<Handle, Error> {
        let completion = block_on(windows.submit(Op::Open {
            at: at.clone(),
            path: RelPathBuf::try_from(path).unwrap(),
            how,
        }));
        match completion.result? {
            Output::Opened(handle) => Ok(handle),
            output => panic!("unexpected {output:?}"),
        }
    }

    fn read_link(windows: &Windows, at: &Handle, name: &str) -> Result<LinkTarget, Error> {
        let completion = block_on(windows.submit(Op::ReadLink {
            at: at.clone(),
            name: EntryName::Utf8(NameBuf::try_from(name).unwrap()),
        }));
        match completion.result? {
            Output::LinkTarget(target) => Ok(target),
            output => panic!("unexpected {output:?}"),
        }
    }

    #[test]
    fn reports_open_errors_with_os_codes() {
        let (_temp, windows, root) = fixture();
        let error = open(&windows, &root, "a/nope", OpenHow::file()).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::NotFound);
        assert_eq!(error.path().map(|p| p.as_str()), Some("a/nope"));
        assert!(error.os().is_some());
        let kind = |path, how| open(&windows, &root, path, how).unwrap_err().kind();
        assert_eq!(kind("a", OpenHow::file()), ErrorKind::IsADirectory);
        assert_eq!(kind("a/z.txt", OpenHow::dir()), ErrorKind::NotADirectory);
        assert_eq!(kind("a/z.txt/x", OpenHow::file()), ErrorKind::NotADirectory);
        // Another component, a stream, and a wildcard.
        for path in [r"a\b\c.txt", "a/z.txt:stream", "a/*"] {
            assert_eq!(
                kind(path, OpenHow::file()),
                ErrorKind::InvalidNameForTarget,
                "{path}"
            );
        }
    }

    #[test]
    fn opens_relative_to_directories() {
        let (_temp, windows, root) = fixture();
        let a = open(&windows, &root, "a", OpenHow::dir()).unwrap();
        open(&windows, &a, "b/c.txt", OpenHow::file()).unwrap();
        let again = open(&windows, &a, "", OpenHow::dir()).unwrap();
        open(&windows, &again, "z.txt", OpenHow::file()).unwrap();
        // Names match without regard to case, as in Win32.
        open(&windows, &root, "A/B/C.TXT", OpenHow::file()).unwrap();
        let file = open(&windows, &root, "a/z.txt", OpenHow::file()).unwrap();
        let error = open(&windows, &file, "x", OpenHow::file()).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::NotADirectory);
        let completion = block_on(windows.submit(Op::Close { handle: file }));
        assert!(matches!(completion.result, Ok(Output::Done)));
    }

    #[test]
    fn follows_links_beneath() {
        let (_temp, windows, root) = fixture();
        for path in ["a/link", "a/backslash", "a/up", "a/dirlink/c.txt"] {
            open(&windows, &root, path, OpenHow::file()).unwrap();
        }
        open(&windows, &root, "a/dirlink", OpenHow::dir()).unwrap();
        let a = open(&windows, &root, "a", OpenHow::dir()).unwrap();
        let kind = |at, path| {
            open(&windows, at, path, OpenHow::file())
                .unwrap_err()
                .kind()
        };
        // `up` leaves `a` before re-entering it.
        assert_eq!(kind(&a, "up"), ErrorKind::EscapeBlocked);
        // Junctions hold absolute paths, so neither stays beneath.
        for path in ["a/abs", "a/escape", "a/junction/c.txt", "a/out/outside.txt"] {
            assert_eq!(kind(&root, path), ErrorKind::EscapeBlocked, "{path}");
        }
        assert_eq!(kind(&root, "a/loop"), ErrorKind::SymlinkLoop);
    }

    #[test]
    fn follows_links_from_the_root_or_not_at_all() {
        let (_temp, windows, root) = fixture();
        let how = |symlinks| {
            let mut how = OpenHow::file();
            how.resolve = Resolve::default().with_symlinks(symlinks);
            how
        };
        open(&windows, &root, "a/abs", how(Symlinks::InRoot)).unwrap();
        // A junction's drive path is taken from the root, which has no
        // such path.
        let error = open(&windows, &root, "a/junction/c.txt", how(Symlinks::InRoot)).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::NotFound);
        let error = open(&windows, &root, "a/link", how(Symlinks::Deny)).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::SymlinkLoop);
    }

    #[test]
    fn reads_links_and_junctions() {
        let (temp, windows, root) = fixture();
        let a = open(&windows, &root, "a", OpenHow::dir()).unwrap();
        let target = |name| read_link(&windows, &a, name).unwrap();
        assert_eq!(target("link").as_str(), Some("b/c.txt"));
        assert_eq!(target("backslash").as_str(), Some("b/c.txt"));
        assert_eq!(target("abs").as_str(), Some("/a/b/c.txt"));
        assert_eq!(target("dirlink").as_str(), Some("b"));
        // `C:\...\top\a\b`, from the drive's root.
        let junction = target("junction");
        let expected = temp.0.join(r"top\a\b");
        let expected = expected.to_string_lossy().replace('\\', "/");
        assert_eq!(junction.as_str(), expected.get(2..));
        let error = read_link(&windows, &a, "z.txt").unwrap_err();
        assert_eq!(error.kind(), ErrorKind::NotASymlink);
        let error = read_link(&windows, &a, "nope").unwrap_err();
        assert_eq!(error.kind(), ErrorKind::NotFound);
    }

    #[test]
    fn opens_and_closes_roots() {
        for windows in [
            Windows::blocking(),
            Windows::thread_pool(filer_api::DefaultPool::new()),
        ] {
            let root = windows
                .open_ambient_dir(std::env::temp_dir(), ambient_authority())
                .unwrap();
            let completion = block_on(windows.submit(Op::Close { handle: root }));
            assert!(matches!(completion.result, Ok(Output::Done)));
            let other = Windows::blocking()
                .open_ambient_dir(std::env::temp_dir(), ambient_authority())
                .unwrap();
            let completion = block_on(windows.submit(Op::Close { handle: other }));
            let error = completion.result.unwrap_err();
            assert_eq!(error.kind(), ErrorKind::CrossDriver);
        }
    }

    #[test]
    fn labels_ambient_errors() {
        let windows = Windows::blocking();
        let error = windows
            .open_ambient_dir(r"C:\nonexistent\filer", ambient_authority())
            .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::NotFound);
        assert_eq!(error.label(), Some(r"C:\nonexistent\filer"));
        assert!(error.os().is_some());
    }

    #[test]
    fn refuses_files_as_roots() {
        let path = std::env::temp_dir().join(std::format!("filer-windows-{}", process::id()));
        fs::write(&path, "").unwrap();
        let error = Windows::blocking()
            .open_ambient_dir(&path, ambient_authority())
            .unwrap_err();
        fs::remove_file(&path).unwrap();
        assert_eq!(error.kind(), ErrorKind::NotADirectory);
        let label = path.to_string_lossy().to_string();
        assert_eq!(error.label(), Some(label.as_str()));
    }

    /// A root's path may be longer than `MAX_PATH`.
    #[test]
    fn opens_long_paths() {
        let base = std::env::temp_dir().join(std::format!("filer-windows-long-{}", process::id()));
        let mut path = base.clone();
        while path.as_os_str().len() < 300 {
            path.push("a".repeat(50));
        }
        fs::create_dir_all(&path).unwrap();
        let opened = Windows::blocking().open_ambient(&path, ambient_authority());
        fs::remove_dir_all(&base).unwrap();
        opened.unwrap();
    }
}

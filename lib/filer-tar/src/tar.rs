// This is free and unencumbered software released into the public domain.

use crate::index::index;
use core::future::Future;
use filer_api::{
    Completion, Dir, Driver, DriverId, Error, Op, ReadAt, Rights,
    archive::{Archive, Limits},
};

/// A read-only driver over a TAR archive.
///
/// [`open`](Self::open) reads the archive's headers once, from any
/// [`ReadAt`] source, and builds an index; afterwards, reading a member
/// reads its bytes directly from the source, since TAR stores them
/// contiguously. The archive's root directory comes back as a [`Dir`]
/// with only [`Rights::READ`]. The serving itself is the shared
/// [`Archive`] driver.
///
/// It reads POSIX ustar headers (including the name prefix), PAX extended
/// headers (`path`, `linkpath`, `size`, `mtime`), and GNU long names and
/// base-256 numbers. Global PAX headers are ignored, GNU sparse members are
/// [`Unsupported`](filer_api::ErrorKind::Unsupported), and, as POSIX
/// requires, other unknown member types are read as regular files.
///
/// The archive is untrusted. Opening it fails, rather than exposing a
/// member, if any member name is absolute, contains `..`, or contains NUL
/// ([`InvalidName`](filer_api::ErrorKind::InvalidName)); if a header is
/// malformed or truncated ([`Corrupt`](filer_api::ErrorKind::Corrupt)); or
/// if a [`Limits`] value is exceeded
/// ([`LimitExceeded`](filer_api::ErrorKind::LimitExceeded)). The error's
/// source is an [`ArchiveError`](filer_api::archive::ArchiveError) naming
/// the offending header's offset. Directories implied by member names are
/// created; of duplicate members, the later wins; a hardlink shares its
/// target's node, and so its file id. Symlinks in the archive resolve
/// within it, confined like any other path.
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
pub struct Tar<S>(Archive<S>);

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
        let archive = Archive::new(tree, source);
        let root = archive.root();
        Ok(Dir::new(Self(archive), root)?.restrict(Rights::READ))
    }
}

impl<S: ReadAt + 'static> Driver for Tar<S> {
    fn id(&self) -> DriverId {
        self.0.id()
    }

    fn submit(&self, op: Op) -> impl Future<Output = Completion> + Send + use<S> {
        self.0.submit(op)
    }
}

impl<S> Clone for Tar<S> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<S> core::fmt::Debug for Tar<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("Tar").field(&self.0).finish()
    }
}

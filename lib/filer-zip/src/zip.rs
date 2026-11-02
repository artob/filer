// This is free and unencumbered software released into the public domain.

use crate::codec::Method;
use crate::index::index;
use core::future::Future;
use filer_api::{
    Completion, Dir, Driver, DriverId, Error, Op, ReadAt, Rights,
    archive::{Archive, Limits},
};

/// A read-only driver over a ZIP archive.
///
/// [`open`](Self::open) reads the central directory once, from any
/// [`ReadAt`] source with random access, and builds an index; afterwards,
/// reading a stored member reads its bytes directly from the source, and
/// reading a compressed one decodes it as a stream (sequential reads cost
/// one pass; seeking backwards decodes again from the start). The
/// archive's root directory comes back as a [`Dir`] with only
/// [`Rights::READ`]. The serving itself is the shared [`Archive`] driver.
///
/// It reads stored members and, with the `deflate` feature, Deflate
/// members, checking a member's CRC-32 when a read reaches its end; ZIP64
/// archives; and archives with a comment. Directories, symlinks, and special files come from Unix
/// modes in the external attributes, and directories also from a trailing
/// `/` or the MS-DOS directory attribute. Modification times come from an
/// NTFS extra field (100 ns), an extended timestamp (1 s), or the DOS
/// fields (2 s, read as UTC since they name no zone), in that order; see
/// [`Timestamp::granularity`](filer_api::Timestamp::granularity). Names
/// are UTF-8 if valid, and foreign names otherwise. Reading a member with
/// another method, or an encrypted one, is
/// [`Unsupported`](filer_api::ErrorKind::Unsupported), as is opening a
/// spanned archive or one with such a symlink, since symlink targets are
/// read at open. Reading a member whose data is malformed, ends early, or
/// fails its CRC is [`Corrupt`](filer_api::ErrorKind::Corrupt); decoding
/// never goes past the size the central directory declares.
///
/// The archive is untrusted. Opening it fails, rather than exposing a
/// member, if any member name is absolute, contains `..`, NUL, or a
/// backslash, or starts with a drive letter
/// ([`InvalidName`](filer_api::ErrorKind::InvalidName)); if a record is
/// malformed or truncated, a member's local header disagrees with its
/// central directory header, or the end record is ambiguous
/// ([`Corrupt`](filer_api::ErrorKind::Corrupt)), since readers that trust
/// one record over another would see different archives; or if a
/// [`Limits`] value is exceeded
/// ([`LimitExceeded`](filer_api::ErrorKind::LimitExceeded)). The error's
/// source is an [`ArchiveError`](filer_api::archive::ArchiveError) naming
/// the offending record's offset. Directories implied by member names are
/// created; of duplicate members, the later in the central directory
/// wins. Symlinks in the archive resolve within it, confined like any
/// other path.
///
/// # Examples
///
/// ```
/// use filer_zip::Zip;
///
/// # filer_spec::block_on(async {
/// let archive: &'static [u8] = include_bytes!("../tests/data/hello.zip");
/// let root = Zip::open(archive).await?;
/// let file = root.open_file("hello/world.txt").await?;
/// assert_eq!(file.read_to_end().await?, b"Hello, world!\n");
/// # Ok::<(), filer_api::Error>(())
/// # }).unwrap();
/// ```
pub struct Zip<S>(Archive<S, Method>);

impl<S: ReadAt + 'static> Zip<S> {
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

impl<S: ReadAt + 'static> Driver for Zip<S> {
    fn id(&self) -> DriverId {
        self.0.id()
    }

    fn submit(&self, op: Op) -> impl Future<Output = Completion> + Send + use<S> {
        self.0.submit(op)
    }
}

impl<S> Clone for Zip<S> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<S> core::fmt::Debug for Zip<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("Zip").field(&self.0).finish()
    }
}

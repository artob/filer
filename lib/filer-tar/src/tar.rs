// This is free and unencumbered software released into the public domain.

use crate::{Compression, index::index, source::Source};
use core::future::Future;
use filer_api::{Completion, Dir, Driver, DriverId, Error, Op, ReadAt, Rights};
use filer_pack::{Archive, Limits};

/// A read-only driver over a TAR archive.
///
/// [`open`](Self::open) reads the archive's headers once, from any
/// [`ReadAt`] source, and builds an index; afterwards, reading a member
/// reads its bytes directly from the source, since TAR stores them
/// contiguously. The archive's root directory comes back as a [`Dir`]
/// with only [`Rights::READ`]. The serving itself is the shared
/// [`Archive`] driver.
///
/// An archive compressed with gzip, zstd, xz, or bzip2, as its first bytes
/// say, is read decoded, with the [`Compression`]'s feature: opening it
/// decodes it once, to learn its size, which must not pass
/// [`Limits::max_total_size`], and again as it indexes; reading members in
/// order decodes on from the last read, while reading back decodes again
/// from the start. Without its feature, it is
/// [`Unsupported`](filer_api::ErrorKind::Unsupported).
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
/// source is an [`ArchiveError`](filer_pack::ArchiveError) naming
/// the offending header's offset. Directories implied by member names are
/// created; of duplicate members, the later wins; a hardlink shares its
/// target's node, and so its file id. Symlinks in the archive resolve
/// within it, confined like any other path.
///
/// # Examples
///
/// ```
/// use filer_tar::{Compression, Tar};
///
/// # filer_spec::block_on(async {
/// let archive: &'static [u8] = include_bytes!("../tests/data/hello.tar");
/// let root = Tar::open(archive).await?;
/// let file = root.open_file("hello/world.txt").await?;
/// assert_eq!(file.read_to_end().await?, b"Hello, world!\n");
///
/// # #[cfg(feature = "gzip")] {
/// // Compressed, read the same way.
/// let archive: &'static [u8] = include_bytes!("../tests/data/hello.tar.gz");
/// let root = Tar::open(archive).await?;
/// assert_eq!(root.driver().compression(), Some(Compression::Gzip));
/// let file = root.open_file("hello/world.txt").await?;
/// assert_eq!(file.read_to_end().await?, b"Hello, world!\n");
/// # }
/// # Ok::<(), filer_api::Error>(())
/// # }).unwrap();
/// ```
pub struct Tar<S> {
    archive: Archive<Source<S>>,
    compression: Option<Compression>,
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
        let source = Source::open(source, &limits).await?;
        let compression = source.compression();
        let tree = index(&source, &limits).await?;
        let archive = Archive::new(tree, source);
        let root = archive.root();
        let tar = Self {
            archive,
            compression,
        };
        Ok(Dir::new(tar, root)?.restrict(Rights::READ))
    }
}

impl<S> Tar<S> {
    /// Returns what the archive is compressed with, if anything.
    #[must_use]
    pub const fn compression(&self) -> Option<Compression> {
        self.compression
    }
}

impl<S: ReadAt + 'static> Driver for Tar<S> {
    fn id(&self) -> DriverId {
        self.archive.id()
    }

    fn submit(&self, op: Op) -> impl Future<Output = Completion> + Send + use<S> {
        self.archive.submit(op)
    }
}

impl<S> Clone for Tar<S> {
    fn clone(&self) -> Self {
        Self {
            archive: self.archive.clone(),
            compression: self.compression,
        }
    }
}

impl<S> core::fmt::Debug for Tar<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Tar")
            .field("archive", &self.archive)
            .field("compression", &self.compression)
            .finish()
    }
}

// This is free and unencumbered software released into the public domain.

use crate::index::index;
use core::future::Future;
use filer_api::{
    Completion, Dir, Driver, DriverId, Error, Op, ReadAt, Rights,
    archive::{Archive, Limits},
};

/// A read-only driver over an ar archive.
///
/// [`open`](Self::open) reads the archive's headers once, from any
/// [`ReadAt`] source, and builds an index; afterwards, reading a member
/// reads its bytes directly from the source, since ar stores them
/// contiguously. The archive's root directory comes back as a [`Dir`]
/// with only [`Rights::READ`]. The serving itself is the shared
/// [`Archive`] driver.
///
/// ar members are regular files, each with a modification time. All three
/// dialects of names are read: System V's and Debian's short names, GNU's
/// and Windows' table of long names (the member `//`), and BSD's names
/// stored before the data (`#1/` and a length). Symbol tables and other
/// members for the linker (GNU's and Windows' `/`, `/SYM64/`, and
/// `/<`...`>/`; BSD's `__.SYMDEF` and its variants) are hidden. A `/`
/// within a name, as `ar P` writes, separates directories, which are then
/// created. GNU thin archives, whose members' data lie outside them, are
/// [`Unsupported`](filer_api::ErrorKind::Unsupported).
///
/// The archive is untrusted. Opening it fails, rather than exposing a
/// member, if any member name is absolute, contains `..`, or contains NUL
/// ([`InvalidName`](filer_api::ErrorKind::InvalidName)); if a header is
/// malformed or truncated, or a long name is missing
/// ([`Corrupt`](filer_api::ErrorKind::Corrupt)); or if a [`Limits`] value
/// is exceeded ([`LimitExceeded`](filer_api::ErrorKind::LimitExceeded):
/// the entry count, the name length, or the name table's size, which
/// [`max_extension_len`](Limits::max_extension_len) bounds). The error's
/// source is an [`ArchiveError`](filer_api::archive::ArchiveError) naming
/// the offending header's offset. Of members with the same name, which
/// static libraries may hold, the later wins, as in every archive format
/// Filer reads.
///
/// # Examples
///
/// A Debian package is an ar archive, whose `data.tar` member
/// [`filer-tar`](https://docs.rs/filer-tar) opens in turn:
///
/// ```
/// use filer_ar::Ar;
/// use filer_tar::Tar;
///
/// # filer_spec::block_on(async {
/// let package: &'static [u8] = include_bytes!("../tests/data/hello.deb");
/// let root = Ar::open(package).await?;
/// let version = root.open_file("debian-binary").await?;
/// assert_eq!(version.read_to_end().await?, b"2.0\n");
/// let data = Tar::open(root.open_file("data.tar").await?).await?;
/// let readme = data.open_file("usr/share/doc/hello/README").await?;
/// assert_eq!(readme.read_to_end().await?, b"Hello, world!\n");
/// # Ok::<(), filer_api::Error>(())
/// # }).unwrap();
/// ```
pub struct Ar<S>(Archive<S>);

impl<S: ReadAt + 'static> Ar<S> {
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

impl<S: ReadAt + 'static> Driver for Ar<S> {
    fn id(&self) -> DriverId {
        self.0.id()
    }

    fn submit(&self, op: Op) -> impl Future<Output = Completion> + Send + use<S> {
        self.0.submit(op)
    }
}

impl<S> Clone for Ar<S> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<S> core::fmt::Debug for Ar<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("Ar").field(&self.0).finish()
    }
}

// This is free and unencumbered software released into the public domain.

use crate::{codec::Sum, index::index};
use core::future::Future;
use filer_api::{Completion, Dir, Driver, DriverId, Error, Op, ReadAt, Rights};
use filer_pack::{Archive, Limits};

/// A read-only driver over a cpio archive.
///
/// [`open`](Self::open) reads the archive's headers once, from any
/// [`ReadAt`] source, and builds an index; afterwards, reading a member
/// reads its bytes directly from the source, since cpio stores them
/// contiguously. The archive's root directory comes back as a [`Dir`]
/// with only [`Rights::READ`]. The serving itself is the shared
/// [`Archive`] driver.
///
/// It reads the new ASCII format (`newc`, as Linux initramfs images and
/// RPM payloads use), with or without checksums, the portable ASCII format
/// (`odc`, which POSIX specifies), and the old binary format in either
/// byte order; directories, regular files, symlinks, and special files,
/// with modification times. Hardlinks, which cpio identifies by device and
/// inode number rather than by path, share one node, and so one file id,
/// whichever link holds the data (`newc` stores it only with the last).
/// A `newc` checksum, which only regular files have, is checked when a
/// read reaches the file's end, failing that read with [`Corrupt`](filer_api::ErrorKind::Corrupt) if
/// the sum differs. Archives concatenated, as in an initramfs image, read
/// as one, with zeros allowed between them; a compressed archive among
/// them is [`Unsupported`](filer_api::ErrorKind::Unsupported), as
/// decompression is not built in.
///
/// The archive is untrusted. Opening it fails, rather than exposing a
/// member, if any member name is absolute, contains `..`, or contains NUL
/// ([`InvalidName`](filer_api::ErrorKind::InvalidName)); if a header is
/// malformed or truncated, or a file type unknown
/// ([`Corrupt`](filer_api::ErrorKind::Corrupt)); or if a [`Limits`] value
/// is exceeded ([`LimitExceeded`](filer_api::ErrorKind::LimitExceeded):
/// the entry count or the name length). The error's source is an
/// [`ArchiveError`](filer_pack::ArchiveError) naming the offending
/// header's offset. Directories implied by member names are created; of
/// duplicate members, the later wins. Symlinks in the archive resolve
/// within it, confined like any other path.
///
/// # Examples
///
/// An initramfs image, as the Linux kernel unpacks it: an early archive
/// for microcode, then the main one.
///
/// ```
/// use filer_cpio::Cpio;
///
/// # filer_spec::block_on(async {
/// let image: &'static [u8] = include_bytes!("../tests/data/initramfs.img");
/// let root = Cpio::open(image).await?;
/// let microcode = root.open_file("kernel/x86/microcode/GenuineIntel.bin").await?;
/// assert_eq!(microcode.read_to_end().await?, b"not really microcode\n");
/// let file = root.open_file("link").await?;
/// assert_eq!(file.read_to_end().await?, b"Hello, world!\n");
/// # Ok::<(), filer_api::Error>(())
/// # }).unwrap();
/// ```
pub struct Cpio<S>(Archive<S, Sum>);

impl<S: ReadAt + 'static> Cpio<S> {
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

impl<S: ReadAt + 'static> Driver for Cpio<S> {
    fn id(&self) -> DriverId {
        self.0.id()
    }

    fn submit(&self, op: Op) -> impl Future<Output = Completion> + Send + use<S> {
        self.0.submit(op)
    }
}

impl<S> Clone for Cpio<S> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<S> core::fmt::Debug for Cpio<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("Cpio").field(&self.0).finish()
    }
}

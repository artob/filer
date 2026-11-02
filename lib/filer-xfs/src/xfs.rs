// This is free and unencumbered software released into the public domain.

use crate::{Options, volume::Volume};
use core::future::Future;
use filer_api::{Completion, Dir, Driver, DriverId, Error, Op, ReadAt, Rights};
use filer_fs::Mounted;

/// A read-only driver for an XFS volume.
///
/// It reads a bare volume's data device, with no partition table, from
/// any [`ReadAt`] source: a disk image [`File`](filer_api::File) of
/// another driver, bytes in memory, or a block device. Opening reads and
/// validates the superblock and the root directory; inodes, directories,
/// and data are read as operations need them.
///
/// Names are bytes, matched exactly, or on an `ASCII_CI` volume folded to
/// lower case as Linux folds them, Latin-1's letters too; those that are
/// not UTF-8 are [`ForeignName`](filer_api::ForeignName)s. Listings are in directory
/// order, without `.` and `..`. A file's id is its inode number, so hard
/// links share it. Times are UTC, at 1 ns. Metadata reports the flags
/// immutable and append-only. Extended attributes are read in the `user`,
/// `trusted`, `security`, and `system` namespaces; POSIX ACLs read as
/// Linux's `getxattr` returns them, as `system` attributes converted and
/// as `trusted` ones as they are. Symlinks are confined like any other;
/// FIFOs, sockets, and devices are what they say, and do not open. Writing
/// is out of scope: everything but reading is
/// [`Unsupported`](filer_api::ErrorKind::Unsupported).
///
/// # Examples
///
/// ```no_run
/// use filer_xfs::Xfs;
///
/// # async fn example(image: Vec<u8>) -> filer_api::Result<()> {
/// let root = Xfs::open(image).await?;
/// let file = root.open_file("etc/hostname").await?;
/// let text = file.read_to_end().await?;
/// # Ok(())
/// # }
/// ```
pub struct Xfs<S>(Mounted<Volume<S>>);

impl<S: ReadAt + 'static> Xfs<S> {
    /// Opens the volume in `source` and returns its root directory, with
    /// [`Rights::READ`]. A volume whose log was not left clean is read as
    /// Linux recovers it, the log replayed in memory, never written.
    ///
    /// # Errors
    ///
    /// As [`open_with`](Self::open_with) with the default [`Options`].
    pub async fn open(source: S) -> Result<Dir<Self>, Error> {
        Self::open_with(source, Options::new()).await
    }

    /// Opens the volume in `source` with `options` and returns its root
    /// directory, with [`Rights::READ`].
    ///
    /// # Errors
    ///
    /// [`Corrupt`](filer_api::ErrorKind::Corrupt), with an
    /// [`XfsError`](crate::XfsError) as its source, if the superblock or
    /// the root directory is invalid, or the source is shorter than the
    /// volume; [`Unsupported`](filer_api::ErrorKind::Unsupported) if the
    /// volume is of an unknown version or uses features this driver does
    /// not read, or, unless `options` allow reading it as it is, if its log
    /// needs work recovery would do that this driver does not, or is on
    /// another device; any error reading the source.
    pub async fn open_with(source: S, options: Options) -> Result<Dir<Self>, Error> {
        let mounted = Mounted::new(Volume::open(source, options).await?);
        let root = mounted.root().await?;
        Ok(Dir::new(Self(mounted), root)?.restrict(Rights::READ))
    }
}

impl<S: ReadAt + 'static> Driver for Xfs<S> {
    fn id(&self) -> DriverId {
        self.0.id()
    }

    fn submit(&self, op: Op) -> impl Future<Output = Completion> + Send + use<S> {
        self.0.submit(op)
    }
}

impl<S> Clone for Xfs<S> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<S: ReadAt + 'static> core::fmt::Debug for Xfs<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let volume = self.0.fs();
        f.debug_struct("Xfs")
            .field("id", &volume.key.id())
            .field("volume", &volume.sb.volume_id())
            .finish_non_exhaustive()
    }
}

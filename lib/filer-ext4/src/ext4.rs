// This is free and unencumbered software released into the public domain.

use crate::{Options, volume::Volume};
use core::future::Future;
use filer_api::{Completion, Dir, Driver, DriverId, Error, Op, ReadAt, Rights};
use filer_fs::Mounted;

/// A read-only driver for an ext2, ext3, or ext4 volume.
///
/// It reads a bare volume, with no partition table, from any [`ReadAt`]
/// source: a disk image [`File`](filer_api::File) of another driver,
/// bytes in memory, or a block device. Opening reads and validates the
/// superblock and the root directory, and replays in memory a journal
/// that needs recovery; inodes, directories, and data are read as
/// operations need them.
///
/// Names are bytes, matched exactly; those that are not UTF-8 are
/// [`ForeignName`](filer_api::ForeignName)s. Listings are in directory
/// order, without `.` and `..`. A file's id is its inode number, so hard
/// links share it. Times are UTC, at 1 ns where the inode has room for
/// the extra fields, else at 1 s, and include the creation time where it
/// is recorded. Metadata reports the flags immutable, append-only,
/// compressed, encrypted, and verity. Extended attributes are read from
/// the inode and its attribute block, in the `user`, `trusted`,
/// `security`, and `system` namespaces; POSIX ACLs read as Linux's
/// `getxattr` returns them. Symlinks are confined like any other; FIFOs,
/// sockets, and devices are what they say, and do not open. Writing is
/// out of scope: everything but reading is
/// [`Unsupported`](filer_api::ErrorKind::Unsupported).
///
/// # Examples
///
/// ```no_run
/// use filer_ext4::Ext4;
///
/// # async fn example(image: Vec<u8>) -> filer_api::Result<()> {
/// let root = Ext4::open(image).await?;
/// let file = root.open_file("etc/hostname").await?;
/// let text = file.read_to_end().await?;
/// # Ok(())
/// # }
/// ```
pub struct Ext4<S>(Mounted<Volume<S>>);

impl<S: ReadAt + 'static> Ext4<S> {
    /// Opens the volume in `source` and returns its root directory, with
    /// [`Rights::READ`], with the default [`Options`].
    ///
    /// # Errors
    ///
    /// As [`open_with`](Self::open_with).
    pub async fn open(source: S) -> Result<Dir<Self>, Error> {
        Self::open_with(source, Options::new()).await
    }

    /// Opens the volume in `source` with `options` and returns its root
    /// directory, with [`Rights::READ`].
    ///
    /// # Errors
    ///
    /// [`Corrupt`](filer_api::ErrorKind::Corrupt), with an
    /// [`Ext4Error`](crate::Ext4Error) as its source, if the superblock,
    /// the root directory, or a journal needing recovery is invalid, or the
    /// source is shorter than the volume;
    /// [`Unsupported`](filer_api::ErrorKind::Unsupported) if the volume is
    /// of an unknown revision, uses incompatible features this driver does
    /// not read, or, unless `options` allow reading it as it is, its
    /// journal needs recovery but is on another device or uses features
    /// this driver does not replay; any error reading the source.
    pub async fn open_with(source: S, options: Options) -> Result<Dir<Self>, Error> {
        let mounted = Mounted::new(Volume::open(source, options).await?);
        let root = mounted.root().await?;
        Ok(Dir::new(Self(mounted), root)?.restrict(Rights::READ))
    }
}

impl<S: ReadAt + 'static> Driver for Ext4<S> {
    fn id(&self) -> DriverId {
        self.0.id()
    }

    fn submit(&self, op: Op) -> impl Future<Output = Completion> + Send + use<S> {
        self.0.submit(op)
    }
}

impl<S> Clone for Ext4<S> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<S: ReadAt + 'static> core::fmt::Debug for Ext4<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let volume = self.0.fs();
        f.debug_struct("Ext4")
            .field("id", &volume.key.id())
            .field("volume", &volume.sb.volume_id())
            .finish_non_exhaustive()
    }
}

// This is free and unencumbered software released into the public domain.

use crate::volume::Volume;
use core::future::Future;
use filer_api::{Completion, Dir, Driver, DriverId, Error, Op, ReadAt, Rights};
use filer_fs::Mounted;

/// A read-only driver for a SquashFS image.
///
/// It reads an image from any [`ReadAt`] source: a file of another driver,
/// such as an AppImage, a snap, or a live CD's root, bytes in memory, or a
/// block device. Opening reads and validates the superblock and the root
/// directory; inodes, directories, and data are read as operations need
/// them, through a small cache of what they decompress.
///
/// Names are bytes, matched exactly; those that are not UTF-8 are
/// [`ForeignName`](filer_api::ForeignName)s. Listings are in the image's
/// order, by name, without `.` and `..`. A file's id is its inode number,
/// so hard links share it. Only the modification time is recorded, at
/// 1 s. Extended attributes are read in the `user`, `trusted`, and
/// `security` namespaces. Symlinks are confined like any other; FIFOs,
/// sockets, and devices are what they say, and do not open. SquashFS is
/// read-only by design: everything but reading is
/// [`Unsupported`](filer_api::ErrorKind::Unsupported).
///
/// # Examples
///
/// ```no_run
/// use filer_squashfs::Squashfs;
///
/// # async fn example(image: Vec<u8>) -> filer_api::Result<()> {
/// let root = Squashfs::open(image).await?;
/// let file = root.open_file("etc/hostname").await?;
/// let text = file.read_to_end().await?;
/// # Ok(())
/// # }
/// ```
pub struct Squashfs<S>(Mounted<Volume<S>>);

impl<S: ReadAt + 'static> Squashfs<S> {
    /// Opens the image in `source` and returns its root directory, with
    /// [`Rights::READ`].
    ///
    /// # Errors
    ///
    /// [`Corrupt`](filer_api::ErrorKind::Corrupt), with a
    /// [`SquashfsError`](crate::SquashfsError) as its source, if the
    /// superblock or the root directory is invalid, or the source is
    /// shorter than the image;
    /// [`Unsupported`](filer_api::ErrorKind::Unsupported) if the image is
    /// of a version other than 4.0, or compressed by a compressor this
    /// driver's features leave out; any error reading the source.
    pub async fn open(source: S) -> Result<Dir<Self>, Error> {
        let mounted = Mounted::new(Volume::open(source).await?);
        let root = mounted.root().await?;
        Ok(Dir::new(Self(mounted), root)?.restrict(Rights::READ))
    }
}

impl<S: ReadAt + 'static> Driver for Squashfs<S> {
    fn id(&self) -> DriverId {
        self.0.id()
    }

    fn submit(&self, op: Op) -> impl Future<Output = Completion> + Send + use<S> {
        self.0.submit(op)
    }
}

impl<S> Clone for Squashfs<S> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<S: ReadAt + 'static> core::fmt::Debug for Squashfs<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let volume = self.0.fs();
        f.debug_struct("Squashfs")
            .field("id", &volume.key.id())
            .field("compressor", &volume.sb.compressor)
            .finish_non_exhaustive()
    }
}

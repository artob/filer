// This is free and unencumbered software released into the public domain.

use crate::volume::Volume;
use core::future::Future;
use filer_api::{Completion, Dir, Driver, DriverId, Error, Op, ReadAt, Rights};
use filer_fs::Mounted;

/// A read-only driver for an ISO 9660 image.
///
/// It reads an image from any [`ReadAt`] source: a file of another driver,
/// such as an installer's `.iso`, bytes in memory, or an optical drive.
/// Opening reads the volume descriptors and the root directory; listings
/// and data are read as operations need them.
///
/// It reads one tree of the image, as the Linux kernel does: Rock Ridge's
/// if the image has it, with POSIX names, kinds, symlinks, and times, and
/// directories relocated for depth shown where they belong; else Joliet's,
/// of UTF-16 names; else ISO 9660:1999's; else the primary one's, whose
/// names show without their version, `;1`. Names are bytes, matched
/// exactly; those that are not UTF-8, as Rock Ridge's may be, or Joliet's
/// of unpaired surrogates, are [`ForeignName`](filer_api::ForeignName)s.
/// Listings are in the image's order, without `.` and `..`, associated
/// files, and older versions of a name. A directory's id is its extent's
/// place; a file's, its Rock Ridge serial number, which hard links share,
/// or its record's place. Times are Rock Ridge's, or else the modification
/// time a record gives, at 1 s. Hidden files are shown, with
/// [`FileAttrs::HIDDEN`](filer_api::FileAttrs::HIDDEN); files zisofs
/// compresses read uncompressed, with
/// [`FileAttrs::COMPRESSED`](filer_api::FileAttrs::COMPRESSED). ISO 9660
/// has no extended attributes Filer reads. Everything but reading is
/// [`Unsupported`](filer_api::ErrorKind::Unsupported).
///
/// # Examples
///
/// ```no_run
/// use filer_cdfs::Cdfs;
///
/// # async fn example(image: Vec<u8>) -> filer_api::Result<()> {
/// let root = Cdfs::open(image).await?;
/// let file = root.open_file("boot/grub/grub.cfg").await?;
/// let text = file.read_to_end().await?;
/// # Ok(())
/// # }
/// ```
pub struct Cdfs<S>(Mounted<Volume<S>>);

impl<S: ReadAt + 'static> Cdfs<S> {
    /// Opens the image in `source` and returns its root directory, with
    /// [`Rights::READ`].
    ///
    /// # Errors
    ///
    /// [`Corrupt`](filer_api::ErrorKind::Corrupt), with a
    /// [`CdfsError`](crate::CdfsError) as its source, if there is no
    /// volume descriptor at sector 16, no primary volume descriptor, or the
    /// root directory is invalid; any error reading the source.
    pub async fn open(source: S) -> Result<Dir<Self>, Error> {
        let mounted = Mounted::new(Volume::open(source).await?);
        let root = mounted.root().await?;
        Ok(Dir::new(Self(mounted), root)?.restrict(Rights::READ))
    }
}

impl<S: ReadAt + 'static> Driver for Cdfs<S> {
    fn id(&self) -> DriverId {
        self.0.id()
    }

    fn submit(&self, op: Op) -> impl Future<Output = Completion> + Send + use<S> {
        self.0.submit(op)
    }
}

impl<S> Clone for Cdfs<S> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<S: ReadAt + 'static> core::fmt::Debug for Cdfs<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let volume = self.0.fs();
        f.debug_struct("Cdfs")
            .field("id", &volume.key.id())
            .field("names", &volume.names)
            .field("rock_ridge", &volume.sharing.is_some())
            .finish_non_exhaustive()
    }
}

// This is free and unencumbered software released into the public domain.

use crate::{Options, geometry, volume::Volume};
use core::future::Future;
use filer_api::{Completion, Dir, Driver, DriverId, DriverKey, Error, Op, ReadAt, Rights};
use filer_fs::Mounted;

/// A read-only driver for a FAT12, FAT16, or FAT32 volume.
///
/// It reads a bare volume, with no partition table, from any [`ReadAt`]
/// source: a disk image [`File`](filer_api::File) of another driver, bytes
/// in memory, or a block device. Opening reads and validates the boot
/// sector; directories and cluster chains are read as operations need
/// them, and a file's chain is read and validated once, when it is opened.
///
/// Names are matched case-insensitively, as FAT preserves case but does
/// not distinguish it, against either an entry's long name or its short
/// (8.3) name. Listings show the long name if there is one. Times are
/// local with no time zone, and are read at the offset from UTC that
/// [`Options`] gives: UTC by default.
///
/// Writing is planned. Until then, everything but reading is
/// [`Unsupported`](filer_api::ErrorKind::Unsupported).
///
/// # Examples
///
/// ```no_run
/// use filer_fat::Fat;
///
/// # async fn example(image: Vec<u8>) -> filer_api::Result<()> {
/// let root = Fat::open(image).await?;
/// let file = root.open_file("README.TXT").await?;
/// let text = file.read_to_end().await?;
/// # Ok(())
/// # }
/// ```
pub struct Fat<S>(Mounted<Volume<S>>);

impl<S: ReadAt + 'static> Fat<S> {
    /// Opens the volume in `source` and returns its root directory, with
    /// [`Rights::READ`], with the default [`Options`].
    ///
    /// # Errors
    ///
    /// [`Corrupt`](filer_api::ErrorKind::Corrupt), with a
    /// [`FatError`](crate::FatError) as its source, if the boot sector is
    /// invalid or the source is shorter than the volume; any error reading
    /// the source.
    pub async fn open(source: S) -> Result<Dir<Self>, Error> {
        Self::open_with(source, Options::new()).await
    }

    /// Opens the volume in `source` as [`open`](Self::open) does, reading
    /// it per `options`.
    ///
    /// # Errors
    ///
    /// As for [`open`](Self::open).
    pub async fn open_with(source: S, options: Options) -> Result<Dir<Self>, Error> {
        let geometry = geometry::read(&source).await?;
        let mounted = Mounted::new(Volume {
            key: DriverKey::new(),
            geometry,
            source,
            options,
        });
        let root = mounted.root().await?;
        Ok(Dir::new(Self(mounted), root)?.restrict(Rights::READ))
    }
}

impl<S: ReadAt + 'static> Driver for Fat<S> {
    fn id(&self) -> DriverId {
        self.0.id()
    }

    fn submit(&self, op: Op) -> impl Future<Output = Completion> + Send + use<S> {
        self.0.submit(op)
    }
}

impl<S> Clone for Fat<S> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<S: ReadAt + 'static> core::fmt::Debug for Fat<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let volume = self.0.fs();
        f.debug_struct("Fat")
            .field("id", &volume.key.id())
            .field("type", &volume.geometry.fat_type())
            .finish_non_exhaustive()
    }
}

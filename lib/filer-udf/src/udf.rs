// This is free and unencumbered software released into the public domain.

use crate::volume::Volume;
use core::future::Future;
use filer_api::{Completion, Dir, Driver, DriverId, Error, Op, ReadAt, Rights};
use filer_fs::Mounted;

/// A read-only driver for a UDF volume.
///
/// It reads a volume from any [`ReadAt`] source: a DVD's or a Blu-ray
/// disc's image, a file of another driver, bytes in memory, or a drive.
/// Opening finds the anchor, at sector 256 or at the volume's end, of a
/// block size from 512 bytes to 4 KiB, and reads the volume descriptors,
/// the partitions, and the root directory's entry; listings and data are
/// read as operations need them.
///
/// Partitions are physical, sparable (of rewritable discs, through their
/// sparing table), virtual (of write-once discs, through their virtual
/// allocation table, at the volume's last block), or of metadata (of
/// UDF 2.50 and later). Names are Unicode, 8- or 16-bit, shown as UTF-8;
/// 16-bit names of unpaired surrogates are
/// [`ForeignName`](filer_api::ForeignName)s. Listings are in the volume's
/// order, without the parent, deleted entries, and repeated names. A
/// file's id is its entry's place, which hard links share. Times are the
/// entry's, at 1 µs: modification, access, and attribute change, and
/// creation in extended entries. Hidden files are shown, with
/// [`FileAttrs::HIDDEN`](filer_api::FileAttrs::HIDDEN). Named streams and
/// extended attributes are not read. Everything but reading is
/// [`Unsupported`](filer_api::ErrorKind::Unsupported).
///
/// # Examples
///
/// ```no_run
/// use filer_udf::Udf;
///
/// # async fn example(image: Vec<u8>) -> filer_api::Result<()> {
/// let root = Udf::open(image).await?;
/// let file = root.open_file("VIDEO_TS/VIDEO_TS.IFO").await?;
/// let info = file.read_to_end().await?;
/// # Ok(())
/// # }
/// ```
pub struct Udf<S>(Mounted<Volume<S>>);

impl<S: ReadAt + 'static> Udf<S> {
    /// Opens the volume in `source` and returns its root directory, with
    /// [`Rights::READ`].
    ///
    /// # Errors
    ///
    /// [`Corrupt`](filer_api::ErrorKind::Corrupt), with a
    /// [`UdfError`](crate::UdfError) as its source, if there is no anchor,
    /// or the descriptors, partitions, file set, or root directory are
    /// invalid; [`Unsupported`](filer_api::ErrorKind::Unsupported) for a
    /// partition map of a kind this driver does not read; any error
    /// reading the source.
    pub async fn open(source: S) -> Result<Dir<Self>, Error> {
        let mounted = Mounted::new(Volume::open(source).await?);
        let root = mounted.root().await?;
        Ok(Dir::new(Self(mounted), root)?.restrict(Rights::READ))
    }
}

impl<S: ReadAt + 'static> Driver for Udf<S> {
    fn id(&self) -> DriverId {
        self.0.id()
    }

    fn submit(&self, op: Op) -> impl Future<Output = Completion> + Send + use<S> {
        self.0.submit(op)
    }
}

impl<S> Clone for Udf<S> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<S: ReadAt + 'static> core::fmt::Debug for Udf<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let volume = self.0.fs();
        f.debug_struct("Udf")
            .field("id", &volume.key.id())
            .field("block", &volume.block)
            .finish_non_exhaustive()
    }
}

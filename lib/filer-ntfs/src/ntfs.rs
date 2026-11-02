// This is free and unencumbered software released into the public domain.

use crate::{Options, volume::Volume};
use core::future::Future;
use filer_api::{Completion, Dir, Driver, DriverId, Error, Op, ReadAt, Rights};
use filer_fs::Mounted;

/// A read-only driver for an NTFS volume.
///
/// It reads a bare volume, with no partition table, from any [`ReadAt`]
/// source: a disk image [`File`](filer_api::File) of another driver,
/// bytes in memory, or a block device. Opening reads and validates the
/// boot sector, the MFT's own record, `$Volume`, and `$UpCase`; files and
/// directories are read as operations need them.
///
/// Names are matched exactly, else case-insensitively by the volume's own
/// `$UpCase` table, as on Windows. Listings show each name of a file once,
/// skipping the DOS (8.3) names that shadow long ones, and hide the
/// system files (`$MFT`, `$LogFile`, and the rest). A file's id is its MFT
/// record number, so hard links share it. Times are UTC, at 100 ns.
///
/// Named data streams (alternate data streams) are extended attributes
/// in the `user` namespace, as ntfs-3g has them: `file:stream` is
/// `user.stream` of `file`, matched as names are. Metadata reports file
/// flags: hidden, system, archive, sparse, compressed, offline,
/// encrypted, and recall on access as dataless.
///
/// Compressed files (LZNT1) read as any other. Symbolic links, junctions,
/// and WSL symlinks are symlinks, confined like any other: a target that
/// is an absolute path on a drive (`C:\\path`) is taken from the root of
/// the volume, assumed to be that drive. Reparse points of other tags,
/// such as cloud placeholders, read as
/// [`Unsupported`](filer_api::ErrorKind::Unsupported). Interix special
/// files, as ntfs-3g makes FIFOs and sockets, and as it and Services for
/// UNIX made symlinks and devices, are what they say; since only a system
/// file's contents tell, listings report no kind for system files.
/// Writing is out of scope: everything but reading is
/// [`Unsupported`](filer_api::ErrorKind::Unsupported), as are encrypted
/// files.
///
/// # Examples
///
/// ```no_run
/// use filer_ntfs::Ntfs;
///
/// # async fn example(image: Vec<u8>) -> filer_api::Result<()> {
/// let root = Ntfs::open(image).await?;
/// let file = root.open_file("Documents/notes.txt").await?;
/// let text = file.read_to_end().await?;
/// # Ok(())
/// # }
/// ```
pub struct Ntfs<S>(Mounted<Volume<S>>);

impl<S: ReadAt + 'static> Ntfs<S> {
    /// Opens the volume in `source` and returns its root directory, with
    /// [`Rights::READ`], with the default [`Options`].
    ///
    /// # Errors
    ///
    /// [`Corrupt`](filer_api::ErrorKind::Corrupt), with an
    /// [`NtfsError`](crate::NtfsError) as its source, if the boot sector,
    /// the MFT, `$Volume`, `$UpCase`, or the root directory is invalid, or
    /// the source is shorter than the volume;
    /// [`Unsupported`](filer_api::ErrorKind::Unsupported) if the volume is
    /// dirty or of an unknown version; any error reading the source.
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
        let mounted = Mounted::new(Volume::open(source, options).await?);
        let root = mounted.root().await?;
        Ok(Dir::new(Self(mounted), root)?.restrict(Rights::READ))
    }
}

impl<S: ReadAt + 'static> Driver for Ntfs<S> {
    fn id(&self) -> DriverId {
        self.0.id()
    }

    fn submit(&self, op: Op) -> impl Future<Output = Completion> + Send + use<S> {
        self.0.submit(op)
    }
}

impl<S> Clone for Ntfs<S> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<S: ReadAt + 'static> core::fmt::Debug for Ntfs<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let volume = self.0.fs();
        f.debug_struct("Ntfs")
            .field("id", &volume.key.id())
            .field("serial", &volume.geometry.serial())
            .finish_non_exhaustive()
    }
}

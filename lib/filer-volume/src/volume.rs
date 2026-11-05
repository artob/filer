// This is free and unencumbered software released into the public domain.

use crate::{VolumeDriver, VolumeFormat};
use alloc::vec::Vec;
use core::future::Future;
use filer_api::{
    BufResult, Completion, Dir, Driver, DriverId, DynDriver, Error, ErrorKind, Op, OpKind, ReadAt,
};
use filer_fs::partition::{self, Window};

/// A read-only driver over a volume of any supported file system, served
/// by the [`VolumeDriver`] of its format.
///
/// [`open`](Self::open) detects the format from the volume's first bytes
/// (see [`VolumeFormat::detect`]), [`open_as`](Self::open_as) takes the
/// driver from the caller, and [`open_partition`](Self::open_partition)
/// opens one partition of a disk image; either way, the format's own
/// driver reads the volume and serves it, so everything its docs say of
/// reading, corruption, and recovery holds here. The volume's root comes
/// back as a [`Dir`] with only [`READ`](filer_api::Rights::READ) rights,
/// of one type whatever the format or the source, as the format's driver
/// is held as a [`DynDriver`]: each operation costs one boxed future.
///
/// # Examples
///
/// ```
/// use filer_volume::{Volume, VolumeDriver, VolumeFormat};
///
/// # #[cfg(feature = "fat")]
/// # filer_spec::block_on(async {
/// let image: &'static [u8] = include_bytes!("../tests/data/hello.img");
/// let root = Volume::open(image).await?;
/// assert_eq!(root.driver().volume_driver(), VolumeDriver::Fat);
/// assert_eq!(root.driver().format(), Some(VolumeFormat::Fat12));
/// let file = root.open_file("hello/world.txt").await?;
/// assert_eq!(file.read_to_end().await?, b"Hello, world!\n");
/// # Ok::<(), filer_api::Error>(())
/// # }).unwrap();
/// ```
#[derive(Clone)]
pub struct Volume {
    inner: DynDriver,
    driver: VolumeDriver,
    format: Option<VolumeFormat>,
}

impl Volume {
    /// Opens the volume in `source`, in the format its first bytes say,
    /// with the default [`VolumeOptions`], and returns its root directory.
    ///
    /// # Errors
    ///
    /// [`Unsupported`](ErrorKind::Unsupported) if `source` holds no known
    /// file system, holds a partitioned disk instead (see
    /// [`open_partition`](Self::open_partition)), or holds a format this
    /// build does not read, with a [`VolumeError`] as its source;
    /// otherwise as the format's driver opens it, or reading `source`
    /// fails.
    pub async fn open<S: ReadAt + 'static>(source: S) -> Result<Dir<Self>, Error> {
        Self::open_with(source, VolumeOptions::default()).await
    }

    /// Opens the volume in `source` as [`open`](Self::open) does, with
    /// `options`.
    ///
    /// # Errors
    ///
    /// As for [`open`](Self::open).
    pub async fn open_with<S: ReadAt + 'static>(
        source: S,
        options: VolumeOptions,
    ) -> Result<Dir<Self>, Error> {
        match detect(&source).await? {
            Some(format) => open(source, format.driver(), Some(format), options).await,
            None if partition::read(&source).await?.is_some() => {
                Err(unsupported(VolumeError::Partitioned))
            },
            None => Err(unsupported(VolumeError::Unknown)),
        }
    }

    /// Opens the volume in `source` with `driver`, with the default
    /// [`VolumeOptions`], and returns its root directory.
    ///
    /// # Errors
    ///
    /// [`Unsupported`](ErrorKind::Unsupported) if this build does not have
    /// `driver`, with a [`VolumeError`] as its source; otherwise as
    /// `driver` opens it.
    pub async fn open_as<S: ReadAt + 'static>(
        source: S,
        driver: VolumeDriver,
    ) -> Result<Dir<Self>, Error> {
        Self::open_as_with(source, driver, VolumeOptions::default()).await
    }

    /// Opens the volume in `source` with `driver` and `options`.
    ///
    /// # Errors
    ///
    /// As for [`open_as`](Self::open_as).
    pub async fn open_as_with<S: ReadAt + 'static>(
        source: S,
        driver: VolumeDriver,
        options: VolumeOptions,
    ) -> Result<Dir<Self>, Error> {
        // The format, if the volume's first bytes name one of the driver's.
        let format = detect(&source)
            .await?
            .filter(|format| format.driver() == driver);
        open(source, driver, format, options).await
    }

    /// Opens partition `number` of the disk image in `source`, numbered as
    /// Linux numbers them (see [`filer_fs::partition`]), as
    /// [`open_with`](Self::open_with) opens a volume.
    ///
    /// # Errors
    ///
    /// [`NotFound`](ErrorKind::NotFound) if `source` has no partition
    /// table, or no such partition, with a [`VolumeError`] as its source;
    /// [`Corrupt`](ErrorKind::Corrupt) if the table is invalid; otherwise
    /// as for [`open_with`](Self::open_with).
    pub async fn open_partition<S: ReadAt + 'static>(
        source: S,
        number: u32,
        options: VolumeOptions,
    ) -> Result<Dir<Self>, Error> {
        let table = partition::read(&source).await?;
        let partition = table
            .as_ref()
            .and_then(|table| table.partition(number))
            .ok_or_else(|| {
                Error::new(ErrorKind::NotFound, OpKind::Open)
                    .with_source(VolumeError::NoPartition(number))
            })?;
        Self::open_with(Window::of(source, partition), options).await
    }

    /// Returns the driver serving the volume.
    #[must_use]
    pub fn volume_driver(&self) -> VolumeDriver {
        self.driver
    }

    /// Returns the volume's format, as its first bytes tell it: `None`
    /// if they name none of its driver's, as may happen to a volume
    /// opened with [`open_as`](Self::open_as).
    #[must_use]
    pub fn format(&self) -> Option<VolumeFormat> {
        self.format
    }
}

/// Opens the volume in `source` with `driver`, as a volume of `format`
/// if its first bytes say so.
async fn open<S: ReadAt + 'static>(
    source: S,
    driver: VolumeDriver,
    format: Option<VolumeFormat>,
    options: VolumeOptions,
) -> Result<Dir<Volume>, Error> {
    match driver {
        #[cfg(feature = "ext4")]
        VolumeDriver::Ext4 => wrap(
            filer_ext4::Ext4::open_with(source, options.ext4).await?,
            driver,
            format,
        ),
        #[cfg(feature = "fat")]
        VolumeDriver::Fat => wrap(
            filer_fat::Fat::open_with(source, options.fat).await?,
            driver,
            format,
        ),
        #[cfg(feature = "ntfs")]
        VolumeDriver::Ntfs => wrap(
            filer_ntfs::Ntfs::open_with(source, options.ntfs).await?,
            driver,
            format,
        ),
        #[cfg(feature = "squashfs")]
        VolumeDriver::Squashfs => wrap(
            filer_squashfs::Squashfs::open(source).await?,
            driver,
            format,
        ),
        #[cfg(feature = "xfs")]
        VolumeDriver::Xfs => wrap(
            filer_xfs::Xfs::open_with(source, options.xfs).await?,
            driver,
            format,
        ),
        #[allow(unreachable_patterns, reason = "with every format enabled")]
        _ => {
            let _ = (source, format, options);
            Err(unsupported(VolumeError::Disabled(driver)))
        },
    }
}

/// Returns the format of the volume in `source`, by its first bytes.
async fn detect<S: ReadAt>(source: &S) -> Result<Option<VolumeFormat>, Error> {
    let size = source.size().await?;
    let len = usize::try_from(size).map_or(VolumeFormat::DETECT_LEN, |size| {
        size.min(VolumeFormat::DETECT_LEN)
    });
    let BufResult(result, head) = source.read_exact_at(Vec::with_capacity(len), 0).await;
    result?;
    Ok(VolumeFormat::detect(&head))
}

fn unsupported(error: VolumeError) -> Error {
    Error::new(ErrorKind::Unsupported, OpKind::Open).with_source(error)
}

/// Returns `root`, the root of a format's driver, as a `Volume`'s, with
/// the same rights.
#[cfg_attr(
    not(any(
        feature = "ext4",
        feature = "fat",
        feature = "ntfs",
        feature = "squashfs",
        feature = "xfs"
    )),
    allow(dead_code)
)]
fn wrap<D: Driver>(
    root: Dir<D>,
    driver: VolumeDriver,
    format: Option<VolumeFormat>,
) -> Result<Dir<Volume>, Error> {
    let volume = Volume {
        inner: DynDriver::new(root.driver().clone()),
        driver,
        format,
    };
    Ok(Dir::new(volume, root.handle().clone())?.restrict(root.rights()))
}

impl Driver for Volume {
    fn id(&self) -> DriverId {
        self.inner.id()
    }

    fn submit(&self, op: Op) -> impl Future<Output = Completion> + Send + use<> {
        self.inner.submit(op)
    }
}

impl core::fmt::Debug for Volume {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Volume")
            .field("driver", &self.driver)
            .field("format", &self.format)
            .field("id", &self.id())
            .finish()
    }
}

/// How to open a volume: each enabled format's own options.
///
/// # Examples
///
/// ```
/// use filer_volume::VolumeOptions;
///
/// // Read volumes not unmounted cleanly as they are on disk.
/// let options = VolumeOptions::default().allow_dirty(true);
/// # let _ = options;
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct VolumeOptions {
    #[cfg(feature = "ext4")]
    ext4: filer_ext4::Options,
    #[cfg(feature = "fat")]
    fat: filer_fat::Options,
    #[cfg(feature = "ntfs")]
    ntfs: filer_ntfs::Options,
    #[cfg(feature = "xfs")]
    xfs: filer_xfs::Options,
}

impl VolumeOptions {
    /// Sets, for every format with a journal or log (ext3 and ext4, NTFS,
    /// and XFS), whether a volume not unmounted cleanly is read as it is on
    /// disk, without the changes its journal or log holds, as each
    /// format's own `allow_dirty` does. Without it, ext4 and XFS replay
    /// those changes in memory, and NTFS refuses the volume.
    #[must_use]
    #[allow(unused_mut, reason = "without a journaled format")]
    pub const fn allow_dirty(mut self, allow: bool) -> Self {
        #[cfg(feature = "ext4")]
        {
            self.ext4 = self.ext4.allow_dirty(allow);
        }
        #[cfg(feature = "ntfs")]
        {
            self.ntfs = self.ntfs.allow_dirty(allow);
        }
        #[cfg(feature = "xfs")]
        {
            self.xfs = self.xfs.allow_dirty(allow);
        }
        let _ = allow;
        self
    }

    /// Sets the options of ext2, ext3, and ext4 volumes.
    #[cfg(feature = "ext4")]
    #[must_use]
    pub const fn with_ext4(mut self, options: filer_ext4::Options) -> Self {
        self.ext4 = options;
        self
    }

    /// Sets the options of FAT volumes, such as their code page.
    #[cfg(feature = "fat")]
    #[must_use]
    pub const fn with_fat(mut self, options: filer_fat::Options) -> Self {
        self.fat = options;
        self
    }

    /// Sets the options of NTFS volumes.
    #[cfg(feature = "ntfs")]
    #[must_use]
    pub const fn with_ntfs(mut self, options: filer_ntfs::Options) -> Self {
        self.ntfs = options;
        self
    }

    /// Sets the options of XFS volumes.
    #[cfg(feature = "xfs")]
    #[must_use]
    pub const fn with_xfs(mut self, options: filer_xfs::Options) -> Self {
        self.xfs = options;
        self
    }
}

/// Why a volume could not be opened in any format: the source of the
/// [`Unsupported`](ErrorKind::Unsupported) or
/// [`NotFound`](ErrorKind::NotFound) error [`Volume`] returns.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum VolumeError {
    /// The source's first bytes are of no known file system.
    Unknown,
    /// The source is a partitioned disk, not a volume: open one of its
    /// partitions with [`Volume::open_partition`].
    Partitioned,
    /// This build does not have the driver, as its feature is disabled.
    Disabled(VolumeDriver),
    /// The disk has no partition table, or no partition of this number.
    NoPartition(u32),
}

impl core::fmt::Display for VolumeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Unknown => f.write_str("no known file system"),
            Self::Partitioned => f.write_str("a partitioned disk, not a volume"),
            Self::Disabled(driver) => write!(f, "the {driver} driver is not enabled"),
            Self::NoPartition(number) => write!(f, "no partition {number}"),
        }
    }
}

impl core::error::Error for VolumeError {}

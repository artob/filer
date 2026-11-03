// This is free and unencumbered software released into the public domain.

use crate::Format;
use alloc::boxed::Box;
use core::future::Future;
use filer_api::{
    Completion, Dir, Driver, DriverId, DynDriver, Error, ErrorKind, Op, OpKind, ReadAt,
    archive::Limits,
};

/// A read-only driver over an archive of any supported [`Format`].
///
/// [`open`](Self::open) picks the format from the archive's file name, and
/// [`open_as`](Self::open_as) takes it from the caller; either way, the
/// format's own driver indexes the archive and serves it, so everything
/// its docs say of reading, hostile archives, and [`Limits`] holds here.
/// The archive's root comes back as a [`Dir`] with only
/// [`READ`](filer_api::Rights::READ) rights, of one type whatever the
/// format or the source, as the format's driver is held as a
/// [`DynDriver`]: each operation costs one boxed future.
///
/// # Examples
///
/// ```
/// use filer_archive::{Archive, Format};
///
/// # #[cfg(feature = "zip")]
/// # filer_spec::block_on(async {
/// let bytes: &'static [u8] = include_bytes!("../tests/data/hello.zip");
/// let root = Archive::open(bytes, "hello.zip").await?;
/// assert_eq!(root.driver().format(), Format::Zip);
/// let file = root.open_file("hello/world.txt").await?;
/// assert_eq!(file.read_to_end().await?, b"Hello, world!\n");
/// # Ok::<(), filer_api::Error>(())
/// # }).unwrap();
/// ```
#[derive(Clone)]
pub struct Archive {
    driver: DynDriver,
    format: Format,
}

impl Archive {
    /// Opens the archive in `source`, in the format its file `name` says
    /// (see [`Format::from_name`]), with the default [`Limits`], and
    /// returns its root directory, labeled with `name`.
    ///
    /// # Errors
    ///
    /// [`Unsupported`](ErrorKind::Unsupported) if `name` names no format,
    /// or one this build does not read, with a [`FormatError`] as its
    /// source; otherwise as the format's driver opens it.
    pub async fn open<S: ReadAt + 'static>(source: S, name: &str) -> Result<Dir<Self>, Error> {
        let format = Format::from_name(name).ok_or_else(|| {
            let error = FormatError::Unknown(Box::from(name));
            Error::new(ErrorKind::Unsupported, OpKind::Open).with_source(error)
        })?;
        let root = Self::open_as(source, format).await?;
        Ok(root.with_label(name))
    }

    /// Opens the archive in `source` as `format`, with the default
    /// [`Limits`], and returns its root directory.
    ///
    /// # Errors
    ///
    /// [`Unsupported`](ErrorKind::Unsupported) if this build does not read
    /// `format`, with a [`FormatError`] as its source; otherwise as the
    /// format's driver opens it.
    pub async fn open_as<S: ReadAt + 'static>(
        source: S,
        format: Format,
    ) -> Result<Dir<Self>, Error> {
        Self::open_with(source, format, Limits::default()).await
    }

    /// Opens the archive in `source` as `format`, with `limits`, and
    /// returns its root directory.
    ///
    /// # Errors
    ///
    /// As for [`open_as`](Self::open_as).
    pub async fn open_with<S: ReadAt + 'static>(
        source: S,
        format: Format,
        limits: Limits,
    ) -> Result<Dir<Self>, Error> {
        match format {
            #[cfg(feature = "ar")]
            Format::Ar => wrap(filer_ar::Ar::open_with(source, limits).await?, format),
            #[cfg(feature = "cpio")]
            Format::Cpio => wrap(filer_cpio::Cpio::open_with(source, limits).await?, format),
            #[cfg(feature = "tar")]
            Format::Tar => wrap(filer_tar::Tar::open_with(source, limits).await?, format),
            #[cfg(feature = "zip")]
            Format::Zip => wrap(filer_zip::Zip::open_with(source, limits).await?, format),
            #[allow(unreachable_patterns, reason = "with every format enabled")]
            _ => {
                let _ = (source, limits);
                let error = FormatError::Disabled(format);
                Err(Error::new(ErrorKind::Unsupported, OpKind::Open).with_source(error))
            },
        }
    }

    /// Returns the archive's format.
    #[must_use]
    pub fn format(&self) -> Format {
        self.format
    }
}

/// Returns `root`, the root of a format's driver, as an `Archive`'s, with
/// the same rights.
#[cfg_attr(
    not(any(feature = "ar", feature = "cpio", feature = "tar", feature = "zip")),
    allow(dead_code)
)]
fn wrap<D: Driver>(root: Dir<D>, format: Format) -> Result<Dir<Archive>, Error> {
    let archive = Archive {
        driver: DynDriver::new(root.driver().clone()),
        format,
    };
    Ok(Dir::new(archive, root.handle().clone())?.restrict(root.rights()))
}

impl Driver for Archive {
    fn id(&self) -> DriverId {
        self.driver.id()
    }

    fn submit(&self, op: Op) -> impl Future<Output = Completion> + Send + use<> {
        self.driver.submit(op)
    }
}

impl core::fmt::Debug for Archive {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Archive")
            .field("format", &self.format)
            .field("id", &self.id())
            .finish()
    }
}

/// Why an archive could not be opened in any format: the source of the
/// [`Unsupported`](ErrorKind::Unsupported) error [`Archive`] returns.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum FormatError {
    /// The file name, held here, names no format.
    Unknown(Box<str>),
    /// This build does not read the format, as its feature is disabled.
    Disabled(Format),
}

impl core::fmt::Display for FormatError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Unknown(name) => write!(f, "no archive format is named by {name:?}"),
            Self::Disabled(format) => write!(f, "the {format} format is not enabled"),
        }
    }
}

impl core::error::Error for FormatError {}

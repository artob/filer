// This is free and unencumbered software released into the public domain.

use crate::Format;
use alloc::{boxed::Box, vec, vec::Vec};
use core::future::Future;
use filer_api::{
    BufResult, Completion, Dir, Driver, DriverId, DynDriver, Error, ErrorKind, Op, OpKind, ReadAt,
};
use filer_pack::{Codec, Compression, Limits};

/// How many of an archive's first bytes tell its format: as far as TAR's
/// magic number reaches, and a block.
const HEAD: usize = 512;
const HEAD_LEN: u64 = 512;

/// The most packed bytes read to decode a compressed archive's first
/// bytes: more than a bzip2 block, the largest unit any decoder needs.
const MAX_PACKED: u64 = 4 << 20;

/// A read-only driver over an archive of any supported [`Format`].
///
/// [`open`](Self::open) picks the format from the archive's content, or
/// else its file name, [`open_by_content`](Self::open_by_content) from its
/// content alone, and [`open_as`](Self::open_as) takes it from the caller.
/// The content tells by magic numbers (see [`Format::detect`]), looking
/// inside a compressed stream for a tarball or cpio archive, and at a ZIP
/// archive's end, past whatever precedes it, such as a self-extractor.
/// Either way, the
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
    /// Opens the archive in `source`, in the format its content says, or
    /// else its file `name` (see [`Format::from_name`]), so that names
    /// that say nothing of the format, as an initramfs image's often do
    /// (`initrd.img`), or misname it, open too; with the default
    /// [`Limits`]; and returns its root directory, labeled with `name`.
    ///
    /// # Errors
    ///
    /// [`Unsupported`](ErrorKind::Unsupported) if neither says a format, or
    /// one says one this build does not read, with a [`FormatError`] as its
    /// source; otherwise as the format's driver opens it, or as reading the
    /// first and last bytes of `source` fails.
    pub async fn open<S: ReadAt + 'static>(source: S, name: &str) -> Result<Dir<Self>, Error> {
        let format = match detect(&source).await? {
            Some(format) => format,
            None => Format::from_name(name)
                .ok_or_else(|| unknown(FormatError::Unknown(Box::from(name))))?,
        };
        let root = Self::open_as(source, format).await?;
        Ok(root.with_label(name))
    }

    /// Opens the archive in `source`, in the format its content says, with
    /// the default [`Limits`], and returns its root directory.
    ///
    /// # Errors
    ///
    /// [`Unsupported`](ErrorKind::Unsupported) if its content says no
    /// format, or one this build does not read, with a [`FormatError`] as
    /// its source; otherwise as the format's driver opens it, or as
    /// reading the first and last bytes of `source` fails.
    pub async fn open_by_content<S: ReadAt + 'static>(source: S) -> Result<Dir<Self>, Error> {
        let format = detect(&source)
            .await?
            .ok_or_else(|| unknown(FormatError::Unrecognized))?;
        Self::open_as(source, format).await
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

/// Returns the error of opening an archive in no format, for `error`.
fn unknown(error: FormatError) -> Error {
    Error::new(ErrorKind::Unsupported, OpKind::Open).with_source(error)
}

/// Returns the format `source`'s content says it is in, if any: what its
/// first bytes say, or, if they begin a compressed stream, what its
/// first bytes decoded say, which only a tarball or cpio archive may be;
/// or else ZIP, if it ends as a ZIP archive does.
async fn detect<S: ReadAt>(source: &S) -> Result<Option<Format>, Error> {
    let size = source.size().await?;
    let head = read(source, 0, size.min(HEAD_LEN)).await?;
    if let Some(format) = Format::detect(&head) {
        return Ok(Some(format));
    }
    if let Some(compression) = Compression::detect(&head).filter(|c| c.is_enabled()) {
        // A stream that does not decode says nothing.
        let decoded = decoded_head(source, size, compression)
            .await
            .unwrap_or_default();
        let format = Format::detect(&decoded);
        return Ok(format.filter(|format| matches!(format, Format::Tar | Format::Cpio)));
    }
    Ok(ends_as_zip(source, size).await?.then_some(Format::Zip))
}

/// Returns the first bytes, up to [`HEAD`], that `source`, of `size` bytes
/// compressed with `compression`, decodes to.
async fn decoded_head<S: ReadAt>(
    source: &S,
    size: u64,
    compression: Compression,
) -> Result<Vec<u8>, Error> {
    let mut decoder = compression.decoder()?;
    let mut head = vec![0; HEAD];
    let (mut filled, mut fetched) = (0_usize, 0_u64);
    let (mut input, mut used) = (Vec::new(), 0_usize);
    let end = size.min(MAX_PACKED);
    while filled < HEAD {
        if used == input.len() && fetched < end {
            let len = end.saturating_sub(fetched).min(64 * 1024);
            input = read(source, fetched, len).await?;
            (used, fetched) = (0, fetched.saturating_add(len));
        }
        let rest = input.get(used..).unwrap_or_default();
        let room = head.get_mut(filled..).unwrap_or_default();
        let (consumed, produced) = decoder.decode(rest, room)?;
        if consumed == 0 && produced == 0 {
            break;
        }
        used = used.saturating_add(consumed);
        filled = filled.saturating_add(produced);
    }
    head.truncate(filled);
    Ok(head)
}

/// Returns whether `source`, of `size` bytes, ends as a ZIP archive does:
/// with an end of central directory record, and as long a comment as it
/// says.
async fn ends_as_zip<S: ReadAt>(source: &S, size: u64) -> Result<bool, Error> {
    const RECORD: usize = 22;
    let len = size.min(u64::from(u16::MAX).saturating_add(22));
    let tail = read(source, size.saturating_sub(len), len).await?;
    Ok((0..tail.len()).rev().any(|at| {
        let record = tail.get(at..).unwrap_or_default();
        let comment = record
            .get(20..RECORD)
            .and_then(|n| <[u8; 2]>::try_from(n).ok());
        record.starts_with(b"PK\x05\x06")
            && comment.is_some_and(|n| {
                usize::from(u16::from_le_bytes(n)) == record.len().saturating_sub(RECORD)
            })
    }))
}

/// Reads the `len` bytes of `source` at `at`, which lie within it.
async fn read<S: ReadAt>(source: &S, at: u64, len: u64) -> Result<Vec<u8>, Error> {
    let len = usize::try_from(len).unwrap_or(HEAD);
    let BufResult(result, bytes) = source.read_exact_at(Vec::with_capacity(len), at).await;
    result.map(|()| bytes)
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
    /// Neither the archive's content nor its file name, held here, says a
    /// format.
    Unknown(Box<str>),
    /// The archive's content says no format.
    Unrecognized,
    /// This build does not read the format, as its feature is disabled.
    Disabled(Format),
}

impl core::fmt::Display for FormatError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Unknown(name) => {
                write!(
                    f,
                    "no archive format is named by {name:?}, or by its content"
                )
            },
            Self::Unrecognized => f.write_str("no archive format is told by its content"),
            Self::Disabled(format) => write!(f, "the {format} format is not enabled"),
        }
    }
}

impl core::error::Error for FormatError {}

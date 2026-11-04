// This is free and unencumbered software released into the public domain.

use crate::{
    header::{BLOCK, BLOCK_LEN, Header},
    index::TAR,
};
use alloc::{boxed::Box, vec::Vec};
use filer_api::{BufResult, Error, ErrorKind, IoBufMut, OpKind, ReadAt};
use filer_pack::{Codec, Decoder, Limits, Unpacked};

/// What a tarball is compressed with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Compression {
    /// bzip2 (`.tar.bz2`), read with the `bzip2` feature.
    Bzip2,
    /// gzip (`.tar.gz`), read with the `gzip` feature.
    Gzip,
    /// xz (`.tar.xz`), read with the `xz` feature.
    Xz,
    /// zstd (`.tar.zst`), read with the `zstd` feature.
    Zstd,
}

impl Compression {
    /// Returns what `head`, an archive's first bytes, says it is compressed
    /// with, by the magic number its stream begins with, if anything.
    ///
    /// bzip2's own is two letters and a digit, so its first block's, or
    /// its end's, must follow, as a member's name might begin `BZh9`.
    #[must_use]
    pub fn detect(head: &[u8]) -> Option<Self> {
        if head.starts_with(&[0x1F, 0x8B]) {
            Some(Self::Gzip)
        } else if head.starts_with(&[0x28, 0xB5, 0x2F, 0xFD]) {
            Some(Self::Zstd)
        } else if head.starts_with(&[0xFD, b'7', b'z', b'X', b'Z', 0]) {
            Some(Self::Xz)
        } else {
            let [b'B', b'Z', b'h', b'1'..=b'9', rest @ ..] = head else {
                return None;
            };
            let block = rest.starts_with(&[0x31, 0x41, 0x59, 0x26, 0x53, 0x59]);
            let end = rest.starts_with(&[0x17, 0x72, 0x45, 0x38, 0x50, 0x90]);
            (block || end).then_some(Self::Bzip2)
        }
    }

    /// Returns whether this build decodes it: whether its feature, named
    /// as it is, is enabled.
    #[must_use]
    pub const fn is_enabled(self) -> bool {
        match self {
            Self::Bzip2 => cfg!(feature = "bzip2"),
            Self::Gzip => cfg!(feature = "gzip"),
            Self::Xz => cfg!(feature = "xz"),
            Self::Zstd => cfg!(feature = "zstd"),
        }
    }
}

impl Codec for Compression {
    fn is_stored(&self) -> bool {
        false
    }

    fn decoder(&self) -> Result<Box<dyn Decoder>, Error> {
        match self {
            #[cfg(feature = "bzip2")]
            Self::Bzip2 => Ok(Box::new(filer_pack::Bzip2::new())),
            #[cfg(feature = "gzip")]
            Self::Gzip => Ok(Box::new(filer_pack::Gzip::new())),
            #[cfg(feature = "xz")]
            Self::Xz => Ok(Box::new(filer_pack::Xz::new())),
            #[cfg(feature = "zstd")]
            Self::Zstd => Ok(Box::new(filer_pack::Zstd::new())),
            #[allow(unreachable_patterns, reason = "with every compression enabled")]
            _ => Err(Error::new(ErrorKind::Unsupported, OpKind::Open)),
        }
    }
}

/// An archive's bytes: as they are, or decoded.
pub(crate) enum Source<S> {
    Plain(S),
    Packed(Unpacked<S, Compression>),
}

impl<S: ReadAt> Source<S> {
    /// Returns the archive in `source`, decoded if it is compressed, which
    /// its first bytes say unless they are a header. Decoding it to the end
    /// once, to learn its size, must not pass `limits`' total size.
    pub(crate) async fn open(source: S, limits: &Limits) -> Result<Self, Error> {
        let size = source.size().await?;
        let len = usize::try_from(size.min(BLOCK_LEN)).unwrap_or(BLOCK);
        let BufResult(result, head) = source.read_exact_at(Vec::with_capacity(len), 0).await;
        result?;
        let header = <&[u8; BLOCK]>::try_from(head.as_slice()).ok().map(Header);
        if header.is_some_and(|header| header.checksum_ok()) {
            return Ok(Self::Plain(source));
        }
        let Some(compression) = Compression::detect(&head) else {
            return Ok(Self::Plain(source));
        };
        if !compression.is_enabled() {
            let reason = "compressed as this build does not decode";
            return Err(TAR.fail(ErrorKind::Unsupported, 0, reason));
        }
        let max = limits.max_total_size;
        let unpacked = Unpacked::measured(source, 0..size, compression, max).await;
        unpacked
            .map(Self::Packed)
            .map_err(|error| match error.kind() {
                ErrorKind::LimitExceeded => {
                    TAR.fail(error.kind(), 0, "decompressed archive too large")
                },
                ErrorKind::Corrupt | ErrorKind::Unsupported => {
                    TAR.fail(error.kind(), 0, "bad compressed stream")
                },
                _ => error,
            })
    }

    /// Returns what the archive is compressed with, if anything.
    pub(crate) fn compression(&self) -> Option<Compression> {
        match self {
            Self::Plain(_) => None,
            Self::Packed(unpacked) => Some(unpacked.codec()),
        }
    }
}

impl<S: ReadAt> ReadAt for Source<S> {
    async fn read_at<B: IoBufMut>(&self, buf: B, off: u64) -> BufResult<usize, B> {
        match self {
            Self::Plain(source) => source.read_at(buf, off).await,
            Self::Packed(unpacked) => unpacked.read_at(buf, off).await,
        }
    }

    async fn size(&self) -> Result<u64, Error> {
        match self {
            Self::Plain(source) => source.size().await,
            Self::Packed(unpacked) => unpacked.size().await,
        }
    }
}

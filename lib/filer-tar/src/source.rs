// This is free and unencumbered software released into the public domain.

use crate::{
    header::{BLOCK, BLOCK_LEN, Header},
    index::TAR,
};
use alloc::vec::Vec;
use filer_api::{BufResult, Error, ErrorKind, IoBufMut, ReadAt};
use filer_pack::{Compression, Limits, Unpacked};

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

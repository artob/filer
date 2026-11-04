// This is free and unencumbered software released into the public domain.

use crate::index::CPIO;
use alloc::vec::Vec;
use filer_api::{BufResult, Error, ErrorKind, IoBufMut, ReadAt};
use filer_pack::{Compression, Limits, Unpacked};

/// An image's bytes: as they are, or as they are up to where its
/// compressed archive begins and then decoded, as the Linux kernel reads
/// an initramfs image.
pub(crate) enum Source<S> {
    Plain(S),
    Spliced {
        /// Where the compressed archive begins, and its decoded bytes.
        at: u64,
        unpacked: Unpacked<S, Compression>,
    },
}

impl<S: ReadAt> Source<S> {
    /// Returns `source` read as it is up to `at`, and from there decoded
    /// with `compression` to its end, which must not pass `limits`' total
    /// size.
    pub(crate) async fn spliced(
        source: S,
        at: u64,
        compression: Compression,
        limits: &Limits,
    ) -> Result<Self, Error> {
        if !compression.is_enabled() {
            let reason = "compressed as this build does not decode";
            return Err(CPIO.fail(ErrorKind::Unsupported, at, reason));
        }
        let end = source.size().await?;
        let max = limits.max_total_size;
        let unpacked = Unpacked::measured(source, at..end, compression, max).await;
        let unpacked = unpacked.map_err(|error| match error.kind() {
            ErrorKind::LimitExceeded => {
                CPIO.fail(error.kind(), at, "decompressed archive too large")
            },
            ErrorKind::Corrupt | ErrorKind::Unsupported => {
                CPIO.fail(error.kind(), at, "bad compressed stream")
            },
            _ => error,
        })?;
        Ok(Self::Spliced { at, unpacked })
    }

    /// Returns what its later part is compressed with, if anything.
    pub(crate) fn compression(&self) -> Option<Compression> {
        match self {
            Self::Plain(_) => None,
            Self::Spliced { unpacked, .. } => Some(unpacked.codec()),
        }
    }
}

impl<S: ReadAt> ReadAt for Source<S> {
    async fn read_at<B: IoBufMut>(&self, mut buf: B, off: u64) -> BufResult<usize, B> {
        match self {
            Self::Plain(source) => source.read_at(buf, off).await,
            Self::Spliced { at, unpacked } => {
                let Some(before) = at.checked_sub(off).filter(|&before| before > 0) else {
                    return unpacked.read_at(buf, off.saturating_sub(*at)).await;
                };
                // Up to the decoded bytes, which a later read goes on to.
                let len = usize::try_from(before).map_or(buf.spare(), |n| n.min(buf.spare()));
                let read = unpacked
                    .source()
                    .read_at(Vec::with_capacity(len), off)
                    .await;
                let BufResult(result, data) = read;
                let result = result.map(|_| buf.fill_from(&data));
                BufResult(result, buf)
            },
        }
    }

    async fn size(&self) -> Result<u64, Error> {
        match self {
            Self::Plain(source) => source.size().await,
            Self::Spliced { at, unpacked } => Ok(at.saturating_add(unpacked.size().await?)),
        }
    }
}

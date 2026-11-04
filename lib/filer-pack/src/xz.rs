// This is free and unencumbered software released into the public domain.

use crate::Decoder;
use alloc::boxed::Box;
use filer_api::{Error, ErrorKind, OpKind};

/// A decoder of xz (the `.xz` file format), `decode-xz`'s, written for
/// Filer from the `.xz` file format specification (`LEGAL.md`, D62):
/// streams, one after another, with the padding between them, each block
/// checked against its CRC-32, CRC-64, or SHA-256 as it ends, and each
/// stream against its index.
///
/// A block's filters may be Delta and the branch converters, then LZMA2;
/// a block with another filter, or needing a dictionary over 128 MiB, is
/// [`Unsupported`](ErrorKind::Unsupported). What follows the last stream
/// must be another, or padding, as xz requires. Between blocks, which
/// decode alone, it offers a [`checkpoint`](Decoder::checkpoint).
///
/// # Examples
///
/// ```
/// use filer_pack::{Decoder, Xz};
///
/// // `printf hello | xz -0 -C crc32`.
/// let packed = [
///     0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00, 0x00, 0x01, 0x69, 0x22, 0xDE, 0x36,
///     0x03, 0xC0, 0x09, 0x05, 0x21, 0x01, 0x0C, 0x00, 0x00, 0x00, 0x00, 0x00,
///     0x6D, 0xFF, 0x09, 0xFF, 0x01, 0x00, 0x04, 0x68, 0x65, 0x6C, 0x6C, 0x6F,
///     0x00, 0x00, 0x00, 0x00, 0x86, 0xA6, 0x10, 0x36, 0x00, 0x01, 0x1D, 0x05,
///     0xB8, 0x2D, 0x80, 0xAF, 0x90, 0x42, 0x99, 0x0D, 0x01, 0x00, 0x00, 0x00,
///     0x00, 0x01, 0x59, 0x5A,
/// ];
/// let mut xz = Xz::new();
/// let mut output = [0; 8];
/// // It returns as its block ends, where it offers a checkpoint.
/// assert_eq!(xz.decode(&packed, &mut output)?, (44, 5));
/// assert_eq!(&output[..5], b"hello");
/// assert!(xz.checkpoint().is_some());
/// // Then come the index and the footer, where the stream ends.
/// assert_eq!(xz.decode(&packed[44..], &mut output)?, (20, 0));
/// assert_eq!(xz.decode(&[], &mut output)?, (0, 0));
/// xz.finish()?;
/// # Ok::<(), filer_api::Error>(())
/// ```
#[derive(Clone, Debug, Default)]
pub struct Xz(decode_xz::Decoder);

impl Xz {
    /// Returns a decoder at the start of a stream.
    #[must_use]
    pub fn new() -> Self {
        Self(decode_xz::Decoder::new())
    }
}

impl Decoder for Xz {
    fn decode(&mut self, input: &[u8], output: &mut [u8]) -> Result<(usize, usize), Error> {
        self.0.decode(input, output).map_err(error)
    }

    fn finish(&mut self) -> Result<(), Error> {
        self.0.finish().map_err(error)
    }

    fn checkpoint(&self) -> Option<Box<dyn Decoder>> {
        let resumed: Box<dyn Decoder> = Box::new(Self(self.0.checkpoint()?));
        Some(resumed)
    }

    fn buffered(&self) -> usize {
        self.0.buffered()
    }
}

/// Returns `decode-xz`'s `error` as one of ours.
fn error(error: decode_xz::Error) -> Error {
    let kind = match error {
        decode_xz::Error::Unsupported => ErrorKind::Unsupported,
        _ => ErrorKind::Corrupt,
    };
    Error::new(kind, OpKind::Read)
}

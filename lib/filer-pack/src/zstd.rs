// This is free and unencumbered software released into the public domain.

use crate::Decoder;
use alloc::boxed::Box;
use filer_api::{Error, ErrorKind, OpKind};

/// A decoder of zstd (RFC 8878), `decode-zstd`'s, written for Filer from
/// RFC 8878 alone (`LEGAL.md`, D62): frames, one after another, each
/// checked against its size and checksum, if it has them; skippable
/// frames are read past.
///
/// It holds at most a block of packed bytes, 128 KiB; and of what they
/// decode to, the frame's window, as much again or 128 KiB if more, and a
/// block. A frame whose window passes 128 MiB, the most zstd itself
/// decodes by default, or that needs a dictionary is
/// [`Unsupported`](ErrorKind::Unsupported). What follows the last frame
/// must be another, as zstd requires. Between frames, it offers a
/// [`checkpoint`](Decoder::checkpoint).
///
/// # Examples
///
/// ```
/// use filer_pack::{Decoder, Zstd};
///
/// // `printf hello | zstd`.
/// let packed = [
///     0x28, 0xB5, 0x2F, 0xFD, 0x04, 0x58, 0x29, 0x00, 0x00, 0x68, 0x65, 0x6C,
///     0x6C, 0x6F, 0xA3, 0x6D, 0x9F, 0x88,
/// ];
/// let mut zstd = Zstd::new();
/// let mut output = [0; 8];
/// assert_eq!(zstd.decode(&packed, &mut output)?, (18, 5));
/// assert_eq!(&output[..5], b"hello");
/// zstd.finish()?;
/// # Ok::<(), filer_api::Error>(())
/// ```
#[derive(Debug, Default)]
pub struct Zstd(decode_zstd::Decoder);

impl Zstd {
    /// Returns a decoder at the start of a stream.
    #[must_use]
    pub const fn new() -> Self {
        Self(decode_zstd::Decoder::new())
    }
}

impl Decoder for Zstd {
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

/// Returns `decode-zstd`'s `error` as one of ours.
fn error(error: decode_zstd::Error) -> Error {
    let kind = match error {
        decode_zstd::Error::Unsupported => ErrorKind::Unsupported,
        _ => ErrorKind::Corrupt,
    };
    Error::new(kind, OpKind::Read)
}

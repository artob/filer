// This is free and unencumbered software released into the public domain.

use crate::Decoder;
use alloc::boxed::Box;
use filer_api::{Error, ErrorKind, OpKind};

/// A decoder of bzip2, `decode-bzip2`'s, written for Filer from Joe Tsai's
/// description of the format (`LEGAL.md`, D62): streams, one after another,
/// each of blocks checked against their CRCs, and their CRCs against the
/// stream's.
///
/// Each block is gathered whole before it is decoded, so it holds at most a
/// block of packed bytes, up to about 900 KiB, and of links between the
/// block's bytes, four bytes each. What a block decodes to, up to 50 times
/// larger, is produced as it is read. After the first stream, bytes that
/// do not start one end the input, unread, as bzip2 ignores trailing
/// garbage. Randomized blocks, which bzip2 stopped writing in 1999, are
/// [`Unsupported`](ErrorKind::Unsupported). Between blocks, it offers a
/// [`checkpoint`](Decoder::checkpoint).
///
/// # Examples
///
/// ```
/// use filer_pack::{Bzip2, Decoder};
///
/// // `printf hello | bzip2`.
/// let packed = [
///     0x42, 0x5A, 0x68, 0x39, 0x31, 0x41, 0x59, 0x26, 0x53, 0x59, 0x19, 0x31,
///     0x65, 0x3D, 0x00, 0x00, 0x00, 0x81, 0x00, 0x02, 0x44, 0xA0, 0x00, 0x21,
///     0x9A, 0x68, 0x33, 0x4D, 0x07, 0x33, 0x8B, 0xB9, 0x22, 0x9C, 0x28, 0x48,
///     0x0C, 0x98, 0xB2, 0x9E, 0x80,
/// ];
/// let mut bzip2 = Bzip2::new();
/// let mut output = [0; 8];
/// assert_eq!(bzip2.decode(&packed, &mut output)?, (41, 5));
/// assert_eq!(&output[..5], b"hello");
/// // It returns after each block; then reads the stream's end.
/// assert_eq!(bzip2.decode(&[], &mut output)?, (0, 0));
/// bzip2.finish()?;
/// # Ok::<(), filer_api::Error>(())
/// ```
#[derive(Debug, Default)]
pub struct Bzip2(decode_bzip2::Decoder);

impl Bzip2 {
    /// Returns a decoder at the start of a stream.
    #[must_use]
    pub const fn new() -> Self {
        Self(decode_bzip2::Decoder::new())
    }
}

impl Decoder for Bzip2 {
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

/// Returns `decode-bzip2`'s `error` as one of ours.
fn error(error: decode_bzip2::Error) -> Error {
    let kind = match error {
        decode_bzip2::Error::Unsupported => ErrorKind::Unsupported,
        _ => ErrorKind::Corrupt,
    };
    Error::new(kind, OpKind::Read)
}

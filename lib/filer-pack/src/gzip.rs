// This is free and unencumbered software released into the public domain.

use crate::Decoder;
use alloc::boxed::Box;
use filer_api::{Error, ErrorKind, OpKind};

/// A decoder of gzip (RFC 1952), `decode-gzip`'s, written for Filer from
/// RFC 1952 alone (`LEGAL.md`, D62): Deflate data in members, each with a
/// header and a trailer, which holds the CRC-32 and length of what the
/// member decodes to, checked as each ends.
///
/// Members may follow one another, as concatenated `.gz` files do, and
/// decode as one stream. After the first, bytes that do not start a member
/// end the stream, unread, as zlib and gzip ignore trailing garbage; a
/// stream that ends within a member fails to
/// [`finish`](Decoder::finish). It offers a
/// [`checkpoint`](Decoder::checkpoint) anywhere.
///
/// # Examples
///
/// ```
/// use filer_pack::{Decoder, Gzip};
///
/// // `printf hello | gzip -n`.
/// let packed = [
///     0x1F, 0x8B, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x03, 0xCB, 0x48,
///     0xCD, 0xC9, 0xC9, 0x07, 0x00, 0x86, 0xA6, 0x10, 0x36, 0x05, 0x00, 0x00,
///     0x00,
/// ];
/// let mut gzip = Gzip::new();
/// let mut output = [0; 8];
/// assert_eq!(gzip.decode(&packed, &mut output)?, (25, 5));
/// assert_eq!(&output[..5], b"hello");
/// gzip.finish()?;
/// # Ok::<(), filer_api::Error>(())
/// ```
#[derive(Clone, Debug, Default)]
pub struct Gzip(decode_gzip::Decoder);

impl Gzip {
    /// Returns a decoder at the start of a stream.
    #[must_use]
    pub fn new() -> Self {
        Self(decode_gzip::Decoder::new())
    }
}

impl Decoder for Gzip {
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

/// Returns `decode-gzip`'s `error` as one of ours.
fn error(_: decode_gzip::Error) -> Error {
    Error::new(ErrorKind::Corrupt, OpKind::Read)
}

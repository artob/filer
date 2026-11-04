// This is free and unencumbered software released into the public domain.

use crate::Decoder;
use alloc::boxed::Box;
use filer_api::{Error, ErrorKind, OpKind};

/// A decoder of a raw Deflate stream (RFC 1951), as ZIP stores members:
/// `decode-deflate`'s, written for Filer from RFC 1951 alone (`LEGAL.md`,
/// D62), which refuses what zlib refuses; or, from
/// [`deflate64`](Self::deflate64), of Deflate64, ZIP's method 9.
///
/// It consumes the stream to its end and no further, and offers a
/// [`checkpoint`](Decoder::checkpoint) anywhere: a copy of its state, its
/// 32 KiB window and a few KiB.
///
/// # Examples
///
/// ```
/// use filer_pack::{Decoder, Deflate};
///
/// // "hello", deflated as one fixed-Huffman block.
/// let packed = [0xCB, 0x48, 0xCD, 0xC9, 0xC9, 0x07, 0x00];
/// let mut output = [0; 5];
/// assert_eq!(Deflate::new().decode(&packed, &mut output)?.1, 5);
/// assert_eq!(&output, b"hello");
/// # Ok::<(), filer_api::Error>(())
/// ```
#[derive(Clone, Debug, Default)]
pub struct Deflate(decode_deflate::Decoder);

impl Deflate {
    /// Returns a decoder at the start of a stream.
    #[must_use]
    pub fn new() -> Self {
        Self(decode_deflate::Decoder::new())
    }

    /// Returns a decoder at the start of a Deflate64 stream: PKWARE's
    /// variant of Deflate, with a 64 KiB window and longer matches.
    #[cfg(feature = "deflate64")]
    #[must_use]
    pub fn deflate64() -> Self {
        Self(decode_deflate::Decoder::deflate64())
    }
}

impl Decoder for Deflate {
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

/// Returns `decode-deflate`'s `error` as one of ours.
fn error(_: decode_deflate::Error) -> Error {
    Error::new(ErrorKind::Corrupt, OpKind::Read)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_malformed_streams() {
        // Block type 3 is reserved.
        let error = Deflate::new().decode(&[0x07], &mut [0; 8]).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Corrupt);
        // An empty input makes no progress, which callers see, and the
        // stream has not ended.
        let mut deflate = Deflate::new();
        assert_eq!(deflate.decode(&[], &mut [0; 8]).unwrap(), (0, 0));
        assert_eq!(deflate.finish().unwrap_err().kind(), ErrorKind::Corrupt);
    }
}

// This is free and unencumbered software released into the public domain.

use crate::Decoder;
use alloc::boxed::Box;
use filer_api::{Error, ErrorKind, OpKind};

/// A decoder of a raw LZMA stream, as ZIP and 7z store members:
/// `decode-lzma`'s, ported for Filer from the LZMA SDK (`LEGAL.md`, D62).
///
/// Its properties come from the container. Of a known size, it ends there,
/// or at an end marker that follows; of an unknown size, it ends at its
/// end marker.
///
/// # Examples
///
/// ```
/// use filer_pack::{Decoder, Lzma};
///
/// // `printf hello | xz --format=lzma -0`: a 13-byte header, of the
/// // properties and an unknown size, then the stream.
/// let packed = [
///     0x5D, 0x00, 0x00, 0x04, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
///     0xFF, 0x00, 0x34, 0x19, 0x49, 0xDB, 0x85, 0x64, 0xF1, 0x93, 0xB1, 0xFF,
///     0xFB, 0x8F, 0xC0, 0x00,
/// ];
/// let properties = packed[..5].try_into().unwrap();
/// let mut lzma = Lzma::new(properties, None)?;
/// let mut output = [0; 8];
/// assert_eq!(lzma.decode(&packed[13..], &mut output)?.1, 5);
/// assert_eq!(&output[..5], b"hello");
/// assert_eq!(lzma.decode(&[], &mut output)?, (0, 0));
/// lzma.finish()?;
/// # Ok::<(), filer_api::Error>(())
/// ```
#[derive(Debug)]
pub struct Lzma(Box<decode_lzma::Decoder>);

impl Lzma {
    /// Returns a decoder of a stream with `properties` as ZIP and 7z store
    /// them, the byte of `lc`, `lp`, and `pb`, then the dictionary size,
    /// little-endian; decoding to `size` bytes, if known.
    ///
    /// # Errors
    ///
    /// [`Corrupt`](ErrorKind::Corrupt) for invalid properties, and
    /// [`Unsupported`](ErrorKind::Unsupported) for a dictionary over 128
    /// MiB that the size does not bound.
    pub fn new(properties: [u8; 5], size: Option<u64>) -> Result<Self, Error> {
        let decoder = decode_lzma::Decoder::lzma(properties, size).map_err(error)?;
        Ok(Self(Box::new(decoder)))
    }
}

impl Decoder for Lzma {
    fn decode(&mut self, input: &[u8], output: &mut [u8]) -> Result<(usize, usize), Error> {
        self.0.decode(input, output).map_err(error)
    }

    fn finish(&mut self) -> Result<(), Error> {
        self.0.finish().map_err(error)
    }
}

/// A decoder of a raw LZMA2 stream, as 7z stores members and xz blocks
/// hold, `decode-lzma`'s.
///
/// # Examples
///
/// ```
/// use filer_pack::{Decoder, Lzma2};
///
/// // `printf hello | xz --format=raw --lzma2=preset=0`: a chunk stored as
/// // is, then the end.
/// let packed = [0x01, 0x00, 0x04, 0x68, 0x65, 0x6C, 0x6C, 0x6F, 0x00];
/// // A dictionary of 256 KiB, as `-0` uses.
/// let mut lzma2 = Lzma2::new(13)?;
/// let mut output = [0; 8];
/// assert_eq!(lzma2.decode(&packed, &mut output)?, (9, 5));
/// assert_eq!(&output[..5], b"hello");
/// lzma2.finish()?;
/// # Ok::<(), filer_api::Error>(())
/// ```
#[derive(Debug)]
pub struct Lzma2(Box<decode_lzma::Decoder>);

impl Lzma2 {
    /// Returns a decoder of a stream whose dictionary size is `property`,
    /// the one byte by which xz and 7z store it.
    ///
    /// # Errors
    ///
    /// [`Corrupt`](ErrorKind::Corrupt) for a property past 40, and
    /// [`Unsupported`](ErrorKind::Unsupported) for a dictionary over 128
    /// MiB.
    pub fn new(property: u8) -> Result<Self, Error> {
        let decoder = decode_lzma::Decoder::lzma2(property).map_err(error)?;
        Ok(Self(Box::new(decoder)))
    }
}

impl Decoder for Lzma2 {
    fn decode(&mut self, input: &[u8], output: &mut [u8]) -> Result<(usize, usize), Error> {
        self.0.decode(input, output).map_err(error)
    }

    fn finish(&mut self) -> Result<(), Error> {
        self.0.finish().map_err(error)
    }
}

/// Returns `decode-lzma`'s `error` as one of ours.
fn error(error: decode_lzma::Error) -> Error {
    let kind = match error {
        decode_lzma::Error::Unsupported => ErrorKind::Unsupported,
        _ => ErrorKind::Corrupt,
    };
    Error::new(kind, OpKind::Read)
}

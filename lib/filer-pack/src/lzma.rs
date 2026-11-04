// This is free and unencumbered software released into the public domain.

use crate::Decoder;
use alloc::boxed::Box;
use filer_api::{Error, ErrorKind, OpKind};
use lzma_rust2::{Action, Lzma2Stream, LzmaStream, Status};

/// The largest dictionary a decoder takes: twice that of `xz -9`, and as
/// large as zstd's window may be. A larger one is
/// [`Unsupported`](ErrorKind::Unsupported).
const MAX_DICTIONARY: u32 = 128 << 20;

/// A decoder of a raw LZMA stream, as ZIP and 7z store members, over
/// `lzma-rust2`.
///
/// Its properties come from the container. Of a known size, it ends there,
/// whether or not an end marker follows; of an unknown size, it ends at its
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
pub struct Lzma {
    stream: Box<LzmaStream>,
    ended: bool,
}

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
        let [props, dictionary @ ..] = properties;
        let dictionary = u32::from_le_bytes(dictionary);
        // No more of the dictionary is used than the stream decodes to.
        let used = size
            .and_then(|size| u32::try_from(size).ok())
            .map_or(dictionary, |size| size.min(dictionary));
        if used > MAX_DICTIONARY {
            return Err(unsupported());
        }
        let size = size.unwrap_or(u64::MAX);
        let stream =
            LzmaStream::new_with_props(size, props, dictionary, None).map_err(|_| corrupt())?;
        Ok(Self {
            stream: Box::new(stream),
            ended: false,
        })
    }
}

impl core::fmt::Debug for Lzma {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Lzma")
            .field("ended", &self.ended)
            .finish_non_exhaustive()
    }
}

impl Decoder for Lzma {
    fn decode(&mut self, input: &[u8], output: &mut [u8]) -> Result<(usize, usize), Error> {
        if self.ended {
            return Ok((0, 0));
        }
        let result = self
            .stream
            .process(input, output, action(input))
            .map_err(|_| corrupt())?;
        self.ended = result.status == Status::StreamEnd;
        Ok((result.bytes_consumed, result.bytes_produced))
    }

    fn finish(&mut self) -> Result<(), Error> {
        ended(self.ended)
    }
}

/// A decoder of a raw LZMA2 stream, as 7z stores members and xz blocks
/// hold, over `lzma-rust2`.
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
pub struct Lzma2 {
    stream: Box<Lzma2Stream>,
    ended: bool,
}

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
        // Two or three, shifted by half the property and 11; at 40, all.
        let dictionary = match property {
            0..=39 => (2 | u32::from(property & 1))
                .checked_shl(u32::from(property >> 1).saturating_add(11))
                .ok_or_else(corrupt)?,
            40 => u32::MAX,
            _ => return Err(corrupt()),
        };
        if dictionary > MAX_DICTIONARY {
            return Err(unsupported());
        }
        Ok(Self {
            stream: Box::new(Lzma2Stream::new(dictionary)),
            ended: false,
        })
    }
}

impl core::fmt::Debug for Lzma2 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Lzma2")
            .field("ended", &self.ended)
            .finish_non_exhaustive()
    }
}

impl Decoder for Lzma2 {
    fn decode(&mut self, input: &[u8], output: &mut [u8]) -> Result<(usize, usize), Error> {
        if self.ended {
            return Ok((0, 0));
        }
        let result = self
            .stream
            .process(input, output, action(input))
            .map_err(|_| corrupt())?;
        self.ended = result.status == Status::StreamEnd;
        Ok((result.bytes_consumed, result.bytes_produced))
    }

    fn finish(&mut self) -> Result<(), Error> {
        ended(self.ended)
    }
}

/// A decoder of xz (the `.xz` file format), over `lzma-rust2`: streams,
/// one after another, with the padding between them, each block checked
/// against its CRC-32, CRC-64, or SHA-256 as it ends, and each stream
/// against its index.
///
/// A block's filters may be one of the BCJ filters or Delta, then LZMA2; a
/// block needing a dictionary over 128 MiB is
/// [`Unsupported`](ErrorKind::Unsupported). What follows the last stream
/// must be another, or padding, as xz requires.
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
/// assert_eq!(xz.decode(&packed, &mut output)?, (64, 5));
/// assert_eq!(&output[..5], b"hello");
/// // The stream ends where its packed bytes do.
/// assert_eq!(xz.decode(&[], &mut output)?, (0, 0));
/// xz.finish()?;
/// # Ok::<(), filer_api::Error>(())
/// ```
#[cfg(feature = "xz")]
pub struct Xz {
    stream: Box<lzma_rust2::XzStream>,
    ended: bool,
}

#[cfg(feature = "xz")]
impl Xz {
    /// Returns a decoder at the start of a stream.
    #[must_use]
    pub fn new() -> Self {
        let limit = lzma_rust2::lzma2_get_memory_usage(MAX_DICTIONARY);
        Self {
            stream: Box::new(lzma_rust2::XzStream::new_mem_limit(true, limit)),
            ended: false,
        }
    }
}

#[cfg(feature = "xz")]
impl Default for Xz {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "xz")]
impl core::fmt::Debug for Xz {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Xz")
            .field("ended", &self.ended)
            .finish_non_exhaustive()
    }
}

#[cfg(feature = "xz")]
impl Decoder for Xz {
    fn decode(&mut self, input: &[u8], output: &mut [u8]) -> Result<(usize, usize), Error> {
        if self.ended {
            return Ok((0, 0));
        }
        let result = self.stream.process(input, output, action(input));
        let result = result.map_err(|_| {
            // The limit is passed only by a dictionary too large.
            let limit = self.stream.mem_limit();
            match self.stream.memory_usage() {
                Some(need) if need > limit => unsupported(),
                _ => corrupt(),
            }
        })?;
        self.ended = result.status == Status::StreamEnd;
        Ok((result.bytes_consumed, result.bytes_produced))
    }

    fn finish(&mut self) -> Result<(), Error> {
        ended(self.ended)
    }
}

/// Returns the action for `input`: to finish once it is empty, as it is
/// once the packed bytes are exhausted, so that the stream ends.
fn action(input: &[u8]) -> Action {
    if input.is_empty() {
        Action::Finish
    } else {
        Action::Run
    }
}

/// Returns whether a stream that has `ended` may finish.
fn ended(ended: bool) -> Result<(), Error> {
    if ended { Ok(()) } else { Err(corrupt()) }
}

fn corrupt() -> Error {
    Error::new(ErrorKind::Corrupt, OpKind::Read)
}

fn unsupported() -> Error {
    Error::new(ErrorKind::Unsupported, OpKind::Read)
}

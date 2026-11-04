// This is free and unencumbered software released into the public domain.

use crate::Decoder;
use alloc::boxed::Box;
use filer_api::{Error, ErrorKind, OpKind};
use lzma_rust2::{Action, Status};

/// The largest dictionary a decoder takes: twice that of `xz -9`, and as
/// large as zstd's window may be. A larger one is
/// [`Unsupported`](ErrorKind::Unsupported).
const MAX_DICTIONARY: u32 = 128 << 20;

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
pub struct Xz {
    stream: Box<lzma_rust2::XzStream>,
    ended: bool,
}

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

impl Default for Xz {
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Debug for Xz {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Xz")
            .field("ended", &self.ended)
            .finish_non_exhaustive()
    }
}

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

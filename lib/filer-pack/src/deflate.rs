// This is free and unencumbered software released into the public domain.

use crate::Decoder;
use alloc::boxed::Box;
use filer_api::{Error, ErrorKind, OpKind};
use miniz_oxide::{
    DataFormat, MZError, MZFlush, MZStatus,
    inflate::stream::{InflateState, inflate},
};

/// A decoder of a raw Deflate stream (RFC 1951), as ZIP stores members,
/// over `miniz_oxide`.
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
#[derive(Clone)]
pub struct Deflate {
    state: Box<InflateState>,
    /// Whether the last block is decoded and all of it produced.
    ended: bool,
}

impl Deflate {
    /// Returns a decoder at the start of a stream.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: InflateState::new_boxed(DataFormat::Raw),
            ended: false,
        }
    }

    /// Returns whether the stream has ended: its last block decoded, and
    /// all of it produced. A container reads on from there, past the
    /// bytes consumed.
    #[cfg(feature = "gzip")]
    pub(crate) const fn ended(&self) -> bool {
        self.ended
    }

    /// Starts over, at the start of a new stream.
    #[cfg(feature = "gzip")]
    pub(crate) fn reset(&mut self) {
        self.state.reset(DataFormat::Raw);
        self.ended = false;
    }
}

impl Default for Deflate {
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Debug for Deflate {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Deflate").finish_non_exhaustive()
    }
}

impl Decoder for Deflate {
    fn decode(&mut self, input: &[u8], output: &mut [u8]) -> Result<(usize, usize), Error> {
        let result = inflate(&mut self.state, input, output, MZFlush::None);
        // At its end, the stream consumes no bytes past it.
        self.ended |= result.status == Ok(MZStatus::StreamEnd);
        match result.status {
            // No progress: the archive sees the member end early.
            Ok(_) | Err(MZError::Buf) => Ok((result.bytes_consumed, result.bytes_written)),
            Err(_) => Err(Error::new(ErrorKind::Corrupt, OpKind::Read)),
        }
    }

    /// Anywhere: a copy of its state, about 43 KiB with its window.
    fn checkpoint(&self) -> Option<Box<dyn Decoder>> {
        Some(Box::new(self.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_malformed_streams() {
        // Block type 3 is reserved.
        let error = Deflate::new().decode(&[0x07], &mut [0; 8]).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Corrupt);
        // An empty input makes no progress, which callers see.
        assert_eq!(Deflate::new().decode(&[], &mut [0; 8]).unwrap(), (0, 0));
    }
}

// This is free and unencumbered software released into the public domain.

use crate::Decoder;
use alloc::boxed::Box;
use filer_api::{Error, ErrorKind, OpKind, checksum::Crc32};

/// A checksum computed over a member's bytes as they are decoded, which a
/// [`Verified`] decoder compares with the one the archive records.
///
/// [`Crc32`] is one, as ZIP and gzip record it.
///
/// # Examples
///
/// ```
/// use filer_api::checksum::Crc32;
/// use filer_pack::Digest;
///
/// let mut crc = Crc32::new();
/// Digest::update(&mut crc, b"1234");
/// Digest::update(&mut crc, b"56789");
/// assert_eq!(Digest::finish(&crc), 0xCBF4_3926);
/// ```
pub trait Digest: Clone + Send + 'static {
    /// The checksum's value, such as a `u32` for CRC-32.
    type Output: Clone + PartialEq + Send + 'static;

    /// Feeds the next decoded `bytes`.
    fn update(&mut self, bytes: &[u8]);

    /// Returns the checksum of the bytes fed so far.
    fn finish(&self) -> Self::Output;
}

impl Digest for Crc32 {
    type Output = u32;

    fn update(&mut self, bytes: &[u8]) {
        *self = Crc32::update(*self, bytes);
    }

    fn finish(&self) -> u32 {
        Crc32::finish(*self)
    }
}

/// A decoder that checks a [`Digest`] of what it produced once the member
/// ends, failing [`finish`](Decoder::finish) with
/// [`Corrupt`](ErrorKind::Corrupt) if it differs from the one recorded.
///
/// `D` decodes the member; for a stored member whose format records a
/// checksum, as cpio's `newc` does, it is [`Passthrough`].
///
/// # Examples
///
/// ```
/// use filer_api::{ErrorKind, checksum::{Crc32, crc32}};
/// use filer_pack::{Decoder, Passthrough, Verified};
///
/// let mut good = Verified::new(Passthrough, Crc32::new(), crc32(b"data"));
/// let mut output = [0; 4];
/// assert_eq!(good.decode(b"data", &mut output)?, (4, 4));
/// assert_eq!(&output, b"data");
/// good.finish()?;
///
/// let mut bad = Verified::new(Passthrough, Crc32::new(), 0);
/// bad.decode(b"data", &mut output)?;
/// assert_eq!(bad.finish().unwrap_err().kind(), ErrorKind::Corrupt);
/// # Ok::<(), filer_api::Error>(())
/// ```
#[derive(Clone, Debug)]
pub struct Verified<D, H: Digest> {
    decoder: D,
    digest: H,
    expected: H::Output,
}

impl<D, H: Digest> Verified<D, H> {
    /// Returns `decoder`, checked by `digest` against `expected`.
    #[must_use]
    pub const fn new(decoder: D, digest: H, expected: H::Output) -> Self {
        Self {
            decoder,
            digest,
            expected,
        }
    }
}

impl<D: Decoder, H: Digest> Decoder for Verified<D, H> {
    fn decode(&mut self, input: &[u8], output: &mut [u8]) -> Result<(usize, usize), Error> {
        let (consumed, produced) = self.decoder.decode(input, output)?;
        self.digest
            .update(output.get(..produced).unwrap_or_default());
        Ok((consumed, produced))
    }

    fn finish(&mut self) -> Result<(), Error> {
        self.decoder.finish()?;
        if self.digest.finish() == self.expected {
            Ok(())
        } else {
            Err(Error::new(ErrorKind::Corrupt, OpKind::Read))
        }
    }

    /// Where the decoder offers one, with the digest so far.
    fn checkpoint(&self) -> Option<Box<dyn Decoder>> {
        let decoder = self.decoder.checkpoint()?;
        let expected = self.expected.clone();
        Some(Box::new(Verified::new(
            decoder,
            self.digest.clone(),
            expected,
        )))
    }
}

/// A decoder that copies its input: for members stored as is whose format
/// records a checksum, under [`Verified`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Passthrough;

impl Decoder for Passthrough {
    fn decode(&mut self, input: &[u8], output: &mut [u8]) -> Result<(usize, usize), Error> {
        let n = input.len().min(output.len());
        let (Some(from), Some(to)) = (input.get(..n), output.get_mut(..n)) else {
            return Ok((0, 0));
        };
        to.copy_from_slice(from);
        Ok((n, n))
    }

    fn checkpoint(&self) -> Option<Box<dyn Decoder>> {
        Some(Box::new(Self))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use filer_api::checksum::crc32;

    /// A decoder that produces each byte twice, and fails to finish if
    /// told to.
    struct Doubled {
        fail: bool,
    }

    impl Decoder for Doubled {
        fn decode(&mut self, input: &[u8], output: &mut [u8]) -> Result<(usize, usize), Error> {
            let n = input.len().min(output.len() / 2);
            for (i, &byte) in input[..n].iter().enumerate() {
                output[2 * i] = byte;
                output[2 * i + 1] = byte;
            }
            Ok((n, 2 * n))
        }

        fn finish(&mut self) -> Result<(), Error> {
            if self.fail {
                Err(Error::new(ErrorKind::Unsupported, OpKind::Read))
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn checks_what_was_produced_in_pieces() {
        let expected = crc32(b"aabbcc");
        let mut verified = Verified::new(Doubled { fail: false }, Crc32::new(), expected);
        let mut output = [0; 4];
        assert_eq!(verified.decode(b"abc", &mut output).unwrap(), (2, 4));
        assert_eq!(verified.decode(b"c", &mut output).unwrap(), (1, 2));
        verified.finish().unwrap();
    }

    #[test]
    fn fails_on_a_mismatch_or_the_inner_decoder() {
        let mut short = Verified::new(Doubled { fail: false }, Crc32::new(), crc32(b"aabb"));
        short.decode(b"a", &mut [0; 2]).unwrap();
        assert_eq!(short.finish().unwrap_err().kind(), ErrorKind::Corrupt);
        let mut failing = Verified::new(Doubled { fail: true }, Crc32::new(), crc32(b""));
        assert_eq!(failing.finish().unwrap_err().kind(), ErrorKind::Unsupported);
    }

    #[test]
    fn checkpoints_carry_the_digest_so_far() {
        let expected = crc32(b"first, then second");
        let mut verified = Verified::new(Passthrough, Crc32::new(), expected);
        let mut output = [0; 32];
        verified.decode(b"first, ", &mut output).unwrap();
        let mut resumed = verified.checkpoint().unwrap();
        for decoder in [&mut verified as &mut dyn Decoder, &mut *resumed] {
            decoder.decode(b"then second", &mut output).unwrap();
            decoder.finish().unwrap();
        }
    }

    #[test]
    fn passes_bytes_through() {
        let mut output = [0; 3];
        assert_eq!(Passthrough.decode(b"abcd", &mut output).unwrap(), (3, 3));
        assert_eq!(&output, b"abc");
        assert_eq!(Passthrough.decode(b"", &mut output).unwrap(), (0, 0));
    }
}

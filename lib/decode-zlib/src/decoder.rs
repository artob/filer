// This is free and unencumbered software released into the public domain.

use crate::{Error, corrupt};
use compute_adler32::Adler32;

/// The one compression method defined, Deflate, in the low half of the
/// header's first byte (RFC 1950 §2.2).
const DEFLATE: u8 = 8;

/// The largest window the header's high half may give, 2^(7 + 8) bytes.
const MAX_CINFO: u8 = 7;

/// The flag of a preset dictionary, in the header's second byte.
const FDICT: u8 = 0x20;

/// Where a [`Decoder`] is in its stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    /// In the header, of 2 bytes, with this many read.
    Header(u8),
    /// In the Deflate data.
    Body,
    /// In the trailer, the Adler-32 of 4 bytes, with this many read.
    Trailer(u8),
    /// Past the trailer, before what follows the stream.
    Ended,
}

/// A decoder of zlib (RFC 1950): Deflate data between a header of 2 bytes
/// and the Adler-32 of what it decodes to, checked as it ends.
///
/// It consumes the stream to its trailer's end and no further, so that
/// what follows is left unread. The header's window size, of at most
/// 32 KiB, is checked but not imposed: as zlib does by default, matches
/// may copy from as far back as Deflate allows. A stream that needs a
/// preset dictionary is [`Unsupported`](Error::Unsupported).
///
/// # Examples
///
/// ```
/// use decode_zlib::Decoder;
///
/// // `hello`, as zlib's `compress` writes it, and what follows.
/// let packed = [
///     0x78, 0x9C, 0xCB, 0x48, 0xCD, 0xC9, 0xC9, 0x07, 0x00, 0x06, 0x2C, 0x02,
///     0x15, 0xFF,
/// ];
/// let mut decoder = Decoder::new();
/// let mut output = [0; 8];
/// assert_eq!(decoder.decode(&packed, &mut output)?, (13, 5));
/// assert_eq!(&output[..5], b"hello");
/// decoder.finish()?;
/// # Ok::<(), decode_zlib::Error>(())
/// ```
#[derive(Clone, Debug)]
pub struct Decoder {
    state: State,
    /// The bytes read of the header or trailer in progress.
    field: [u8; 4],
    deflate: decode_deflate::Decoder,
    /// The Adler-32 of what it has decoded to so far.
    sum: Adler32,
}

impl Decoder {
    /// Returns a decoder at the start of a stream.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: State::Header(0),
            field: [0; 4],
            deflate: decode_deflate::Decoder::new(),
            sum: Adler32::new(),
        }
    }

    /// Decodes what it can of `input` into `output`, and returns how many
    /// bytes of each it consumed and produced.
    ///
    /// It returns neither consumed nor produced once it can go no further:
    /// needing more input, or more room, or past the end of the stream,
    /// which [`finish`](Self::finish) then tells apart. It consumes no
    /// byte past the stream's end.
    ///
    /// # Errors
    ///
    /// [`Corrupt`](Error::Corrupt) for a malformed stream, and
    /// [`Unsupported`](Error::Unsupported) for one that needs a preset
    /// dictionary. The decoder is not to be used after an error.
    pub fn decode(&mut self, input: &[u8], output: &mut [u8]) -> Result<(usize, usize), Error> {
        let (mut consumed, mut produced) = (0_usize, 0_usize);
        loop {
            let state = self.state;
            let input = input.get(consumed..).unwrap_or_default();
            let output = output.get_mut(produced..).unwrap_or_default();
            let (c, p) = self.step(input, output)?;
            consumed = consumed.saturating_add(c);
            produced = produced.saturating_add(p);
            if c == 0 && p == 0 && self.state == state {
                return Ok((consumed, produced));
            }
        }
    }

    /// Checks that the stream has ended: its trailer read, and checked.
    ///
    /// # Errors
    ///
    /// [`Corrupt`](Error::Corrupt) if it has not.
    pub fn finish(&self) -> Result<(), Error> {
        if self.state == State::Ended {
            Ok(())
        } else {
            Err(corrupt())
        }
    }

    /// Returns a decoder that goes on from where this one is: a copy of
    /// it, with no more of the Deflate window than matches may copy from.
    #[must_use]
    pub fn checkpoint(&self) -> Option<Self> {
        Some(Self {
            deflate: self.deflate.checkpoint()?,
            ..*self
        })
    }

    /// Returns how many of the bytes it has consumed a
    /// [`checkpoint`](Self::checkpoint) must be given again: none, as it
    /// holds them.
    #[must_use]
    pub const fn buffered(&self) -> usize {
        0
    }

    /// Decodes what it can from `input` into `output` in the current
    /// state, and returns how much of each it consumed and produced.
    fn step(&mut self, input: &[u8], output: &mut [u8]) -> Result<(usize, usize), Error> {
        match self.state {
            State::Ended => Ok((0, 0)),
            State::Body => {
                let (consumed, produced) =
                    self.deflate.decode(input, output).map_err(|_| corrupt())?;
                self.sum = self.sum.update(output.get(..produced).unwrap_or_default());
                if self.deflate.finish().is_ok() {
                    self.state = State::Trailer(0);
                }
                Ok((consumed, produced))
            },
            State::Header(seen) | State::Trailer(seen) => {
                let Some(&byte) = input.first() else {
                    return Ok((0, 0));
                };
                if let Some(slot) = self.field.get_mut(usize::from(seen)) {
                    *slot = byte;
                }
                let seen = seen.saturating_add(1);
                self.state = match self.state {
                    State::Header(_) if seen == 2 => self.header()?,
                    State::Trailer(_) if seen == 4 => self.trailer()?,
                    State::Header(_) => State::Header(seen),
                    _ => State::Trailer(seen),
                };
                Ok((1, 0))
            },
        }
    }

    /// Checks the whole header, and returns the state after it.
    fn header(&self) -> Result<State, Error> {
        let [cmf, flg, ..] = self.field;
        // The two bytes, as a number, are a multiple of 31.
        let check = u16::from_be_bytes([cmf, flg]).wrapping_rem(31);
        if check != 0 || cmf & 0x0F != DEFLATE || cmf >> 4 > MAX_CINFO {
            return Err(corrupt());
        }
        if flg & FDICT != 0 {
            return Err(Error::Unsupported);
        }
        Ok(State::Body)
    }

    /// Checks the whole trailer against what the stream decoded to, and
    /// returns the state after it.
    fn trailer(&self) -> Result<State, Error> {
        if u32::from_be_bytes(self.field) != self.sum.finish() {
            return Err(corrupt());
        }
        Ok(State::Ended)
    }
}

impl Default for Decoder {
    fn default() -> Self {
        Self::new()
    }
}

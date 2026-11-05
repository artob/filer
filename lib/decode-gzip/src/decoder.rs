// This is free and unencumbered software released into the public domain.

use crate::{Error, corrupt};
use compute_crc::Crc32;

/// The flags of a member's header (RFC 1952 §2.3.1). Those of its optional
/// fields ascend in the order the fields follow the fixed header, which
/// the header's CRC-16 follows last; the top three bits are reserved.
const FHCRC: u8 = 0x02;
const FEXTRA: u8 = 0x04;
const FNAME: u8 = 0x08;
const FCOMMENT: u8 = 0x10;
const RESERVED: u8 = 0xE0;

/// Where a [`Decoder`] is in its stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    /// In a member's fixed header, of 10 bytes, with this many read: at
    /// none, before a member.
    Fixed(u8),
    /// In the extra field's length, of 2 bytes, with this many read.
    ExtraLen(u8),
    /// In the extra field, with this many bytes left.
    Extra(u16),
    /// In the zero-terminated name.
    Name,
    /// In the zero-terminated comment.
    Comment,
    /// In the header's CRC-16, of 2 bytes, with this many read.
    HeaderCrc(u8),
    /// In the Deflate data.
    Body,
    /// In the trailer, of 8 bytes, with this many read.
    Trailer(u8),
    /// Past the last member, before what is not one.
    Ended,
}

/// A decoder of gzip (RFC 1952): Deflate data in members, each with a
/// header and a trailer, which holds the CRC-32 and length of what the
/// member decodes to, checked as each ends.
///
/// Members may follow one another, as concatenated `.gz` files do, and
/// decode as one stream. After the first, bytes that do not start a member
/// end the stream, unread, as gzip and zlib ignore trailing garbage, such
/// as a tape's padding of zeros; but a byte that starts gzip's magic
/// number must start a member. A stream that ends within a member fails to
/// [`finish`](Self::finish). The header's optional fields are read past,
/// and its CRC-16, if present, checked.
///
/// # Examples
///
/// ```
/// use decode_gzip::Decoder;
///
/// // `printf hello | gzip -n`, and what follows.
/// let packed = [
///     0x1F, 0x8B, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x03, 0xCB, 0x48,
///     0xCD, 0xC9, 0xC9, 0x07, 0x00, 0x86, 0xA6, 0x10, 0x36, 0x05, 0x00, 0x00,
///     0x00, 0xFF,
/// ];
/// let mut decoder = Decoder::new();
/// let mut output = [0; 8];
/// assert_eq!(decoder.decode(&packed, &mut output)?, (25, 5));
/// assert_eq!(&output[..5], b"hello");
/// decoder.finish()?;
/// # Ok::<(), decode_gzip::Error>(())
/// ```
#[derive(Clone, Debug)]
pub struct Decoder {
    state: State,
    /// The member's flags.
    flags: u8,
    /// The bytes read of the fixed-size field in progress.
    field: [u8; 8],
    /// The CRC-32 of the member's header so far, for its CRC-16.
    header_crc: Crc32,
    deflate: decode_deflate::Decoder,
    /// The CRC-32 and length of what the member has decoded to so far.
    crc: Crc32,
    len: u64,
    /// The members decoded whole.
    members: u64,
}

impl Decoder {
    /// Returns a decoder at the start of a stream.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: State::Fixed(0),
            flags: 0,
            field: [0; 8],
            header_crc: Crc32::new(),
            deflate: decode_deflate::Decoder::new(),
            crc: Crc32::new(),
            len: 0,
            members: 0,
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
    /// [`Corrupt`](Error::Corrupt) for a malformed stream. The decoder is
    /// not to be used after an error.
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

    /// Checks that the stream has ended: at least one member decoded, and
    /// the stream between members, or past the last.
    ///
    /// # Errors
    ///
    /// [`Corrupt`](Error::Corrupt) if it has not.
    pub fn finish(&self) -> Result<(), Error> {
        if self.members > 0 && matches!(self.state, State::Fixed(0) | State::Ended) {
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
                self.crc.update(output.get(..produced).unwrap_or_default());
                let produced64 = u64::try_from(produced).unwrap_or(u64::MAX);
                self.len = self.len.wrapping_add(produced64);
                if self.deflate.finish().is_ok() {
                    self.state = State::Trailer(0);
                }
                Ok((consumed, produced))
            },
            State::Extra(left) => {
                let n = input.len().min(usize::from(left));
                self.header_crc.update(input.get(..n).unwrap_or_default());
                let left = left.saturating_sub(u16::try_from(n).unwrap_or(u16::MAX));
                self.state = match left {
                    0 => self.after(FEXTRA),
                    left => State::Extra(left),
                };
                Ok((n, 0))
            },
            State::Name | State::Comment => {
                let (n, whole) = match input.iter().position(|&b| b == 0) {
                    Some(at) => (at.saturating_add(1), true),
                    None => (input.len(), false),
                };
                self.header_crc.update(input.get(..n).unwrap_or_default());
                if whole {
                    let done = if self.state == State::Name {
                        FNAME
                    } else {
                        FCOMMENT
                    };
                    self.state = self.after(done);
                }
                Ok((n, 0))
            },
            State::Fixed(_) | State::ExtraLen(_) | State::HeaderCrc(_) | State::Trailer(_) => {
                match input.first() {
                    Some(&byte) => self.byte(byte),
                    None => Ok((0, 0)),
                }
            },
        }
    }

    /// Reads `byte`, the next of a fixed-size field, and returns how many
    /// bytes it consumed: none if it ends the stream.
    fn byte(&mut self, byte: u8) -> Result<(usize, usize), Error> {
        match self.state {
            State::Fixed(0) if byte != 0x1F && self.members > 0 => {
                self.state = State::Ended;
                return Ok((0, 0));
            },
            State::Fixed(seen) => {
                let valid = match seen {
                    0 => byte == 0x1F,
                    1 => byte == 0x8B,
                    // Deflate, the one method defined.
                    2 => byte == 8,
                    3 => byte & RESERVED == 0,
                    _ => true,
                };
                if !valid {
                    return Err(corrupt());
                }
                if seen == 0 {
                    self.header_crc = Crc32::new();
                } else if seen == 3 {
                    self.flags = byte;
                }
                self.header_crc.update(&[byte]);
                self.state = match seen {
                    9 => self.after(0),
                    seen => State::Fixed(seen.saturating_add(1)),
                };
            },
            State::ExtraLen(seen) => {
                self.header_crc.update(&[byte]);
                self.state = match self.field(seen, byte) {
                    Some([a, b, ..]) => match u16::from_le_bytes([a, b]) {
                        0 => self.after(FEXTRA),
                        len => State::Extra(len),
                    },
                    None => State::ExtraLen(seen.saturating_add(1)),
                };
            },
            State::HeaderCrc(seen) => {
                self.state = match self.field(seen, byte) {
                    Some([a, b, ..]) => {
                        let stored = u16::from_le_bytes([a, b]);
                        if u32::from(stored) != self.header_crc.finish() & 0xFFFF {
                            return Err(corrupt());
                        }
                        self.body()
                    },
                    None => State::HeaderCrc(seen.saturating_add(1)),
                };
            },
            State::Trailer(seen) => {
                self.state = match self.field(seen, byte) {
                    Some([a, b, c, d, e, f, g, h]) => {
                        let crc = u32::from_le_bytes([a, b, c, d]);
                        let len = u32::from_le_bytes([e, f, g, h]);
                        // The length is stored modulo 2^32.
                        if crc != self.crc.finish() || u64::from(len) != self.len & 0xFFFF_FFFF {
                            return Err(corrupt());
                        }
                        self.members = self.members.saturating_add(1);
                        State::Fixed(0)
                    },
                    None => State::Trailer(seen.saturating_add(1)),
                };
            },
            State::Extra(_) | State::Name | State::Comment | State::Body | State::Ended => {
                return Err(corrupt());
            },
        }
        Ok((1, 0))
    }

    /// Stores `byte` at `seen` in the field in progress, and returns the
    /// field once whole: of 2 bytes, or 8 for the trailer.
    fn field(&mut self, seen: u8, byte: u8) -> Option<[u8; 8]> {
        if let Some(slot) = self.field.get_mut(usize::from(seen)) {
            *slot = byte;
        }
        let len = if matches!(self.state, State::Trailer(_)) {
            8
        } else {
            2
        };
        (seen.saturating_add(1) == len).then_some(self.field)
    }

    /// Returns the state after the header's part `done`, which is a flag or
    /// none for the fixed header, as the flags say which parts follow.
    fn after(&mut self, done: u8) -> State {
        let flags = self.flags;
        if done < FEXTRA && flags & FEXTRA != 0 {
            State::ExtraLen(0)
        } else if done < FNAME && flags & FNAME != 0 {
            State::Name
        } else if done < FCOMMENT && flags & FCOMMENT != 0 {
            State::Comment
        } else if flags & FHCRC != 0 {
            State::HeaderCrc(0)
        } else {
            self.body()
        }
    }

    /// Returns the state at the start of the member's data, ready to decode
    /// it.
    fn body(&mut self) -> State {
        self.deflate = decode_deflate::Decoder::new();
        self.crc = Crc32::new();
        self.len = 0;
        State::Body
    }
}

impl Default for Decoder {
    fn default() -> Self {
        Self::new()
    }
}

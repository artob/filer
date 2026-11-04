// This is free and unencumbered software released into the public domain.

use crate::{
    Error, corrupt,
    model::{Context, END, Model, Packet, Properties},
    range::{Dry, INIT, Range, Real},
    unsupported,
    window::Window,
};

/// The largest dictionary a decoder takes: twice that of `xz -9`, and as
/// large as zstd's window may be by default.
const MAX_DICTIONARY: u32 = 128 << 20;

/// The smallest dictionary, which a smaller one is taken as.
const MIN_DICTIONARY: u32 = 4096;

/// The most input one packet takes, at the worst.
const REQUIRED: usize = 20;

/// The most of a literal's probabilities' bits LZMA2 allows: `lc + lp`.
const LZMA2_LITERAL_BITS: u32 = 4;

/// What a decoder decodes, and where it is in it.
#[derive(Clone, Copy, Debug)]
enum Format {
    /// LZMA, with this many bytes left to decode, if known.
    Lzma {
        left: Option<u64>,
        ended: bool,
    },
    Lzma2(Lzma2),
}

/// Where an LZMA2 decoder is in its chunks.
#[derive(Clone, Copy, Debug)]
struct Lzma2 {
    chunk: Chunk,
    /// The chunk's control byte.
    control: u8,
    /// The least control byte the next LZMA chunk must have: one that
    /// resets the dictionary, at first, and after a dictionary reset, one
    /// that sets new properties.
    need: u8,
    /// What is left of the chunk: of its data, and of its packed bytes.
    unpacked: u32,
    packed: u32,
}

/// Where an LZMA2 decoder is in a chunk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Chunk {
    /// Before its control byte.
    Control,
    /// In its unpacked size, of 2 bytes, with this many read.
    Unpacked(u8),
    /// In its packed size, of 2 bytes, with this many read.
    Packed(u8),
    /// Before its properties.
    Properties,
    /// In its data, stored as it is.
    Stored,
    /// In its data, coded by LZMA.
    Coded,
    /// Past the end.
    End,
}

/// Why decoding coded data stopped.
enum Coded {
    /// It needs more input.
    More,
    /// It holds as much as it decodes at once.
    Full,
    /// The data ended.
    End,
}

/// A decoder of LZMA, as 7z and ZIP store it and `.lzma` files hold it,
/// or of LZMA2, as xz streams and 7z hold it.
///
/// LZMA is the Lempel-Ziv-Markov chain algorithm: literals and matches,
/// coded bit by bit by a range coder, with probabilities that adapt to
/// what it decodes. LZMA2 holds LZMA in chunks, each stored as it is or
/// coded, which may reset the dictionary, the probabilities, or the
/// properties.
///
/// It holds the dictionary, up to its size, though no more than the
/// stream decodes to, and as much again as 64 KiB, and of its input, at
/// most the 20 bytes of a packet that its input has not yet given it
/// whole.
///
/// # Examples
///
/// ```
/// use decode_lzma::Decoder;
///
/// // `printf hello | xz --format=lzma -0`: a 13-byte header, of the
/// // properties and an unknown size, then the stream.
/// let packed = [
///     0x5D, 0x00, 0x00, 0x04, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
///     0xFF, 0x00, 0x34, 0x19, 0x49, 0xDB, 0x85, 0x64, 0xF1, 0x93, 0xB1, 0xFF,
///     0xFB, 0x8F, 0xC0, 0x00,
/// ];
/// let properties = packed[..5].try_into().unwrap();
/// let mut decoder = Decoder::lzma(properties, None)?;
/// let mut output = [0; 8];
/// assert_eq!(decoder.decode(&packed[13..], &mut output)?, (15, 5));
/// assert_eq!(&output[..5], b"hello");
/// decoder.finish()?;
/// # Ok::<(), decode_lzma::Error>(())
/// ```
#[derive(Clone)]
pub struct Decoder {
    format: Format,
    model: Model,
    window: Window,
    /// The range decoder, once its data has begun.
    range: Option<Range>,
    /// Bytes taken from the input, too few for the next packet, or for
    /// the range decoder to begin.
    held: [u8; REQUIRED],
    held_len: usize,
}

impl Decoder {
    /// Returns a decoder of LZMA with `properties` as 7z and ZIP store
    /// them and `.lzma` files begin with: the byte of `lc`, `lp`, and `pb`,
    /// then the dictionary's size, little-endian. Of a known `size`, it
    /// ends there, or at an end marker that follows; of an unknown one, at
    /// its end marker.
    ///
    /// # Errors
    ///
    /// [`Corrupt`](Error::Corrupt) for invalid properties, and
    /// [`Unsupported`](Error::Unsupported) for a dictionary over 128 MiB
    /// that the size does not bound.
    pub fn lzma(properties: [u8; 5], size: Option<u64>) -> Result<Self, Error> {
        let [byte, dictionary @ ..] = properties;
        let properties = Properties::new(byte).ok_or_else(corrupt)?;
        let dictionary = u32::from_le_bytes(dictionary).max(MIN_DICTIONARY);
        // No more of the dictionary is used than the stream decodes to.
        let used = size.map_or(u64::from(dictionary), |size| {
            size.min(u64::from(dictionary))
        });
        if used > u64::from(MAX_DICTIONARY) {
            return Err(unsupported());
        }
        Ok(Self::new(
            Format::Lzma {
                left: size,
                ended: false,
            },
            properties,
            Window::new(dictionary, size),
        ))
    }

    /// Returns a decoder of LZMA2 whose dictionary's size is `property`,
    /// the one byte by which xz and 7z store it.
    ///
    /// # Errors
    ///
    /// [`Corrupt`](Error::Corrupt) for a property past 40, and
    /// [`Unsupported`](Error::Unsupported) for a dictionary over 128 MiB.
    pub fn lzma2(property: u8) -> Result<Self, Error> {
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
        let lzma2 = Lzma2 {
            chunk: Chunk::Control,
            control: 0,
            need: 0xE0,
            unpacked: 0,
            packed: 0,
        };
        // The first coded chunk sets the properties.
        let properties = Properties::new(0).ok_or_else(corrupt)?;
        Ok(Self::new(
            Format::Lzma2(lzma2),
            properties,
            Window::new(dictionary, None),
        ))
    }

    fn new(format: Format, properties: Properties, window: Window) -> Self {
        Self {
            format,
            model: Model::new(properties),
            window,
            range: None,
            held: [0; REQUIRED],
            held_len: 0,
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
        let (mut at, mut produced) = (0_usize, 0_usize);
        loop {
            let room = output.get_mut(produced..).unwrap_or_default();
            produced = produced.saturating_add(self.window.produce(room));
            if self.window.pending() > 0 {
                return Ok((at, produced));
            }
            let going = match self.format {
                Format::Lzma { .. } => self.step_lzma(input, &mut at)?,
                Format::Lzma2(_) => self.step_lzma2(input, &mut at)?,
            };
            if !going {
                let room = output.get_mut(produced..).unwrap_or_default();
                produced = produced.saturating_add(self.window.produce(room));
                return Ok((at, produced));
            }
        }
    }

    /// Checks that the stream has ended, and all that it decoded to has
    /// been produced.
    ///
    /// # Errors
    ///
    /// [`Corrupt`](Error::Corrupt) if it has not.
    pub fn finish(&self) -> Result<(), Error> {
        let ended = match self.format {
            Format::Lzma { ended, .. } => ended,
            Format::Lzma2(lzma2) => lzma2.chunk == Chunk::End,
        };
        if ended && self.window.pending() == 0 {
            Ok(())
        } else {
            Err(corrupt())
        }
    }

    /// Returns a decoder that goes on from where this one is: a copy of
    /// it, its dictionary included.
    #[must_use]
    pub fn checkpoint(&self) -> Option<Self> {
        Some(self.clone())
    }

    /// Returns how many of the bytes it has consumed a
    /// [`checkpoint`](Self::checkpoint) must be given again: none, as it
    /// holds them.
    #[must_use]
    pub const fn buffered(&self) -> usize {
        0
    }

    /// Decodes LZMA, and returns whether it can go on.
    fn step_lzma(&mut self, input: &[u8], at: &mut usize) -> Result<bool, Error> {
        let Format::Lzma { mut left, ended } = self.format else {
            return Err(corrupt());
        };
        if ended {
            return Ok(false);
        }
        let coded = self.coded(input, at, input.len(), &mut left, true);
        let ended = matches!(coded, Ok(Coded::End));
        self.format = Format::Lzma { left, ended };
        Ok(matches!(coded?, Coded::Full))
    }

    /// Decodes LZMA2, and returns whether it can go on.
    fn step_lzma2(&mut self, input: &[u8], at: &mut usize) -> Result<bool, Error> {
        let Format::Lzma2(mut lzma2) = self.format else {
            return Err(corrupt());
        };
        let going = self.chunk(&mut lzma2, input, at);
        self.format = Format::Lzma2(lzma2);
        going
    }

    /// Decodes what it can of an LZMA2 chunk, and returns whether it can
    /// go on.
    fn chunk(&mut self, lzma2: &mut Lzma2, input: &[u8], at: &mut usize) -> Result<bool, Error> {
        let rest = input.get(*at..).unwrap_or_default();
        match lzma2.chunk {
            Chunk::End => Ok(false),
            Chunk::Stored => {
                let n = rest
                    .len()
                    .min(usize::try_from(lzma2.unpacked).unwrap_or(usize::MAX));
                if n == 0 {
                    return Ok(false);
                }
                let written = self.window.write(rest.get(..n).unwrap_or_default());
                *at = at.saturating_add(written);
                let written = u32::try_from(written).map_err(|_| corrupt())?;
                lzma2.unpacked = lzma2.unpacked.saturating_sub(written);
                if lzma2.unpacked == 0 {
                    lzma2.chunk = Chunk::Control;
                }
                Ok(true)
            },
            Chunk::Coded => {
                let packed = usize::try_from(lzma2.packed).unwrap_or(usize::MAX);
                let end = at.saturating_add(packed).min(input.len());
                let before = *at;
                let mut left = Some(u64::from(lzma2.unpacked));
                let coded = self.coded(input, at, end, &mut left, false)?;
                let taken = u32::try_from(at.saturating_sub(before)).map_err(|_| corrupt())?;
                lzma2.packed = lzma2.packed.checked_sub(taken).ok_or_else(corrupt)?;
                lzma2.unpacked = left
                    .and_then(|left| u32::try_from(left).ok())
                    .ok_or_else(corrupt)?;
                match coded {
                    Coded::End if lzma2.packed == 0 => {
                        lzma2.chunk = Chunk::Control;
                        Ok(true)
                    },
                    Coded::Full => Ok(true),
                    Coded::More if lzma2.packed > 0 => Ok(false),
                    // Data that ends before or after its packed bytes do.
                    Coded::End | Coded::More => Err(corrupt()),
                }
            },
            Chunk::Control | Chunk::Unpacked(_) | Chunk::Packed(_) | Chunk::Properties => {
                let Some(&byte) = rest.first() else {
                    return Ok(false);
                };
                *at = at.saturating_add(1);
                self.header(lzma2, byte)?;
                Ok(true)
            },
        }
    }

    /// Reads `byte`, the next of an LZMA2 chunk's header.
    fn header(&mut self, lzma2: &mut Lzma2, byte: u8) -> Result<(), Error> {
        let stored = lzma2.control & 0x80 == 0;
        lzma2.chunk = match lzma2.chunk {
            Chunk::Control if byte == 0 => Chunk::End,
            Chunk::Control => {
                lzma2.control = byte;
                if byte & 0x80 == 0 {
                    // Stored, resetting the dictionary or not.
                    if byte == 1 {
                        lzma2.need = 0xC0;
                    } else if byte > 2 || lzma2.need == 0xE0 {
                        return Err(corrupt());
                    }
                    lzma2.unpacked = 0;
                } else {
                    if byte < lzma2.need {
                        return Err(corrupt());
                    }
                    lzma2.need = 0;
                    lzma2.unpacked = u32::from(byte & 0x1F) << 16;
                }
                Chunk::Unpacked(0)
            },
            Chunk::Unpacked(0) => {
                lzma2.unpacked |= u32::from(byte) << 8;
                Chunk::Unpacked(1)
            },
            Chunk::Unpacked(_) => {
                lzma2.unpacked = (lzma2.unpacked | u32::from(byte)).saturating_add(1);
                if !stored {
                    Chunk::Packed(0)
                } else {
                    if lzma2.control == 1 {
                        self.window.reset();
                    }
                    Chunk::Stored
                }
            },
            Chunk::Packed(0) => {
                lzma2.packed = u32::from(byte) << 8;
                Chunk::Packed(1)
            },
            Chunk::Packed(_) => {
                lzma2.packed = (lzma2.packed | u32::from(byte)).saturating_add(1);
                if lzma2.control & 0x40 != 0 {
                    Chunk::Properties
                } else {
                    self.begin(lzma2.control, None);
                    Chunk::Coded
                }
            },
            Chunk::Properties => {
                let properties = Properties::new(byte)
                    .filter(|properties| properties.literal_bits() <= LZMA2_LITERAL_BITS)
                    .ok_or_else(corrupt)?;
                self.begin(lzma2.control, Some(properties));
                Chunk::Coded
            },
            Chunk::Stored | Chunk::Coded | Chunk::End => return Err(corrupt()),
        };
        Ok(())
    }

    /// Begins an LZMA2 chunk of LZMA with `control`, and new `properties`,
    /// if it has them, which reset the model too.
    fn begin(&mut self, control: u8, properties: Option<Properties>) {
        if control >= 0xE0 {
            self.window.reset();
        }
        if let Some(properties) = properties {
            self.model.reset(properties);
        } else if control >= 0xA0 {
            self.model.reset(self.model.properties());
        }
        self.range = None;
        self.held_len = 0;
    }

    /// Decodes LZMA from `input` past `at` and before `end`, of `left`
    /// bytes, if known, ending there unless an end `marker` may follow,
    /// until it holds a chunk, the data ends, or it needs more input.
    fn coded(
        &mut self,
        input: &[u8],
        at: &mut usize,
        end: usize,
        left: &mut Option<u64>,
        marker: bool,
    ) -> Result<Coded, Error> {
        let input = input.get(..end).unwrap_or_default();
        let Some(mut range) = self.begin_range(input, at)? else {
            return Ok(Coded::More);
        };
        let coded = loop {
            if *left == Some(0) {
                if range.finished() {
                    break Coded::End;
                }
                if !marker {
                    return Err(corrupt());
                }
            }
            if !self.window.ready() {
                break Coded::Full;
            }
            // Straight from the input, while it surely holds a packet.
            if self.held_len == 0 && input.len().saturating_sub(*at) >= REQUIRED {
                let mut coder = Real {
                    range,
                    input,
                    at: *at,
                };
                let mut ended = false;
                // Past this, the input may not hold a packet.
                let limit = input.len().saturating_sub(REQUIRED);
                loop {
                    let context = self.context()?;
                    let packet = self.model.packet(&mut coder, context);
                    if self.apply(packet.ok_or_else(corrupt)?, left)? {
                        ended = true;
                        break;
                    }
                    if coder.at > limit || *left == Some(0) || !self.window.ready() {
                        break;
                    }
                }
                (range, *at) = (coder.range, coder.at);
                if ended {
                    break self.marked(range, *left, marker)?;
                }
                continue;
            }
            // A packet at a time, from the bytes held and the input, once
            // a dry run shows that they hold it.
            let held = self.held_len;
            let mut bytes = self.held;
            let fresh = REQUIRED
                .saturating_sub(held)
                .min(input.len().saturating_sub(*at));
            let fresh_bytes = input.get(*at..at.saturating_add(fresh));
            let all = held.saturating_add(fresh);
            if let (Some(to), Some(from)) = (bytes.get_mut(held..all), fresh_bytes) {
                to.copy_from_slice(from);
            }
            let bytes = bytes.get(..all).unwrap_or_default();
            let context = self.context()?;
            let mut dry = Dry {
                range,
                input: bytes,
                at: 0,
            };
            if self.model.packet(&mut dry, context).is_none() {
                // Too few: hold them all, for more to follow.
                if all >= REQUIRED {
                    return Err(corrupt());
                }
                self.hold(bytes);
                *at = at.saturating_add(fresh);
                break Coded::More;
            }
            let mut coder = Real {
                range,
                input: bytes,
                at: 0,
            };
            let packet = self.model.packet(&mut coder, context).ok_or_else(corrupt)?;
            range = coder.range;
            // What it took past the bytes held came from the input.
            match held.checked_sub(coder.at) {
                Some(_) => self.hold(bytes.get(coder.at..held).unwrap_or_default()),
                None => {
                    self.held_len = 0;
                    *at = at.saturating_add(coder.at.saturating_sub(held));
                },
            }
            if self.apply(packet, left)? {
                break self.marked(range, *left, marker)?;
            }
        };
        self.range = Some(range);
        Ok(coded)
    }

    /// Returns the range decoder at the start of the data, from the bytes
    /// held and `input` past `at`, or `None` until there are enough.
    fn begin_range(&mut self, input: &[u8], at: &mut usize) -> Result<Option<Range>, Error> {
        if let Some(range) = self.range {
            return Ok(Some(range));
        }
        let held = self.held_len;
        let fresh = INIT
            .saturating_sub(held)
            .min(input.len().saturating_sub(*at));
        let mut bytes = self.held;
        let all = held.saturating_add(fresh);
        if let (Some(to), Some(from)) = (
            bytes.get_mut(held..all),
            input.get(*at..at.saturating_add(fresh)),
        ) {
            to.copy_from_slice(from);
        }
        *at = at.saturating_add(fresh);
        self.held = bytes;
        self.held_len = all;
        // A zero begins the data.
        if bytes.first().is_some_and(|&byte| byte != 0) && all > 0 {
            return Err(corrupt());
        }
        let Some(&first) = bytes.first_chunk::<INIT>() else {
            return Err(corrupt());
        };
        if all < INIT {
            return Ok(None);
        }
        self.held_len = 0;
        let range = Range::new(first).ok_or_else(corrupt)?;
        self.range = Some(range);
        Ok(Some(range))
    }

    /// Holds `bytes`, too few for the next packet.
    fn hold(&mut self, bytes: &[u8]) {
        let mut held = [0; REQUIRED];
        if let Some(to) = held.get_mut(..bytes.len()) {
            to.copy_from_slice(bytes);
        }
        (self.held, self.held_len) = (held, bytes.len().min(REQUIRED));
    }

    /// Checks an end marker, which `range` follows, with `left` bytes left
    /// to decode, if known, and whether the data may have one.
    fn marked(&self, range: Range, left: Option<u64>, marker: bool) -> Result<Coded, Error> {
        if marker && range.finished() && left.is_none_or(|left| left == 0) {
            Ok(Coded::End)
        } else {
            Err(corrupt())
        }
    }

    /// Returns the context the next packet is decoded in.
    fn context(&self) -> Result<Context, Error> {
        let processed = self.window.processed;
        let previous = if processed == 0 {
            0
        } else {
            u32::from(self.window.last)
        };
        let matched = if self.model.after_match() {
            Some(u32::from(
                self.window.byte(self.rep0()?).ok_or_else(corrupt)?,
            ))
        } else {
            None
        };
        Ok(Context {
            position: u32::try_from(processed & u64::from(u32::MAX)).unwrap_or_default(),
            previous,
            matched,
        })
    }

    /// Returns the latest match's distance, from 1.
    fn rep0(&self) -> Result<usize, Error> {
        let [rep0, ..] = self.model.reps;
        usize::try_from(rep0)
            .ok()
            .and_then(|rep0| rep0.checked_add(1))
            .ok_or_else(corrupt)
    }

    /// Applies `packet`, of `left` bytes left, if known, and returns
    /// whether it ends the data.
    #[inline(always)]
    fn apply(&mut self, packet: Packet, left: &mut Option<u64>) -> Result<bool, Error> {
        match packet {
            Packet::Literal(byte) => {
                take(left, 1)?;
                self.window.put(byte);
                self.model.literal_done();
            },
            Packet::ShortRep => {
                if self.window.processed == 0 {
                    return Err(corrupt());
                }
                take(left, 1)?;
                let byte = self.window.byte(self.rep0()?).ok_or_else(corrupt)?;
                self.window.put(byte);
                self.model.short_rep_done();
            },
            Packet::Rep { index, length } => {
                if self.window.processed == 0 {
                    return Err(corrupt());
                }
                self.model.rep_done(index);
                self.copy(length, left)?;
            },
            Packet::Match { length, distance } => {
                self.model.match_done(distance);
                if distance == END {
                    return Ok(true);
                }
                if *left == Some(0) || !self.window.reaches(distance) {
                    return Err(corrupt());
                }
                self.copy(length, left)?;
            },
        }
        Ok(false)
    }

    /// Copies a match of `length` at the latest distance.
    fn copy(&mut self, length: u32, left: &mut Option<u64>) -> Result<(), Error> {
        take(left, u64::from(length))?;
        let length = usize::try_from(length).map_err(|_| corrupt())?;
        self.window.copy(self.rep0()?, length);
        Ok(())
    }
}

/// Takes `n` bytes from what is `left`, if known, which must hold them.
fn take(left: &mut Option<u64>, n: u64) -> Result<(), Error> {
    if let Some(left) = left {
        *left = left.checked_sub(n).ok_or_else(corrupt)?;
    }
    Ok(())
}

impl core::fmt::Debug for Decoder {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Decoder")
            .field("format", &self.format)
            .field("pending", &self.window.pending())
            .finish_non_exhaustive()
    }
}

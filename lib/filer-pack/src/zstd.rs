// This is free and unencumbered software released into the public domain.

use crate::Decoder;
use alloc::{boxed::Box, vec::Vec};
use core::ops::RangeInclusive;
use filer_api::{Error, ErrorKind, OpKind};
use ruzstd::decoding::{FrameDecoder, errors::FrameDecoderError};

#[cfg_attr(
    not(test),
    allow(dead_code, reason = "`Zstd` decodes with it from PACK-24")
)]
mod frames;
mod xxh64;

/// A frame's magic number, and the range of skippable frames' (RFC 8878
/// §3.1).
const MAGIC: u32 = 0xFD2F_B528;
const SKIPPABLE: RangeInclusive<u32> = 0x184D_2A50..=0x184D_2A5F;

/// The most a block may hold or decode to (RFC 8878 §3.1.1.2.3).
const MAX_BLOCK: usize = 128 * 1024;

/// Where a [`Zstd`] decoder is in its stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    /// Before a frame, or in its header, gathered so far.
    Frame,
    /// In a skippable frame, with this many bytes left.
    Skip(u32),
    /// Before a block of a frame, or in it, gathered so far; whether the
    /// frame ends with a checksum.
    Block { checksum: bool },
    /// Past a frame's last block, with what it decoded to left to produce,
    /// and then its checksum, if any, to check.
    Last { checksum: bool },
}

/// A decoder of zstd (RFC 8878), over `ruzstd`: frames, one after another,
/// each checked against its checksum, if it has one, as it ends; skippable
/// frames are read past.
///
/// It gathers each frame header and block whole before `ruzstd` decodes
/// it, so it holds at most a block of packed bytes, and `ruzstd` a frame's
/// window and a block of what they decode to. A frame whose window passes
/// 128 MiB, the most zstd itself decodes by default, or that needs a
/// dictionary is [`Unsupported`](ErrorKind::Unsupported). What follows the
/// last frame must be another, as zstd requires.
///
/// # Examples
///
/// ```
/// use filer_pack::{Decoder, Zstd};
///
/// // `printf hello | zstd`.
/// let packed = [
///     0x28, 0xB5, 0x2F, 0xFD, 0x04, 0x58, 0x29, 0x00, 0x00, 0x68, 0x65, 0x6C,
///     0x6C, 0x6F, 0xA3, 0x6D, 0x9F, 0x88,
/// ];
/// let mut zstd = Zstd::new();
/// let mut output = [0; 8];
/// assert_eq!(zstd.decode(&packed, &mut output)?, (18, 5));
/// assert_eq!(&output[..5], b"hello");
/// zstd.finish()?;
/// # Ok::<(), filer_api::Error>(())
/// ```
pub struct Zstd {
    decoder: Box<FrameDecoder>,
    /// The packed bytes of a frame header or block, gathered until whole.
    unit: Vec<u8>,
    state: State,
    /// The frames decoded whole, skippable ones included.
    frames: u64,
}

impl Zstd {
    /// Returns a decoder at the start of a stream.
    #[must_use]
    pub fn new() -> Self {
        Self {
            decoder: Box::new(FrameDecoder::new()),
            unit: Vec::new(),
            state: State::Frame,
            frames: 0,
        }
    }

    /// Makes what progress it can from `input` into `output`, returning how
    /// much of each it consumed and produced, or nothing if it can make
    /// none.
    fn step(&mut self, input: &[u8], output: &mut [u8]) -> Result<Option<(usize, usize)>, Error> {
        match self.state {
            State::Skip(left) => {
                let n = input.len().min(usize::try_from(left).unwrap_or(usize::MAX));
                match left.saturating_sub(u32::try_from(n).unwrap_or(u32::MAX)) {
                    0 => self.ended(),
                    left => self.state = State::Skip(left),
                }
                Ok((n > 0).then_some((n, 0)))
            },
            State::Block { .. } | State::Last { .. } if self.decoder.can_collect() > 0 => {
                if output.is_empty() {
                    return Ok(None);
                }
                let (_, produced) = self.decoder.decode_from_to(&[], output).map_err(error)?;
                Ok((produced > 0).then_some((0, produced)))
            },
            State::Last { checksum } => {
                let stored = self.decoder.get_checksum_from_data();
                if checksum && stored != self.decoder.get_calculated_checksum() {
                    return Err(corrupt());
                }
                self.ended();
                Ok(Some((0, 0)))
            },
            State::Frame | State::Block { .. } => {
                let need = self.need()?;
                match need.checked_sub(self.unit.len()) {
                    Some(0) | None => self.unit(output).map(Some),
                    Some(more) => {
                        let taken = input.get(..more.min(input.len())).unwrap_or_default();
                        self.unit.extend_from_slice(taken);
                        Ok((!taken.is_empty()).then_some((taken.len(), 0)))
                    },
                }
            },
        }
    }

    /// Returns how many bytes the frame header or block in progress takes
    /// in all, as far as what is gathered of it tells.
    fn need(&self) -> Result<usize, Error> {
        match self.state {
            State::Frame => {
                let Some(magic) = self.unit.first_chunk().copied().map(u32::from_le_bytes) else {
                    return Ok(4);
                };
                if SKIPPABLE.contains(&magic) {
                    return Ok(8);
                }
                if magic != MAGIC {
                    return Err(corrupt());
                }
                let Some(&descriptor) = self.unit.get(4) else {
                    return Ok(5);
                };
                // A decoder must refuse the reserved bit, which `ruzstd`
                // does not check.
                if descriptor & 0x08 != 0 {
                    return Err(corrupt());
                }
                // A single segment has no window descriptor, and a size.
                let single = descriptor & 0x20 != 0;
                let dictionary = match descriptor & 0x03 {
                    0 => 0,
                    1 => 1,
                    2 => 2,
                    _ => 4,
                };
                let size = match descriptor >> 6 {
                    0 => usize::from(single),
                    1 => 2,
                    2 => 4,
                    _ => 8,
                };
                Ok(5_usize
                    .saturating_add(usize::from(!single))
                    .saturating_add(dictionary)
                    .saturating_add(size))
            },
            State::Block { checksum } => {
                let Some(&[a, b, c]) = self.unit.first_chunk() else {
                    return Ok(3);
                };
                let header = u32::from_le_bytes([a, b, c, 0]);
                let size = usize::try_from(header >> 3).unwrap_or(usize::MAX);
                if size > MAX_BLOCK {
                    return Err(corrupt());
                }
                // Raw and compressed blocks hold their size, and RLE
                // blocks one byte, repeated; the fourth type is reserved.
                let content = match (header >> 1) & 0x03 {
                    0 | 2 => size,
                    1 => 1,
                    _ => return Err(corrupt()),
                };
                let last = header & 1 != 0;
                let trailer = if last && checksum { 4 } else { 0 };
                Ok(3_usize.saturating_add(content).saturating_add(trailer))
            },
            State::Skip(_) | State::Last { .. } => Ok(0),
        }
    }

    /// Reads the frame header or block gathered, decoding a block into
    /// `output`, and returns how much of each it consumed and produced.
    fn unit(&mut self, output: &mut [u8]) -> Result<(usize, usize), Error> {
        let mut produced = 0;
        match self.state {
            State::Frame => match self.unit.first_chunk().copied().map(u32::from_le_bytes) {
                Some(magic) if SKIPPABLE.contains(&magic) => {
                    let len = self.unit.get(4..8).and_then(|len| len.try_into().ok());
                    match len.map(u32::from_le_bytes) {
                        Some(0) => self.ended(),
                        Some(len) => self.state = State::Skip(len),
                        None => return Err(corrupt()),
                    }
                },
                _ => {
                    // A new decoder, whose buffer grows only as the frame
                    // decodes, as `reset` would allocate its whole window.
                    *self.decoder = FrameDecoder::new();
                    self.decoder.reset(self.unit.as_slice()).map_err(error)?;
                    let descriptor = self.unit.get(4).copied().unwrap_or_default();
                    self.state = State::Block {
                        checksum: descriptor & 0x04 != 0,
                    };
                },
            },
            State::Block { checksum } => {
                let (read, written) = self
                    .decoder
                    .decode_from_to(&self.unit, output)
                    .map_err(error)?;
                if read != self.unit.len() || written > output.len() {
                    return Err(corrupt());
                }
                produced = written;
                if self.unit.first().is_some_and(|header| header & 1 != 0) {
                    self.state = State::Last { checksum };
                }
            },
            State::Skip(_) | State::Last { .. } => return Err(corrupt()),
        }
        self.unit.clear();
        Ok((0, produced))
    }

    /// Returns whether it is between frames, past at least one.
    fn between(&self) -> bool {
        self.frames > 0 && self.state == State::Frame && self.unit.is_empty()
    }

    /// Ends the frame in progress.
    fn ended(&mut self) {
        self.frames = self.frames.saturating_add(1);
        self.state = State::Frame;
    }
}

impl Default for Zstd {
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Debug for Zstd {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Zstd")
            .field("state", &self.state)
            .field("frames", &self.frames)
            .finish_non_exhaustive()
    }
}

impl Decoder for Zstd {
    fn decode(&mut self, input: &[u8], output: &mut [u8]) -> Result<(usize, usize), Error> {
        let (mut consumed, mut produced) = (0_usize, 0_usize);
        let frames = self.frames;
        loop {
            let input = input.get(consumed..).unwrap_or_default();
            let output = output.get_mut(produced..).unwrap_or_default();
            let Some((c, p)) = self.step(input, output)? else {
                return Ok((consumed, produced));
            };
            consumed = consumed.saturating_add(c);
            produced = produced.saturating_add(p);
            // Stop between frames, where a checkpoint may be, once there
            // is progress to report.
            if self.frames != frames && (consumed > 0 || produced > 0) {
                return Ok((consumed, produced));
            }
        }
    }

    fn finish(&mut self) -> Result<(), Error> {
        if self.between() {
            Ok(())
        } else {
            Err(corrupt())
        }
    }

    fn checkpoint(&self) -> Option<Box<dyn Decoder>> {
        if !self.between() {
            return None;
        }
        // Frames decode alone, so a fresh decoder goes on from here.
        let fresh: Box<dyn Decoder> = Box::new(Self {
            frames: self.frames,
            ..Self::new()
        });
        Some(fresh)
    }
}

fn corrupt() -> Error {
    Error::new(ErrorKind::Corrupt, OpKind::Read)
}

/// Returns `ruzstd`'s `error` as one of ours: a window too large, or a
/// dictionary, is unsupported, and anything else corrupt.
fn error(error: FrameDecoderError) -> Error {
    match error {
        FrameDecoderError::WindowSizeTooBig { .. } | FrameDecoderError::DictNotProvided { .. } => {
            Error::new(ErrorKind::Unsupported, OpKind::Read)
        },
        _ => corrupt(),
    }
}

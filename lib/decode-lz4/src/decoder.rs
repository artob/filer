// This is free and unencumbered software released into the public domain.

use crate::{Error, block, corrupt, unsupported};
use alloc::vec::Vec;
use compute_xxhash::{Xxh32, xxh32};
use core::ops::RangeInclusive;

/// The magic numbers of a frame, of a legacy frame, and the range of
/// skippable frames' (the LZ4 Frame Format Description).
const MAGIC: u32 = 0x184D_2204;
const LEGACY: u32 = 0x184C_2102;
const SKIPPABLE: RangeInclusive<u32> = 0x184D_2A50..=0x184D_2A5F;

/// How far back a frame's linked blocks may copy from.
const HISTORY: usize = 64 * 1024;

/// What a legacy frame's blocks decode to at most, and what they hold at
/// most: as much, as LZ4's worst case expands it.
const LEGACY_BLOCK: usize = 8 << 20;
const LEGACY_PACKED: usize = LEGACY_BLOCK + LEGACY_BLOCK / 255 + 16;

/// A block size's flag that the block is stored as is.
const STORED: u32 = 0x8000_0000;

/// A frame's descriptor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Header {
    /// Whether each block decodes alone, or copies from those before it.
    independent: bool,
    block_checksum: bool,
    size: Option<u64>,
    checksum: bool,
    /// The most a block may hold or decode to.
    max: usize,
}

impl Header {
    /// The descriptor of a legacy frame, which has none: its blocks are
    /// independent, unchecked, and of 8 MiB.
    const LEGACY: Self = Self {
        independent: true,
        block_checksum: false,
        size: None,
        checksum: false,
        max: LEGACY_BLOCK,
    };

    /// Returns how long a frame's header is, magic number included, from
    /// its descriptor's flags.
    fn len(flags: u8) -> usize {
        let size = if flags & 0x08 != 0 { 8 } else { 0 };
        let dictionary = if flags & 0x01 != 0 { 4 } else { 0 };
        7_usize.saturating_add(size).saturating_add(dictionary)
    }

    /// Parses a whole header, magic number included.
    fn parse(header: &[u8]) -> Result<Self, Error> {
        let [_, _, _, _, descriptor @ .., checksum] = header else {
            return Err(corrupt());
        };
        let &[flags, bd, ref rest @ ..] = descriptor else {
            return Err(corrupt());
        };
        if header.len() != Self::len(flags) {
            return Err(corrupt());
        }
        // The second byte of the descriptor's XXH32.
        let [_, check, ..] = xxh32(descriptor, 0).to_le_bytes();
        // Version 1, and the reserved bits clear.
        if check != *checksum || flags >> 6 != 1 || flags & 0x02 != 0 || bd & 0x8F != 0 {
            return Err(corrupt());
        }
        let max = match bd >> 4 {
            4 => 64 << 10,
            5 => 256 << 10,
            6 => 1 << 20,
            7 => 4 << 20,
            _ => return Err(corrupt()),
        };
        if flags & 0x01 != 0 {
            return Err(unsupported());
        }
        let size = match rest.first_chunk() {
            Some(&size) if flags & 0x08 != 0 => Some(u64::from_le_bytes(size)),
            _ => None,
        };
        Ok(Self {
            independent: flags & 0x20 != 0,
            block_checksum: flags & 0x10 != 0,
            size,
            checksum: flags & 0x04 != 0,
            max,
        })
    }
}

/// What a frame has decoded: its last block, those of its bytes not yet
/// produced, and before them, if its blocks are linked, what that block
/// may copy from.
#[derive(Clone, Debug, Default)]
struct Window {
    /// Room for a block after what comes before it, of which `end` bytes
    /// are decoded and `produced` produced.
    buf: Vec<u8>,
    end: usize,
    produced: usize,
}

impl Window {
    /// Returns room for a block of up to `max` bytes, after what a linked
    /// block copies from, or none, which it moves to the start.
    fn room(&mut self, linked: bool, max: usize) -> &mut [u8] {
        let kept = if linked { self.end.min(HISTORY) } else { 0 };
        let from = self.end.saturating_sub(kept);
        self.buf.copy_within(from..self.end, 0);
        (self.end, self.produced) = (kept, kept);
        let len = kept.saturating_add(max);
        if self.buf.len() < len {
            self.buf.resize(len, 0);
        }
        self.buf.get_mut(..len).unwrap_or_default()
    }

    /// Produces what it can into `output`, and returns how much.
    fn produce(&mut self, output: &mut [u8]) -> usize {
        let pending = self.buf.get(self.produced..self.end).unwrap_or_default();
        let n = pending.len().min(output.len());
        if let (Some(to), Some(from)) = (output.get_mut(..n), pending.get(..n)) {
            to.copy_from_slice(from);
        }
        self.produced = self.produced.saturating_add(n);
        n
    }

    fn pending(&self) -> bool {
        self.produced < self.end
    }

    /// Returns a copy of what the next block may copy from, if `linked`,
    /// and what is yet to be produced.
    fn copy(&self, linked: bool) -> Self {
        let back = if linked {
            self.end.saturating_sub(HISTORY)
        } else {
            self.end
        };
        let from = back.min(self.produced);
        let buf = self.buf.get(from..self.end).unwrap_or_default().to_vec();
        Self {
            end: buf.len(),
            produced: self.produced.saturating_sub(from),
            buf,
        }
    }
}

/// A frame being decoded.
#[derive(Clone, Debug)]
struct Frame {
    header: Header,
    window: Window,
    /// What its blocks have decoded to so far, hashed and counted.
    hash: Xxh32,
    decoded: u64,
}

impl Frame {
    fn new(header: Header) -> Self {
        Self {
            header,
            window: Window::default(),
            hash: Xxh32::new(0),
            decoded: 0,
        }
    }

    /// Returns a copy that decodes on as it would, holding only what it
    /// needs of its window.
    fn copy(&self) -> Self {
        Self {
            window: self.window.copy(!self.header.independent),
            hash: self.hash.clone(),
            ..*self
        }
    }
}

/// Where a [`Decoder`] is in its stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    /// Before a frame, or in its header, gathered so far.
    Frame,
    /// In a skippable frame, with this many bytes left.
    Skip(u32),
    /// Before a block of the frame, or its end mark, or in either,
    /// gathered so far.
    Block,
    /// Before a block of a legacy frame, or in it, gathered so far.
    Legacy,
    /// Past the frame's end, with what it decoded to left to produce.
    Last,
}

/// A decoder of LZ4 frames (the LZ4 Frame Format Description), one after
/// another: each checked against its descriptor's checksum, and those of
/// its blocks, content, and size, if it has them; and legacy frames, as
/// `lz4 -l` writes them and the Linux kernel reads them from initramfs
/// images; skippable frames are read past.
///
/// It gathers each frame header and block whole before it decodes it,
/// unless the input holds it whole, so it holds at most a block of packed
/// bytes, 4 MiB, or 8 MiB in a legacy frame, and a block of what they
/// decode to, after the 64 KiB it may copy from. A frame that needs a
/// dictionary is [`Unsupported`](Error::Unsupported). What follows a
/// frame must be another, as `lz4` requires, but for a legacy frame,
/// which ends where the input does, or at four zero bytes, as the Linux
/// kernel's do.
///
/// # Examples
///
/// ```
/// use decode_lz4::Decoder;
///
/// // `printf hello | lz4`.
/// let packed = [
///     0x04, 0x22, 0x4D, 0x18, 0x64, 0x40, 0xA7, 0x05, 0x00, 0x00, 0x80, 0x68,
///     0x65, 0x6C, 0x6C, 0x6F, 0x00, 0x00, 0x00, 0x00, 0xF9, 0x77, 0x00, 0xFB,
/// ];
/// let mut decoder = Decoder::new();
/// let mut output = [0; 8];
/// assert_eq!(decoder.decode(&packed, &mut output)?, (24, 5));
/// assert_eq!(&output[..5], b"hello");
/// decoder.finish()?;
/// # Ok::<(), decode_lz4::Error>(())
/// ```
#[derive(Clone)]
pub struct Decoder {
    /// The packed bytes of a frame header or block, gathered until whole.
    unit: Vec<u8>,
    state: State,
    frame: Option<Frame>,
    /// The frames decoded whole, skippable ones included.
    frames: u64,
}

impl Decoder {
    /// Returns a decoder at the start of a stream.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            unit: Vec::new(),
            state: State::Frame,
            frame: None,
            frames: 0,
        }
    }

    /// Decodes what it can of `input` into `output`, and returns how many
    /// bytes of each it consumed and produced.
    ///
    /// Given an empty `input`, it takes the input to have ended. It returns
    /// neither consumed nor produced once it can go no further: needing
    /// more input, or more room, or past the end of the stream, which
    /// [`finish`](Self::finish) then tells apart. It returns at the end of
    /// each frame.
    ///
    /// # Errors
    ///
    /// [`Corrupt`](Error::Corrupt) for a malformed stream, and
    /// [`Unsupported`](Error::Unsupported) for a frame that needs a
    /// dictionary. The decoder is not to be used after an error.
    pub fn decode(&mut self, input: &[u8], output: &mut [u8]) -> Result<(usize, usize), Error> {
        let (mut consumed, mut produced) = (0_usize, 0_usize);
        let frames = self.frames;
        loop {
            if let Some(frame) = &mut self.frame {
                let room = output.get_mut(produced..).unwrap_or_default();
                produced = produced.saturating_add(frame.window.produce(room));
                if frame.window.pending() {
                    return Ok((consumed, produced));
                }
                if self.state == State::Last {
                    self.ended();
                }
            }
            // Stop between frames, once there is progress to report.
            if self.frames != frames && (consumed > 0 || produced > 0) {
                return Ok((consumed, produced));
            }
            let Some(n) = self.step(input.get(consumed..).unwrap_or_default())? else {
                return Ok((consumed, produced));
            };
            consumed = consumed.saturating_add(n);
        }
    }

    /// Checks that the stream has ended where a stream may: past a whole
    /// frame, or between a legacy frame's blocks, and everything it
    /// decoded to produced.
    ///
    /// # Errors
    ///
    /// [`Corrupt`](Error::Corrupt) if it ended anywhere else, or before
    /// any frame.
    pub fn finish(&self) -> Result<(), Error> {
        let pending = self
            .frame
            .as_ref()
            .is_some_and(|frame| frame.window.pending());
        let between = match self.state {
            State::Frame => self.frames > 0,
            State::Legacy => true,
            _ => false,
        };
        if between && self.unit.is_empty() && !pending {
            Ok(())
        } else {
            Err(corrupt())
        }
    }

    /// Returns a decoder that goes on from where this one is: given the
    /// input from where this one has consumed to, less what it has
    /// [`buffered`](Self::buffered), it decodes what this one would. It is
    /// a copy of this one's state: the 64 KiB a frame's next block may copy
    /// from, if its blocks are linked, and what is not yet produced, which
    /// [`footprint`](Self::footprint) measures. It is always offered, so
    /// `Option` only for the shape that the `decode-*` crates share.
    #[must_use]
    pub fn checkpoint(&self) -> Option<Self> {
        Some(Self {
            unit: Vec::new(),
            state: self.state,
            frame: self.frame.as_ref().map(Frame::copy),
            frames: self.frames,
        })
    }

    /// Returns how many of the bytes it has consumed it holds, gathered
    /// toward a frame header or block that is not yet whole.
    #[must_use]
    pub fn buffered(&self) -> usize {
        self.unit.len()
    }

    /// Returns about how many bytes of memory it holds: mostly room for a
    /// block, gathered and decoded.
    #[must_use]
    pub fn footprint(&self) -> usize {
        let window = self
            .frame
            .as_ref()
            .map_or(0, |frame| frame.window.buf.capacity());
        size_of::<Self>()
            .saturating_add(size_of::<Frame>())
            .saturating_add(window)
            .saturating_add(self.unit.capacity())
    }

    /// Takes what it can of `input`, decoding a frame header or block once
    /// whole, and returns how much it consumed, or nothing if it can make
    /// no progress.
    fn step(&mut self, input: &[u8]) -> Result<Option<usize>, Error> {
        match self.state {
            State::Skip(left) => {
                let n = input.len().min(usize::try_from(left).unwrap_or(usize::MAX));
                match left.saturating_sub(u32::try_from(n).unwrap_or(u32::MAX)) {
                    0 => self.ended(),
                    left => self.state = State::Skip(left),
                }
                Ok((n > 0).then_some(n))
            },
            State::Last => Ok(None),
            State::Legacy if self.magic_follows(input) => {
                // The next frame's, which ends the legacy frame; what of
                // it is gathered is the start of that frame's header.
                self.ended();
                Ok(Some(0))
            },
            State::Frame | State::Block | State::Legacy => {
                // Straight from the input, if it holds the unit whole.
                if self.unit.is_empty() {
                    let need = self.need(input)?;
                    if let Some(unit) = input.get(..need) {
                        self.unit(unit)?;
                        return Ok(Some(need));
                    }
                }
                let need = self.need(&self.unit)?;
                match need.checked_sub(self.unit.len()) {
                    Some(0) | None => {
                        let unit = core::mem::take(&mut self.unit);
                        let parsed = self.unit(&unit);
                        self.unit = unit;
                        self.unit.clear();
                        parsed.map(|()| Some(0))
                    },
                    Some(more) => {
                        let taken = input.get(..more.min(input.len())).unwrap_or_default();
                        self.unit.extend_from_slice(taken);
                        Ok((!taken.is_empty()).then_some(taken.len()))
                    },
                }
            },
        }
    }

    /// Returns whether a frame's magic number follows, where a legacy
    /// frame's next block size would be, in what is gathered and then
    /// `input`.
    fn magic_follows(&self, input: &[u8]) -> bool {
        let mut head = [0; 4];
        let gathered = self.unit.len().min(4);
        let rest = input.get(..4_usize.saturating_sub(gathered).min(input.len()));
        let (Some(first), Some(rest)) = (self.unit.get(..gathered), rest) else {
            return false;
        };
        if first.len().saturating_add(rest.len()) < 4 {
            return false;
        }
        for (slot, &byte) in head.iter_mut().zip(first.iter().chain(rest)) {
            *slot = byte;
        }
        let magic = u32::from_le_bytes(head);
        magic == MAGIC || magic == LEGACY || SKIPPABLE.contains(&magic)
    }

    /// Returns how many bytes the frame header or block that `head` begins
    /// takes in all, as far as `head` tells.
    fn need(&self, head: &[u8]) -> Result<usize, Error> {
        let Some(word) = head.first_chunk().copied().map(u32::from_le_bytes) else {
            return Ok(4);
        };
        match (self.state, &self.frame) {
            (State::Frame, _) => match word {
                MAGIC => Ok(head.get(4).map_or(5, |&flags| Header::len(flags))),
                LEGACY => Ok(4),
                magic if SKIPPABLE.contains(&magic) => Ok(8),
                _ => Err(corrupt()),
            },
            // The end mark, and the content's checksum.
            (State::Block, Some(frame)) if word == 0 => {
                Ok(if frame.header.checksum { 8 } else { 4 })
            },
            (State::Block, Some(frame)) => {
                let size = usize::try_from(word & !STORED).unwrap_or(usize::MAX);
                if size > frame.header.max {
                    return Err(corrupt());
                }
                let checksum = if frame.header.block_checksum { 4 } else { 0 };
                Ok(4_usize.saturating_add(size).saturating_add(checksum))
            },
            // The Linux kernel's end mark.
            (State::Legacy, _) if word == 0 => Ok(4),
            (State::Legacy, _) => {
                let size = usize::try_from(word).unwrap_or(usize::MAX);
                if size > LEGACY_PACKED {
                    return Err(corrupt());
                }
                Ok(4_usize.saturating_add(size))
            },
            _ => Err(corrupt()),
        }
    }

    /// Reads a whole frame header or block.
    fn unit(&mut self, unit: &[u8]) -> Result<(), Error> {
        match self.state {
            State::Block | State::Legacy => return self.block(unit),
            _ => {},
        }
        let magic = unit.first_chunk().copied().map(u32::from_le_bytes);
        match magic {
            Some(LEGACY) => {
                self.frame = Some(Frame::new(Header::LEGACY));
                self.state = State::Legacy;
            },
            Some(magic) if SKIPPABLE.contains(&magic) => {
                let len = unit.get(4..8).and_then(|len| len.try_into().ok());
                match len.map(u32::from_le_bytes) {
                    Some(0) => self.ended(),
                    Some(len) => self.state = State::Skip(len),
                    None => return Err(corrupt()),
                }
            },
            _ => {
                self.frame = Some(Frame::new(Header::parse(unit)?));
                self.state = State::Block;
            },
        }
        Ok(())
    }

    /// Decodes a whole block, its size included, onto the frame's window,
    /// or reads the frame's end.
    fn block(&mut self, unit: &[u8]) -> Result<(), Error> {
        let frame = self.frame.as_mut().ok_or_else(corrupt)?;
        let Some((&word, rest)) = unit.split_first_chunk::<4>() else {
            return Err(corrupt());
        };
        let word = u32::from_le_bytes(word);
        if word == 0 {
            if self.state == State::Legacy {
                self.ended();
                return Ok(());
            }
            if frame.header.size.is_some_and(|size| frame.decoded != size) {
                return Err(corrupt());
            }
            let digest = frame.hash.finish().to_le_bytes();
            if frame.header.checksum && rest != digest {
                return Err(corrupt());
            }
            self.state = State::Last;
            return Ok(());
        }
        let (data, checksum) = if frame.header.block_checksum {
            rest.split_last_chunk::<4>().ok_or_else(corrupt)?
        } else {
            (rest, &[0; 4])
        };
        if frame.header.block_checksum && xxh32(data, 0).to_le_bytes() != *checksum {
            return Err(corrupt());
        }
        let (linked, max) = (!frame.header.independent, frame.header.max);
        let start = if linked {
            frame.window.end.min(HISTORY)
        } else {
            0
        };
        let room = frame.window.room(linked, max);
        let end = if self.state == State::Block && word & STORED != 0 {
            let end = start.saturating_add(data.len());
            room.get_mut(start..end)
                .ok_or_else(corrupt)?
                .copy_from_slice(data);
            end
        } else {
            block::decode(data, room, start)?
        };
        frame.window.end = end;
        let decoded = frame.window.buf.get(start..end).unwrap_or_default();
        frame.hash.update(decoded);
        let n = u64::try_from(decoded.len()).unwrap_or(u64::MAX);
        frame.decoded = frame.decoded.saturating_add(n);
        if frame.header.size.is_some_and(|size| frame.decoded > size) {
            return Err(corrupt());
        }
        Ok(())
    }

    /// Ends the frame in progress.
    fn ended(&mut self) {
        self.frame = None;
        self.frames = self.frames.saturating_add(1);
        self.state = State::Frame;
    }
}

impl Default for Decoder {
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Debug for Decoder {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Decoder")
            .field("state", &self.state)
            .field("frames", &self.frames)
            .finish_non_exhaustive()
    }
}

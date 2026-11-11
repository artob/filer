// This is free and unencumbered software released into the public domain.

use crate::{
    Error,
    block::{self, Tables},
    corrupt, unsupported,
    window::{Cursor, Window},
};
use alloc::{boxed::Box, vec::Vec};
use compute_xxhash::Xxh64;
use core::ops::RangeInclusive;

/// A frame's magic number, and the range of skippable frames' (RFC 8878
/// §3.1).
const MAGIC: u32 = 0xFD2F_B528;
const SKIPPABLE: RangeInclusive<u32> = 0x184D_2A50..=0x184D_2A5F;

/// The most a block may hold or decode to (RFC 8878 §3.1.1.2.3).
const MAX_BLOCK: usize = 128 * 1024;

/// The largest window decoded, 128 MiB, the most zstd itself decodes by
/// default: a frame needing more is unsupported.
const MAX_WINDOW: u64 = 128 << 20;

/// A frame's header (RFC 8878 §3.1.1.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Header {
    /// How far back its blocks may copy from.
    window: usize,
    /// What it decodes to, if it says.
    size: Option<u64>,
    checksum: bool,
}

impl Header {
    /// Returns how long a header's dictionary identifier and content size
    /// are, from its descriptor.
    fn fields(descriptor: u8) -> (usize, usize) {
        let dictionary = match descriptor & 0x03 {
            0 => 0,
            1 => 1,
            2 => 2,
            _ => 4,
        };
        // A single segment always has a size.
        let size = match descriptor >> 6 {
            0 => usize::from(descriptor & 0x20 != 0),
            1 => 2,
            2 => 4,
            _ => 8,
        };
        (dictionary, size)
    }

    /// Returns how long a header is, magic number included, from its
    /// descriptor: a single segment has no window descriptor.
    fn len(descriptor: u8) -> usize {
        let (dictionary, size) = Self::fields(descriptor);
        5_usize
            .saturating_add(usize::from(descriptor & 0x20 == 0))
            .saturating_add(dictionary)
            .saturating_add(size)
    }

    /// Parses a whole header, magic number included.
    fn parse(header: &[u8]) -> Result<Self, Error> {
        let [_, _, _, _, descriptor, rest @ ..] = header else {
            return Err(corrupt());
        };
        if descriptor & 0x08 != 0 || header.len() != Self::len(*descriptor) {
            return Err(corrupt());
        }
        let single = descriptor & 0x20 != 0;
        let (window, rest) = match rest.split_first() {
            Some((&window, rest)) if !single => (Some(window), rest),
            _ => (None, rest),
        };
        let (dictionary, _) = Self::fields(*descriptor);
        let (dictionary, rest) = rest.split_at_checked(dictionary).ok_or_else(corrupt)?;
        // An identifier of zero names no dictionary.
        if dictionary.iter().any(|&byte| byte != 0) {
            return Err(unsupported());
        }
        let mut size = [0; 8];
        size.get_mut(..rest.len())
            .ok_or_else(corrupt)?
            .copy_from_slice(rest);
        let size = u64::from_le_bytes(size);
        let size = match rest.len() {
            0 => None,
            2 => Some(size.saturating_add(256)),
            _ => Some(size),
        };
        let window = match (window, size) {
            (Some(descriptor), _) => {
                let log = u32::from(descriptor >> 3).saturating_add(10);
                let base = 1_u64.checked_shl(log).ok_or_else(unsupported)?;
                base.saturating_add((base >> 3).saturating_mul(u64::from(descriptor & 0x07)))
            },
            // A single segment's window is its size.
            (None, Some(size)) => size,
            (None, None) => return Err(corrupt()),
        };
        if window > MAX_WINDOW {
            return Err(unsupported());
        }
        Ok(Self {
            window: usize::try_from(window).map_err(|_| unsupported())?,
            size,
            checksum: descriptor & 0x04 != 0,
        })
    }

    /// Returns the most a block of the frame may hold or decode to.
    fn max_block(&self) -> usize {
        self.window.min(MAX_BLOCK)
    }

    /// Returns how many bytes the block that `head` begins takes in all,
    /// header and checksum included, once `head` holds its header.
    fn block_len(&self, head: &[u8]) -> Result<Option<usize>, Error> {
        let Some(&[a, b, c]) = head.first_chunk() else {
            return Ok(None);
        };
        let header = u32::from_le_bytes([a, b, c, 0]);
        let size = usize::try_from(header >> 3).unwrap_or(usize::MAX);
        if size > self.max_block() {
            return Err(corrupt());
        }
        // Raw and compressed blocks hold their size, and RLE blocks one
        // byte, repeated; the fourth type is reserved.
        let content = match (header >> 1) & 0x03 {
            0 | 2 => size,
            1 => 1,
            _ => return Err(corrupt()),
        };
        let last = header & 1 != 0;
        let trailer = if last && self.checksum { 4 } else { 0 };
        Ok(Some(
            3_usize.saturating_add(content).saturating_add(trailer),
        ))
    }
}

/// A whole block, read apart: its type, the size it gives, whether it is
/// its frame's last, its content, and the frame's checksum, if last.
struct Block<'a> {
    kind: u32,
    size: usize,
    last: bool,
    content: &'a [u8],
    checksum: &'a [u8; 4],
}

impl<'a> Block<'a> {
    /// Reads a whole block apart, of a frame checked by `checksum`.
    fn read(unit: &'a [u8], checksum: bool) -> Result<Self, Error> {
        let Some((&[a, b, c], content)) = unit.split_first_chunk() else {
            return Err(corrupt());
        };
        let header = u32::from_le_bytes([a, b, c, 0]);
        let last = header & 1 != 0;
        let (content, checksum) = if last && checksum {
            content.split_last_chunk::<4>().ok_or_else(corrupt)?
        } else {
            (content, &[0; 4])
        };
        Ok(Self {
            kind: (header >> 1) & 0x03,
            size: usize::try_from(header >> 3).map_err(|_| corrupt())?,
            last,
            content,
            checksum,
        })
    }

    /// Decodes it onto `output`, with `tables` and `scratch` for a
    /// compressed block, of at most `max` literals.
    fn decode(
        &self,
        tables: &mut Tables,
        scratch: &mut Vec<u8>,
        output: &mut Cursor<'_>,
        max: usize,
    ) -> Result<(), Error> {
        match self.kind {
            0 => output.extend(self.content),
            1 => {
                let byte = self.content.first().copied().ok_or_else(corrupt)?;
                output.fill(byte, self.size)
            },
            2 => block::decode(self.content, tables, scratch, output, max),
            _ => Err(corrupt()),
        }
    }
}

/// What a frame's blocks have decoded to so far, hashed and counted.
#[derive(Clone)]
struct Sum {
    hash: Xxh64,
    decoded: u64,
}

impl Sum {
    /// Adds what a block of a frame of `header` decoded to, checking the
    /// frame's size and, past its last block, its checksum, if it has one,
    /// and only then hashing it.
    fn add(&mut self, header: &Header, decoded: &[u8], block: &Block<'_>) -> Result<(), Error> {
        if header.checksum {
            self.hash.update(decoded);
        }
        let n = u64::try_from(decoded.len()).unwrap_or(u64::MAX);
        self.decoded = self.decoded.saturating_add(n);
        if header.size.is_some_and(|size| self.decoded > size) {
            return Err(corrupt());
        }
        if block.last {
            if header.size.is_some_and(|size| self.decoded != size) {
                return Err(corrupt());
            }
            // The low four bytes of the hash, little-endian.
            let digest = self.hash.finish().to_le_bytes();
            if header.checksum && digest.get(..4) != Some(block.checksum) {
                return Err(corrupt());
            }
        }
        Ok(())
    }
}

/// A frame being decoded.
struct Frame {
    header: Header,
    window: Window,
    tables: Tables,
    sum: Sum,
}

impl Frame {
    /// Returns a frame of `header` at its start, its window not yet
    /// allocated.
    fn new(header: Header) -> Self {
        Self {
            header,
            window: Window::new(
                header.window,
                header.max_block(),
                header.size.and_then(|size| usize::try_from(size).ok()),
            ),
            tables: Tables::default(),
            sum: Sum {
                hash: Xxh64::new(0),
                decoded: 0,
            },
        }
    }

    /// Returns a copy that decodes on as it would, holding only what it
    /// needs of its window.
    fn copy(&self) -> Self {
        Self {
            header: self.header,
            window: self.window.copy_state(),
            tables: self.tables.clone(),
            sum: self.sum.clone(),
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
    /// Before a block of the frame, or in it, gathered so far.
    Block,
    /// Past the frame's last block, with what it decoded to left to
    /// produce.
    Last,
}

/// A decoder of a zstd stream (RFC 8878): frames, one after another, each
/// checked against its size and checksum, if it has them; skippable
/// frames are read past.
///
/// It gathers each frame header and block whole before it decodes it,
/// unless the input holds it whole, so it holds at most a block of packed
/// bytes; and of what they decode to, the frame's window and two blocks,
/// in a buffer that decoding goes round, allocated as it fills; and its
/// tables, some 41 KiB at the most: 4 KiB allocated once a frame, 29 KiB
/// by its first block that has sequences, and 8 KiB by the first whose
/// literals are enough for their codes to decode several at a time. A
/// frame that the input
/// holds whole, and whose header says what it decodes to, which the
/// output has room for, it decodes straight into the output, holding no
/// window, as libzstd does. A frame whose window passes 128 MiB, the most
/// zstd itself decodes by default, or that needs a dictionary is
/// [`Unsupported`](Error::Unsupported). What follows the last frame must
/// be another, as zstd requires.
///
/// # Examples
///
/// ```
/// use decode_zstd::Decoder;
///
/// // `printf hello | zstd`.
/// let packed = [
///     0x28, 0xB5, 0x2F, 0xFD, 0x04, 0x58, 0x29, 0x00, 0x00, 0x68, 0x65, 0x6C,
///     0x6C, 0x6F, 0xA3, 0x6D, 0x9F, 0x88,
/// ];
/// let mut decoder = Decoder::new();
/// let mut output = [0; 8];
/// assert_eq!(decoder.decode(&packed, &mut output)?, (18, 5));
/// assert_eq!(&output[..5], b"hello");
/// decoder.finish()?;
/// # Ok::<(), decode_zstd::Error>(())
/// ```
pub struct Decoder {
    /// The packed bytes of a frame header or block, gathered until whole.
    unit: Vec<u8>,
    /// What a block's literals decode to, before its sequences run.
    literals: Vec<u8>,
    state: State,
    /// The frame in progress, on the heap, as it takes some 4 KiB.
    frame: Option<Box<Frame>>,
    /// The frames decoded whole, skippable ones included.
    frames: u64,
}

impl Decoder {
    /// Returns a decoder at the start of a stream.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            unit: Vec::new(),
            literals: Vec::new(),
            state: State::Frame,
            frame: None,
            frames: 0,
        }
    }

    /// Decodes what it can of `input` into `output`, and returns how many
    /// bytes of each it consumed and produced.
    ///
    /// It returns neither consumed nor produced once it can go no further: needing
    /// more input, or more room, or past the end of the stream, which
    /// [`finish`](Self::finish) then tells apart. It returns at the end of
    /// each frame, where a [`checkpoint`](Self::checkpoint) costs nothing.
    ///
    /// # Errors
    ///
    /// [`Corrupt`](Error::Corrupt) for a malformed stream, and
    /// [`Unsupported`](Error::Unsupported) for a frame that needs a
    /// dictionary or a window larger than 128 MiB. The decoder is not to be
    /// used after an error.
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
            // Stop between frames, where a checkpoint costs nothing, once
            // there is progress to report.
            if self.frames != frames && (consumed > 0 || produced > 0) {
                return Ok((consumed, produced));
            }
            // A frame given whole, with room for it, straight into the
            // output.
            if self.frame.is_none() && self.state == State::Frame && self.unit.is_empty() {
                let rest = input.get(consumed..).unwrap_or_default();
                let room = output.get_mut(produced..).unwrap_or_default();
                if let Some((c, p)) = self.whole(rest, room)? {
                    consumed = consumed.saturating_add(c);
                    produced = produced.saturating_add(p);
                    self.frames = self.frames.saturating_add(1);
                    continue;
                }
            }
            let Some(n) = self.step(input.get(consumed..).unwrap_or_default())? else {
                return Ok((consumed, produced));
            };
            consumed = consumed.saturating_add(n);
        }
    }

    /// Checks that the stream has ended where a stream may: past a whole
    /// frame, and everything it decoded to produced.
    ///
    /// # Errors
    ///
    /// [`Corrupt`](Error::Corrupt) if it ended anywhere else, or before
    /// any frame.
    pub fn finish(&self) -> Result<(), Error> {
        if self.between() {
            Ok(())
        } else {
            Err(corrupt())
        }
    }

    /// Returns a decoder that goes on from where this one is: given the
    /// input from where this one has consumed to, less what it has
    /// [`buffered`](Self::buffered), it decodes what this one would.
    ///
    /// Between frames, which decode alone, that is a new decoder. Within
    /// one, it is a copy of this one's state: its window's size of the
    /// latest bytes, and those not yet produced, its tables and repeat
    /// offsets, and its hash so far, which [`footprint`](Self::footprint)
    /// measures. It is always offered, so `Option` only for the shape that
    /// the `decode-*` crates share.
    #[must_use]
    pub fn checkpoint(&self) -> Option<Self> {
        Some(Self {
            state: self.state,
            frame: self.frame.as_ref().map(|frame| Box::new(frame.copy())),
            frames: self.frames,
            ..Self::new()
        })
    }

    /// Returns about how many bytes of memory it holds: mostly its
    /// frame's window, which is what a [`checkpoint`](Self::checkpoint)
    /// within a frame costs.
    #[must_use]
    pub fn footprint(&self) -> usize {
        let frame = self.frame.as_ref().map_or(0, |frame| {
            let held = frame.window.footprint();
            held.saturating_add(frame.tables.footprint())
        });
        size_of::<Self>()
            .saturating_add(size_of::<Frame>())
            .saturating_add(frame)
            .saturating_add(self.unit.capacity())
            .saturating_add(self.literals.capacity())
    }

    /// Returns how many of the bytes it has consumed it holds, gathered
    /// toward a frame header or block that is not yet whole.
    #[must_use]
    pub fn buffered(&self) -> usize {
        self.unit.len()
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
            State::Frame | State::Block => {
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
            State::Last => Ok(None),
        }
    }

    /// Returns how many bytes the frame header or block that `head` begins
    /// takes in all, as far as `head` tells.
    fn need(&self, head: &[u8]) -> Result<usize, Error> {
        match (self.state, &self.frame) {
            (State::Block, Some(frame)) => Ok(frame.header.block_len(head)?.unwrap_or(3)),
            (State::Frame, _) => {
                let Some(magic) = head.first_chunk().copied().map(u32::from_le_bytes) else {
                    return Ok(4);
                };
                if SKIPPABLE.contains(&magic) {
                    return Ok(8);
                }
                if magic != MAGIC {
                    return Err(corrupt());
                }
                match head.get(4) {
                    Some(descriptor) if descriptor & 0x08 != 0 => Err(corrupt()),
                    Some(&descriptor) => Ok(Header::len(descriptor)),
                    None => Ok(5),
                }
            },
            _ => Err(corrupt()),
        }
    }

    /// Reads a whole frame header or block.
    fn unit(&mut self, unit: &[u8]) -> Result<(), Error> {
        if self.state == State::Block {
            return self.block(unit);
        }
        let magic = unit.first_chunk().copied().map(u32::from_le_bytes);
        if magic.is_some_and(|magic| SKIPPABLE.contains(&magic)) {
            let len = unit.get(4..8).and_then(|len| len.try_into().ok());
            match len.map(u32::from_le_bytes) {
                Some(0) => self.ended(),
                Some(len) => self.state = State::Skip(len),
                None => return Err(corrupt()),
            }
            return Ok(());
        }
        self.frame = Some(Box::new(Frame::new(Header::parse(unit)?)));
        self.state = State::Block;
        Ok(())
    }

    /// Decodes the frame that `input` begins straight into `output`, if
    /// `input` holds it whole, and its header says what it decodes to,
    /// which `output` has room for; and returns how many bytes of each it
    /// consumed and produced, or nothing, for the frame to be decoded onto
    /// a window, as anything malformed before its blocks is.
    fn whole(&mut self, input: &[u8], output: &mut [u8]) -> Result<Option<(usize, usize)>, Error> {
        let magic = input.first_chunk().copied().map(u32::from_le_bytes);
        let Some(&descriptor) = input.get(4).filter(|_| magic == Some(MAGIC)) else {
            return Ok(None);
        };
        let len = Header::len(descriptor);
        let Some(Ok(header)) = input.get(..len).map(Header::parse) else {
            return Ok(None);
        };
        let size = header.size.and_then(|size| usize::try_from(size).ok());
        let Some(output) = size.and_then(|size| output.get_mut(..size)) else {
            return Ok(None);
        };
        // Its blocks' headers, to its last, all within the input.
        let mut at = len;
        loop {
            let head = input.get(at..).unwrap_or_default();
            let Ok(Some(n)) = header.block_len(head) else {
                return Ok(None);
            };
            let Some(&first) = input.get(at..at.saturating_add(n)).and_then(<[u8]>::first) else {
                return Ok(None);
            };
            at = at.saturating_add(n);
            if first & 1 != 0 {
                break;
            }
        }
        let mut frame = Box::new(Frame::new(header));
        // No block decodes past the output, nor do its literals, which
        // are then allocated for no more.
        let max = header.max_block().min(output.len());
        let (mut at, mut end) = (len, 0);
        loop {
            let head = input.get(at..).unwrap_or_default();
            let n = header.block_len(head)?.ok_or_else(corrupt)?;
            let unit = input.get(at..at.saturating_add(n)).ok_or_else(corrupt)?;
            let block = Block::read(unit, header.checksum)?;
            let room = output.len().saturating_sub(end).min(max);
            let mut cursor = Cursor::over(output, end, room, header.window);
            block.decode(&mut frame.tables, &mut self.literals, &mut cursor, max)?;
            let to = cursor.end;
            let decoded = output.get(end..to).ok_or_else(corrupt)?;
            frame.sum.add(&header, decoded, &block)?;
            (at, end) = (at.saturating_add(n), to);
            if block.last {
                return Ok(Some((at, end)));
            }
        }
    }

    /// Decodes a whole block, header included, onto the frame's window.
    fn block(&mut self, unit: &[u8]) -> Result<(), Error> {
        let frame = self.frame.as_mut().ok_or_else(corrupt)?;
        let block = Block::read(unit, frame.header.checksum)?;
        let max = frame.header.max_block();
        let window = &mut frame.window;
        window.prepare()?;
        let start = window.end();
        let mut cursor = window.cursor(max);
        block.decode(&mut frame.tables, &mut self.literals, &mut cursor, max)?;
        let end = cursor.end;
        window.advance(end);
        frame.sum.add(&frame.header, window.since(start), &block)?;
        if block.last {
            self.state = State::Last;
        }
        Ok(())
    }

    /// Returns whether it is between frames, past at least one.
    fn between(&self) -> bool {
        self.frames > 0 && self.state == State::Frame && self.unit.is_empty()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::{Block, agree, zstd};
    use alloc::vec;

    fn decode(stream: &[u8]) -> Result<Vec<u8>, Error> {
        agree(stream, 7)
    }

    fn kind(stream: &[u8]) -> Error {
        decode(stream).unwrap_err()
    }

    /// Returns a frame of `blocks`, as [`zstd`] builds it, with a checksum
    /// of what they decode to, and a single segment of that size if
    /// `single`.
    fn checked(blocks: &[Block<'_>], single: bool) -> Vec<u8> {
        let mut hash = Xxh64::new(0);
        let mut size = 0;
        for block in blocks {
            match *block {
                Block::Raw(bytes) => hash.update(bytes),
                Block::Rle(byte, n) => hash.update(&vec![byte; n]),
            }
            size += match *block {
                Block::Raw(bytes) => bytes.len(),
                Block::Rle(_, n) => n,
            };
        }
        let mut frame = zstd(blocks);
        frame[4] |= 0x04;
        if single {
            // Eight bytes of size in place of the window.
            frame[4] |= 0xE0;
            frame.splice(5..6, (size as u64).to_le_bytes());
        }
        frame.extend_from_slice(&hash.finish().to_le_bytes()[..4]);
        frame
    }

    #[test]
    fn decodes_literals_stored_and_repeated() {
        // A compressed block of `content`, alone in its frame.
        let block = |content: &[u8]| {
            let mut stream = zstd(&[Block::Raw(content)]);
            stream[6] |= 2 << 1;
            stream
        };
        // Five bytes stored, in each size format, then none repeated.
        for header in [
            &[5 << 3][..],
            &[1 << 2 | 5 << 4, 0],
            &[3 << 2 | 5 << 4, 0, 0],
        ] {
            let stream = block(&[header, b"hello", &[0]].concat());
            assert_eq!(decode(&stream).unwrap(), b"hello");
        }
        assert_eq!(decode(&block(&[1 | 5 << 3, b'z', 0])).unwrap(), b"zzzzz");
        // The sequences' count must end the block, as zstd checks.
        assert_eq!(kind(&block(&[5 << 3, 1, 2, 3, 4, 5])), Error::Corrupt);
        assert_eq!(kind(&block(&[1 << 3, 1, 0, 0])), Error::Corrupt);
        // More literals than a block may decode to, which zstd refuses:
        // 128 KiB and one, repeated, in 20 bits.
        let large = block(&[1 | 3 << 2 | 1 << 4, 0x00, 0x20, b'z', 0]);
        assert_eq!(kind(&large), Error::Corrupt);
    }

    /// Four Huffman streams of a code of one bit, each stream a byte with
    /// its marker and `n` bits of symbol 0: the jump table, the streams,
    /// and no sequences, in a compressed block alone in its frame.
    fn four_streams(regenerated: u32, n: u32) -> Vec<u8> {
        // One weight, of symbol 0, so that symbol 1 has the other code.
        let tree = [0x80, 0x10];
        let stream = 1 << n;
        let compressed = (tree.len() + 6 + 4) as u32;
        let header = (2 | 1 << 2 | regenerated << 4 | compressed << 14).to_le_bytes();
        let content = [&header[..3], &tree, &[1, 0, 1, 0, 1, 0], &[stream; 4], &[0]].concat();
        let mut frame = zstd(&[Block::Raw(&content)]);
        frame[6] |= 2 << 1;
        frame
    }

    #[test]
    fn needs_six_literals_for_four_streams() {
        assert_eq!(decode(&four_streams(8, 2)).unwrap(), [0; 8]);
        // Fewer than six, which zstd refuses, as the streams cannot split.
        for regenerated in [0, 4] {
            assert_eq!(kind(&four_streams(regenerated, 1)), Error::Corrupt);
        }
    }

    #[test]
    fn decodes_rle_blocks_checked() {
        for single in [false, true] {
            let blocks = [
                Block::Rle(b'a', 100_000),
                Block::Raw(b"between"),
                Block::Rle(0, 1),
                Block::Rle(b'z', 128 * 1024),
            ];
            let stream = checked(&blocks, single);
            let data = decode(&stream).unwrap();
            assert_eq!(data.len(), 100_000 + 7 + 1 + 128 * 1024);
            let mut bad = stream.clone();
            *bad.last_mut().unwrap() ^= 1;
            assert_eq!(kind(&bad), Error::Corrupt);
        }
    }

    #[test]
    fn decodes_whole_only_frames_given_whole_with_room() {
        let blocks = [Block::Raw(b"hello"), Block::Rle(b'!', 3)];
        let sized = checked(&blocks, true);
        let mut output = [0; 9];
        let decoded = Decoder::new().whole(&sized, &mut output).unwrap();
        assert_eq!(decoded, Some((sized.len(), 8)));
        assert_eq!(&output[..8], b"hello!!!");
        // Room for less, the frame cut short, or no size in its header.
        let short = &sized[..sized.len() - 1];
        let windowed = checked(&blocks, false);
        for (stream, room) in [(&sized[..], 7), (short, 8), (&windowed, 8)] {
            let mut output = vec![0; room];
            assert_eq!(Decoder::new().whole(stream, &mut output).unwrap(), None);
        }
        // A checksum that fails, found there.
        let mut bad = sized.clone();
        *bad.last_mut().unwrap() ^= 1;
        assert_eq!(Decoder::new().whole(&bad, &mut output), Err(Error::Corrupt));
    }

    #[test]
    fn reads_sizes_of_each_length() {
        // One, two, four, and eight bytes of size, each in a single
        // segment, and two past a window; 300 bytes, but for one.
        for (descriptor, size, len) in [
            (0x20, vec![44], 44),
            (0x60, vec![44, 0], 300),
            (0xA0, vec![44, 1, 0, 0], 300),
            (0xE0, vec![44, 1, 0, 0, 0, 0, 0, 0], 300),
        ] {
            let data = vec![7; len];
            let mut stream = zstd(&[Block::Raw(&data)]);
            stream[4] = descriptor;
            stream.splice(5..6, size);
            assert_eq!(decode(&stream).unwrap(), data, "{descriptor:#x}");
            // A size other than what the frame decodes to.
            let mut short = stream.clone();
            short[5] -= 1;
            assert_eq!(kind(&short), Error::Corrupt, "{descriptor:#x}");
        }
        let data = vec![7; 300];
        let mut stream = zstd(&[Block::Raw(&data)]);
        stream[4] = 0x40;
        stream.splice(6..6, [44, 0]);
        assert_eq!(decode(&stream).unwrap(), data);
    }
}

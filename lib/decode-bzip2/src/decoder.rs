// This is free and unencumbered software released into the public domain.

use crate::{Error, corrupt, unsupported};
use alloc::{vec, vec::Vec};
use compute_crc::Crc32Bzip2;

/// The magic numbers that begin a block, and that end a stream: π and √π,
/// in binary-coded decimal.
const BLOCK: u64 = 0x3141_5926_5359;
const END: u64 = 0x1772_4538_5090;

/// The symbols of a block that each Huffman table selector covers.
const GROUP: usize = 50;

/// The most selectors kept: enough for a block of 900,000 bytes, each its
/// own symbol, and the end. Those past them are read but never used.
const MAX_SELECTORS: usize = 18_002;

/// The longest a Huffman code may be.
const MAX_CODE: u8 = 20;

/// The most packed bytes held toward a block: more than any block takes,
/// of 900,000 symbols at 20 bits at most, and the selectors and tables
/// that go with them.
const MAX_PACKED: usize = 4 << 20;

/// Why a unit of a stream did not parse.
enum Fail {
    /// The input ends before it does.
    More,
    Corrupt,
    Unsupported,
}

/// Bits read most significant first, from bit `at` on.
struct Bits<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Bits<'_> {
    fn bit(&mut self) -> Result<bool, Fail> {
        let byte = self.bytes.get(self.at >> 3).ok_or(Fail::More)?;
        let bit = byte.wrapping_shl(u32::try_from(self.at & 7).unwrap_or_default()) & 0x80 != 0;
        self.at = self.at.saturating_add(1);
        Ok(bit)
    }

    /// Reads `n` bits, up to 64, as a number.
    fn bits(&mut self, n: u8) -> Result<u64, Fail> {
        let mut value = 0_u64;
        for _ in 0..n {
            value = value.wrapping_shl(1) | u64::from(self.bit()?);
        }
        Ok(value)
    }

    /// Reads `n` bits, up to 32, as a `usize`.
    fn size(&mut self, n: u8) -> Result<usize, Fail> {
        usize::try_from(self.bits(n)?).map_err(|_| Fail::Corrupt)
    }
}

/// A canonical Huffman code, as bzip2 assigns them: the shorter codes
/// first, and those of a length in the order of their symbols.
struct Code {
    /// How many codes each length has.
    counts: [u16; 21],
    /// The symbols, in the order of their codes.
    symbols: Vec<u16>,
}

impl Code {
    /// Returns the code of symbols of `lengths`, each from 1 to 20.
    fn new(lengths: &[u8]) -> Self {
        let mut counts = [0_u16; 21];
        let mut symbols = Vec::with_capacity(lengths.len());
        for len in 1..=MAX_CODE {
            for (symbol, _) in lengths.iter().enumerate().filter(|(_, l)| **l == len) {
                symbols.push(u16::try_from(symbol).unwrap_or_default());
                if let Some(count) = counts.get_mut(usize::from(len)) {
                    *count = count.saturating_add(1);
                }
            }
        }
        Self { counts, symbols }
    }

    /// Reads a code and returns its symbol.
    fn decode(&self, bits: &mut Bits<'_>) -> Result<usize, Fail> {
        // Each length's first code, and the place of its symbol.
        let (mut code, mut first, mut index) = (0_u32, 0_u32, 0_u32);
        for &count in self.counts.iter().skip(1) {
            code |= u32::from(bits.bit()?);
            let count = u32::from(count);
            if let Some(offset) = code.checked_sub(first).filter(|&offset| offset < count) {
                let place = usize::try_from(index.saturating_add(offset)).unwrap_or(usize::MAX);
                return self
                    .symbols
                    .get(place)
                    .map(|&symbol| usize::from(symbol))
                    .ok_or(Fail::Corrupt);
            }
            index = index.saturating_add(count);
            first = first.saturating_add(count).wrapping_shl(1);
            code = code.wrapping_shl(1);
        }
        Err(Fail::Corrupt)
    }
}

/// A block, decoded but for its last two stages, which run as it is read:
/// the inverse Burrows–Wheeler transform, through `links`, each the byte at
/// its place and, above it, the place of the next; and the undoing of the
/// runs that bzip2 encodes first, where four bytes alike are followed by a
/// count of more.
struct Block {
    links: Vec<u32>,
    /// The place of the next byte, and how many are left.
    next: u32,
    left: usize,
    /// The last byte produced, and how many times running, up to four.
    last: Option<u8>,
    run: u8,
    /// The copies of `last` left to produce, from a run's count.
    repeat: u8,
    /// The CRC of what it has written, which bzip2 checks blocks with, and
    /// the one it must come to.
    crc: Crc32Bzip2,
    expected: u32,
}

impl Block {
    /// Reads a block, after its magic number, of at most `max` bytes before
    /// its runs are undone.
    fn read(bits: &mut Bits<'_>, max: usize) -> Result<Self, Fail> {
        let expected = u32::try_from(bits.bits(32)?).map_err(|_| Fail::Corrupt)?;
        // Randomized blocks, which no bzip2 has written since 0.9.5.
        if bits.bit()? {
            return Err(Fail::Unsupported);
        }
        let origin = bits.size(24)?;

        // The byte values in use, in sixteen ranges of sixteen.
        let ranges = bits.bits(16)?;
        let mut used = Vec::with_capacity(256);
        for range in 0..16_u8 {
            if ranges.wrapping_shl(u32::from(range)) & 0x8000 == 0 {
                continue;
            }
            let values = bits.bits(16)?;
            for value in 0..16_u8 {
                if values.wrapping_shl(u32::from(value)) & 0x8000 != 0 {
                    used.push(range.wrapping_mul(16) | value);
                }
            }
        }
        // Two symbols for runs of the first in order, one per other byte,
        // and one for the end.
        let alphabet = used.len().saturating_add(2);
        let Some(end) = alphabet.checked_sub(1).filter(|_| !used.is_empty()) else {
            return Err(Fail::Corrupt);
        };

        // The Huffman tables, and which each group of symbols uses, coded
        // as moves to the front of a list of them.
        let tables = bits.size(3)?;
        let count = bits.size(15)?;
        if !(2..=6).contains(&tables) || count == 0 {
            return Err(Fail::Corrupt);
        }
        let mut order = [0_u8, 1, 2, 3, 4, 5];
        let mut selectors = Vec::with_capacity(count.min(MAX_SELECTORS));
        for _ in 0..count {
            let mut j = 0_usize;
            while bits.bit()? {
                j = j.saturating_add(1);
                if j >= tables {
                    return Err(Fail::Corrupt);
                }
            }
            let table = to_front(&mut order, j)?;
            if selectors.len() < MAX_SELECTORS {
                selectors.push(table);
            }
        }
        let mut codes = Vec::with_capacity(tables);
        for _ in 0..tables {
            // Each length differs from the last by a step up or down.
            let mut len = u8::try_from(bits.bits(5)?).map_err(|_| Fail::Corrupt)?;
            let mut lengths = vec![0; alphabet];
            for length in &mut lengths {
                loop {
                    if !(1..=MAX_CODE).contains(&len) {
                        return Err(Fail::Corrupt);
                    }
                    if !bits.bit()? {
                        break;
                    }
                    len = if bits.bit()? {
                        len.saturating_sub(1)
                    } else {
                        len.saturating_add(1)
                    };
                }
                *length = len;
            }
            codes.push(Code::new(&lengths));
        }

        // The symbols: runs of the first in order, counted in bijective
        // base two by two symbols, and moves of others to the front.
        let mut links: Vec<u32> = Vec::new();
        let mut counts = [0_u32; 256];
        let mut order: Vec<u8> = (0..=u8::MAX).collect();
        let (mut run, mut weight) = (0_usize, 1_usize);
        let mut code = None;
        for symbols in 0_usize.. {
            if symbols % GROUP == 0 {
                let selector = selectors.get(symbols / GROUP).ok_or(Fail::Corrupt)?;
                code = codes.get(usize::from(*selector));
            }
            let symbol = code.ok_or(Fail::Corrupt)?.decode(bits)?;
            if symbol < 2 {
                run = weight
                    .checked_mul(symbol.saturating_add(1))
                    .and_then(|add| run.checked_add(add))
                    .filter(|&run| run <= max)
                    .ok_or(Fail::Corrupt)?;
                weight = weight.saturating_mul(2);
                continue;
            }
            if run > 0 {
                let byte = *order
                    .first()
                    .and_then(|&i| used.get(usize::from(i)))
                    .ok_or(Fail::Corrupt)?;
                push(&mut links, &mut counts, byte, run, max)?;
                (run, weight) = (0, 1);
            }
            if symbol == end {
                break;
            }
            let index = to_front(&mut order, symbol.saturating_sub(1))?;
            let byte = *used.get(usize::from(index)).ok_or(Fail::Corrupt)?;
            push(&mut links, &mut counts, byte, 1, max)?;
        }

        // Link each byte to the next, by where the sort put the bytes of
        // each value: in order of their places.
        if origin >= links.len() {
            return Err(Fail::Corrupt);
        }
        let mut starts = [0_u32; 256];
        let mut sum = 0_u32;
        for (start, count) in starts.iter_mut().zip(counts) {
            *start = sum;
            sum = sum.saturating_add(count);
        }
        for place in 0..links.len() {
            let [.., byte] = links.get(place).ok_or(Fail::Corrupt)?.to_be_bytes();
            let start = starts.get_mut(usize::from(byte)).ok_or(Fail::Corrupt)?;
            let link = links.get_mut(usize::try_from(*start).map_err(|_| Fail::Corrupt)?);
            let place = u32::try_from(place).map_err(|_| Fail::Corrupt)?;
            *link.ok_or(Fail::Corrupt)? |= place.wrapping_shl(8);
            *start = start.saturating_add(1);
        }
        let next = links.get(origin).ok_or(Fail::Corrupt)?.wrapping_shr(8);
        Ok(Self {
            left: links.len(),
            links,
            next,
            last: None,
            run: 0,
            repeat: 0,
            crc: Crc32Bzip2::new(),
            expected,
        })
    }

    /// Returns the next byte, or none at the block's end.
    fn next(&mut self) -> Result<Option<u8>, Error> {
        loop {
            if let Some(last) = self.last.filter(|_| self.repeat > 0) {
                self.repeat = self.repeat.saturating_sub(1);
                return Ok(Some(last));
            }
            let Some(left) = self.left.checked_sub(1) else {
                return Ok(None);
            };
            self.left = left;
            let place = usize::try_from(self.next).map_err(|_| corrupt())?;
            let link = *self.links.get(place).ok_or_else(corrupt)?;
            self.next = link.wrapping_shr(8);
            let [.., byte] = link.to_be_bytes();
            if self.run == 4 {
                (self.repeat, self.run) = (byte, 0);
                continue;
            }
            if self.last == Some(byte) {
                self.run = self.run.saturating_add(1);
            } else {
                (self.last, self.run) = (Some(byte), 1);
            }
            return Ok(Some(byte));
        }
    }

    /// Writes its next bytes to `output`, and returns how many.
    fn write(&mut self, output: &mut [u8]) -> Result<usize, Error> {
        let mut written = 0_usize;
        for slot in output.iter_mut() {
            let Some(byte) = self.next()? else {
                break;
            };
            *slot = byte;
            written = written.saturating_add(1);
        }
        self.crc.update(output.get(..written).unwrap_or_default());
        Ok(written)
    }

    /// Returns whether every byte has been written.
    const fn done(&self) -> bool {
        self.left == 0 && self.repeat == 0
    }
}

/// Moves the entry at `index` of `order` to its front, and returns it.
fn to_front(order: &mut [u8], index: usize) -> Result<u8, Fail> {
    let entry = *order.get(index).ok_or(Fail::Corrupt)?;
    order.copy_within(..index, 1);
    *order.first_mut().ok_or(Fail::Corrupt)? = entry;
    Ok(entry)
}

/// Appends `n` of `byte` to a block's bytes, of at most `max`.
fn push(
    links: &mut Vec<u32>,
    counts: &mut [u32; 256],
    byte: u8,
    n: usize,
    max: usize,
) -> Result<(), Fail> {
    if links.len().saturating_add(n) > max {
        return Err(Fail::Corrupt);
    }
    let count = counts.get_mut(usize::from(byte)).ok_or(Fail::Corrupt)?;
    *count = count.saturating_add(u32::try_from(n).map_err(|_| Fail::Corrupt)?);
    links.resize(links.len().saturating_add(n), u32::from(byte));
    Ok(())
}

/// What a stream holds next.
enum Unit {
    /// A stream's header, with its blocks' most bytes.
    Header(usize),
    Block(Block),
    /// A stream's end, with its blocks' combined CRC.
    End(u32),
    /// What is not a stream, after one, which ends the input.
    Garbage,
}

/// Where a [`Decoder`] is in its input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    /// Before a stream.
    Header,
    /// Before a block of a stream, or its end.
    Blocks,
    /// Past the last stream.
    Ended,
}

/// A decoder of a bzip2 stream: streams, one after another, each of blocks
/// checked against their CRCs, and their CRCs against the stream's.
///
/// Each block is gathered whole before it is decoded, so it holds at most
/// 4 MiB of packed bytes, more than any block takes, and of links between
/// the block's bytes, four bytes each. What a block decodes to, up to 50
/// times larger, is produced as it is read. After the first stream, bytes that
/// do not start one end the input, unread, as bzip2 ignores trailing
/// garbage. Randomized blocks, which bzip2 stopped writing in 1999, are
/// [`Unsupported`](Error::Unsupported).
///
/// # Examples
///
/// ```
/// use decode_bzip2::Decoder;
///
/// // `printf hello | bzip2`.
/// let packed = [
///     0x42, 0x5A, 0x68, 0x39, 0x31, 0x41, 0x59, 0x26, 0x53, 0x59, 0x19, 0x31,
///     0x65, 0x3D, 0x00, 0x00, 0x00, 0x81, 0x00, 0x02, 0x44, 0xA0, 0x00, 0x21,
///     0x9A, 0x68, 0x33, 0x4D, 0x07, 0x33, 0x8B, 0xB9, 0x22, 0x9C, 0x28, 0x48,
///     0x0C, 0x98, 0xB2, 0x9E, 0x80,
/// ];
/// let mut decoder = Decoder::new();
/// let mut output = [0; 8];
/// assert_eq!(decoder.decode(&packed, &mut output)?, (41, 5));
/// assert_eq!(&output[..5], b"hello");
/// // It returns after each block; then reads the stream's end.
/// assert_eq!(decoder.decode(&[], &mut output)?, (0, 0));
/// decoder.finish()?;
/// # Ok::<(), decode_bzip2::Error>(())
/// ```
pub struct Decoder {
    /// Packed bytes not yet parsed, from bit `bit` of the first.
    input: Vec<u8>,
    bit: usize,
    /// How much input parsing last ran out of, so that it waits for twice
    /// as much, or the end of the input, before trying again.
    tried: usize,
    state: State,
    /// The most bytes a block of the stream may hold before its runs are
    /// undone.
    max: usize,
    /// The CRC of the stream's blocks so far.
    combined: u32,
    streams: u64,
    block: Option<Block>,
}

impl Decoder {
    /// Returns a decoder at the start of a stream.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            input: Vec::new(),
            bit: 0,
            tried: 0,
            state: State::Header,
            max: 0,
            combined: 0,
            streams: 0,
            block: None,
        }
    }

    /// Decodes what it can of `input` into `output`, and returns how many
    /// bytes of each it consumed and produced.
    ///
    /// Given an empty `input`, it takes the input to have ended. It returns
    /// neither consumed nor produced once it can go no further: needing
    /// more input, or more room, or past the end of the stream, which
    /// [`finish`](Self::finish) then tells apart. It returns at the end of
    /// each block, where a [`checkpoint`](Self::checkpoint) may be.
    ///
    /// # Errors
    ///
    /// [`Corrupt`](Error::Corrupt) for a malformed stream, and
    /// [`Unsupported`](Error::Unsupported) for a randomized block. The
    /// decoder is not to be used after an error.
    pub fn decode(&mut self, input: &[u8], output: &mut [u8]) -> Result<(usize, usize), Error> {
        let exhausted = input.is_empty();
        let (mut consumed, mut produced) = (0_usize, 0_usize);
        loop {
            if let Some(block) = &mut self.block {
                let room = output.get_mut(produced..).unwrap_or_default();
                produced = produced.saturating_add(block.write(room)?);
                if !block.done() {
                    return Ok((consumed, produced));
                }
                if block.crc.finish() != block.expected {
                    return Err(corrupt());
                }
                self.combined = self.combined.rotate_left(1) ^ block.expected;
                self.block = None;
                // Stop between blocks, where a checkpoint may be, once
                // there is progress to report.
                if consumed > 0 || produced > 0 {
                    return Ok((consumed, produced));
                }
            }
            // Parsing takes input only once the last block is written, so
            // that it holds no more than the next block's, up to the most
            // one takes, and none past the last stream.
            if self.state != State::Ended && consumed < input.len() {
                let room = MAX_PACKED.saturating_sub(self.input.len());
                let end = consumed.saturating_add(room).min(input.len());
                self.input
                    .extend_from_slice(input.get(consumed..end).unwrap_or_default());
                consumed = end;
            }
            if !self.parse(exhausted)? {
                return Ok((consumed, produced));
            }
        }
    }

    /// Returns a decoder that goes on from where this one is, if it is
    /// between blocks or streams: given the input from where this one has
    /// consumed to, less what it has [`buffered`](Self::buffered), it
    /// parses on from the bit where the next begins, with the stream's
    /// limit and CRC so far, and decodes what this one would.
    #[must_use]
    pub fn checkpoint(&self) -> Option<Self> {
        if self.block.is_some() || self.state == State::Ended {
            return None;
        }
        Some(Self {
            bit: self.bit,
            state: self.state,
            max: self.max,
            combined: self.combined,
            streams: self.streams,
            ..Self::new()
        })
    }

    /// Returns how many of the bytes it has consumed it holds unparsed:
    /// it gathers each block whole before it decodes it.
    #[must_use]
    pub fn buffered(&self) -> usize {
        self.input.len()
    }

    /// Checks that the input has ended where it may: between streams, or
    /// in what follows the last, past at least one.
    ///
    /// # Errors
    ///
    /// [`Corrupt`](Error::Corrupt) if it ended anywhere else.
    pub fn finish(&self) -> Result<(), Error> {
        // Between streams, or past the last.
        let between = matches!(self.state, State::Header | State::Ended);
        if self.streams > 0 && self.block.is_none() && between {
            Ok(())
        } else {
            Err(corrupt())
        }
    }

    /// Parses what the input holds next, if it holds it whole, or all of it
    /// if `exhausted`, and returns whether it did.
    fn parse(&mut self, exhausted: bool) -> Result<bool, Error> {
        let full = self.input.len() >= MAX_PACKED;
        if self.state == State::Ended
            || (!exhausted && !full && self.input.len() < self.tried.saturating_mul(2))
        {
            return Ok(false);
        }
        let parsed = match self.state {
            State::Header => self.header(exhausted),
            _ => {
                let mut bits = Bits {
                    bytes: &self.input,
                    at: self.bit,
                };
                match bits.bits(48) {
                    Ok(BLOCK) => Block::read(&mut bits, self.max).map(Unit::Block),
                    Ok(END) => bits.bits(32).and_then(|crc| {
                        u32::try_from(crc).map(Unit::End).map_err(|_| Fail::Corrupt)
                    }),
                    Ok(_) => Err(Fail::Corrupt),
                    Err(fail) => Err(fail),
                }
                .map(|unit| (unit, bits.at))
            },
        };
        let (unit, mut at) = match parsed {
            Ok(parsed) => parsed,
            Err(Fail::More) => {
                // No block takes this much: one that would is malformed.
                if full {
                    return Err(corrupt());
                }
                self.tried = self.input.len();
                return Ok(false);
            },
            Err(Fail::Corrupt) => return Err(corrupt()),
            Err(Fail::Unsupported) => return Err(unsupported()),
        };
        self.tried = 0;
        match unit {
            Unit::Header(max) => (self.state, self.max, self.combined) = (State::Blocks, max, 0),
            Unit::Block(block) => self.block = Some(block),
            Unit::End(crc) => {
                if crc != self.combined {
                    return Err(corrupt());
                }
                self.streams = self.streams.saturating_add(1);
                self.state = State::Header;
                // A stream ends at a byte's end.
                at = at.checked_next_multiple_of(8).ok_or_else(corrupt)?;
            },
            Unit::Garbage => self.state = State::Ended,
        }
        self.input.drain(..(at >> 3).min(self.input.len()));
        self.bit = at & 7;
        Ok(true)
    }

    /// Parses a stream's header, `BZh` and the most hundreds of thousands
    /// of bytes a block holds, and returns it with the bits it took.
    fn header(&self, exhausted: bool) -> Result<(Unit, usize), Fail> {
        let head = self.input.get(..4).unwrap_or(&self.input);
        let digit = head.get(3).copied();
        let begins = head.iter().zip(b"BZh").all(|(byte, magic)| byte == magic)
            && digit.is_none_or(|digit| (b'1'..=b'9').contains(&digit));
        match digit {
            Some(digit) if begins => {
                let max = usize::from(digit.saturating_sub(b'0')).saturating_mul(100_000);
                Ok((Unit::Header(max), 32))
            },
            // After a stream, the input ends with what is not another.
            _ if (!begins || exhausted) && self.streams > 0 => Ok((Unit::Garbage, 0)),
            _ if !begins || exhausted => Err(Fail::Corrupt),
            _ => Err(Fail::More),
        }
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
            .field("streams", &self.streams)
            .finish_non_exhaustive()
    }
}

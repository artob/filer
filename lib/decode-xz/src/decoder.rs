// This is free and unencumbered software released into the public domain.

use crate::{
    Error,
    check::{Crc32, Crc64, Sha256, crc32},
    corrupt,
    filter::{Filter, Kind},
    unsupported,
};
use alloc::{boxed::Box, vec::Vec};

/// The bytes a stream begins with (§2.1.1.1) and ends with (§2.1.2.4).
const HEADER_MAGIC: [u8; 6] = [0xFD, b'7', b'z', b'X', b'Z', 0x00];
const FOOTER_MAGIC: [u8; 2] = *b"YZ";

/// The size of a stream's header and of its footer.
const HEADER: usize = 12;

/// The filter IDs of LZMA2 and Delta (§5.3); the branch converters' are
/// `filter::Kind::branch`'s.
const LZMA2: u64 = 0x21;
const DELTA: u64 = 0x03;

/// The most a filter stage holds of data to convert.
const STAGE: usize = 64 * 1024;

/// What the check of a stream's blocks is (§2.1.1.2, §3.4).
#[derive(Clone, Debug)]
enum Check {
    None,
    Crc32(Crc32),
    Crc64(Crc64),
    Sha256(Box<Sha256>),
    /// One this decoder does not know, of this many bytes, which it reads
    /// past without checking, as xz does.
    Unknown(usize),
}

impl Check {
    /// Returns the check of type `id`, at its start.
    fn new(id: u8) -> Self {
        match id {
            0x00 => Self::None,
            0x01 => Self::Crc32(Crc32::new()),
            0x04 => Self::Crc64(Crc64::new()),
            0x0A => Self::Sha256(Box::new(Sha256::new())),
            // Sizes of 4, 8, 16, 32, and 64 bytes, three types each.
            id => Self::Unknown(4_usize.wrapping_shl(u32::from(id.wrapping_sub(1) / 3))),
        }
    }

    /// Returns how many bytes it takes.
    const fn size(&self) -> usize {
        match self {
            Self::None => 0,
            Self::Crc32(_) => 4,
            Self::Crc64(_) => 8,
            Self::Sha256(_) => 32,
            Self::Unknown(size) => *size,
        }
    }

    /// Feeds `bytes`.
    fn update(&mut self, bytes: &[u8]) {
        match self {
            Self::None | Self::Unknown(_) => {},
            Self::Crc32(crc) => *crc = crc.update(bytes),
            Self::Crc64(crc) => *crc = crc.update(bytes),
            Self::Sha256(sha) => sha.update(bytes),
        }
    }

    /// Returns whether `stored` is the check of the bytes fed.
    fn matches(&self, stored: &[u8]) -> bool {
        match self {
            Self::None | Self::Unknown(_) => true,
            Self::Crc32(crc) => stored == crc.finish().to_le_bytes(),
            Self::Crc64(crc) => stored == crc.finish().to_le_bytes(),
            Self::Sha256(sha) => stored == sha.as_ref().clone().finish(),
        }
    }
}

/// A variable-length integer read so far (§1.2): seven bits a byte, the
/// lowest first, in nine bytes at most, the last without its high bit.
#[derive(Clone, Copy, Debug, Default)]
struct Varint {
    value: u64,
    shift: u32,
}

impl Varint {
    /// Reads `byte`, and returns the integer once whole.
    fn read(&mut self, byte: u8) -> Result<Option<u64>, Error> {
        // No byte but the first may be a zero, nor any past the ninth.
        if (self.shift > 0 && byte == 0) || self.shift >= 63 {
            return Err(corrupt());
        }
        self.value |= u64::from(byte & 0x7F).wrapping_shl(self.shift);
        self.shift = self.shift.wrapping_add(7);
        if byte & 0x80 != 0 {
            return Ok(None);
        }
        let value = self.value;
        *self = Self::default();
        Ok(Some(value))
    }
}

/// Reads a variable-length integer from `bytes` past `at`, which must hold
/// it whole.
fn varint(bytes: &[u8], at: &mut usize) -> Result<u64, Error> {
    let mut varint = Varint::default();
    loop {
        let byte = *bytes.get(*at).ok_or_else(corrupt)?;
        *at = at.saturating_add(1);
        if let Some(value) = varint.read(byte)? {
            return Ok(value);
        }
    }
}

/// A filter of a block's chain, with the data it holds: converted up to
/// `done`, and after that, what it may convert once more follows.
#[derive(Clone, Debug)]
struct Stage {
    filter: Filter,
    buf: Vec<u8>,
    done: usize,
    /// Whether all of its input has come.
    ended: bool,
}

/// A block being decoded.
#[derive(Clone, Debug)]
struct Block {
    lzma2: decode_lzma::Decoder,
    /// The filters before LZMA2, in the order they decode: from the last
    /// in the header to the first.
    stages: Vec<Stage>,
    check: Check,
    header: u64,
    /// What its header says of its sizes, and what it has come to.
    declared: (Option<u64>, Option<u64>),
    compressed: u64,
    uncompressed: u64,
}

impl Block {
    /// Decodes what it can of the block's data from `input` into `output`,
    /// and returns how much of each it consumed and produced, and whether
    /// the data has ended.
    fn decode(&mut self, input: &[u8], output: &mut [u8]) -> Result<(usize, usize, bool), Error> {
        let (consumed, produced) = if self.stages.is_empty() {
            let input = self.limit(input);
            let (consumed, produced) = self.lzma2.decode(input, output).map_err(lzma)?;
            self.compressed = self.compressed.saturating_add(to_u64(consumed));
            (consumed, produced)
        } else {
            self.filtered(input, output)?
        };
        let decoded = output.get(..produced).unwrap_or_default();
        self.check.update(decoded);
        self.uncompressed = self.uncompressed.saturating_add(to_u64(produced));
        let (compressed, uncompressed) = self.declared;
        if compressed.is_some_and(|size| self.compressed > size)
            || uncompressed.is_some_and(|size| self.uncompressed > size)
        {
            return Err(corrupt());
        }
        let ended = self.lzma2.finish().is_ok()
            && self
                .stages
                .last()
                .is_none_or(|stage| stage.ended && stage.buf.is_empty());
        Ok((consumed, produced, ended))
    }

    /// Returns `input` no longer than what is left of the compressed data,
    /// if its size is declared.
    fn limit<'a>(&self, input: &'a [u8]) -> &'a [u8] {
        match self.declared.0 {
            Some(size) => {
                let left = size.saturating_sub(self.compressed);
                let left = usize::try_from(left).unwrap_or(usize::MAX);
                input.get(..left.min(input.len())).unwrap_or_default()
            },
            None => input,
        }
    }

    /// Decodes through the filters, as `decode` does.
    fn filtered(&mut self, input: &[u8], output: &mut [u8]) -> Result<(usize, usize), Error> {
        let (mut consumed, mut produced) = (0_usize, 0_usize);
        loop {
            // What the last stage has converted goes out first.
            if let Some(last) = self.stages.last_mut() {
                let room = output.get_mut(produced..).unwrap_or_default();
                let n = last.done.min(room.len());
                if let (Some(to), Some(from)) = (room.get_mut(..n), last.buf.get(..n)) {
                    to.copy_from_slice(from);
                }
                last.buf.drain(..n);
                last.done = last.done.saturating_sub(n);
                produced = produced.saturating_add(n);
                if last.done > 0 || (last.ended && last.buf.is_empty()) {
                    return Ok((consumed, produced));
                }
            }
            // Then more from LZMA2, into the first stage.
            let rest = self.limit(input.get(consumed..).unwrap_or_default());
            let mut pulled = 0;
            if let Some(first) = self.stages.first_mut()
                && !first.ended
            {
                let len = first.buf.len();
                let room = STAGE.saturating_sub(len);
                first.buf.resize(len.saturating_add(room), 0);
                let to = first.buf.get_mut(len..).unwrap_or_default();
                let (c, p) = self.lzma2.decode(rest, to).map_err(lzma)?;
                first.buf.truncate(len.saturating_add(p));
                pulled = c.saturating_add(p);
                consumed = consumed.saturating_add(c);
                self.compressed = self.compressed.saturating_add(to_u64(c));
                first.ended = self.lzma2.finish().is_ok();
            }
            // And on through each stage, as far as it can convert.
            let mut moved = false;
            let mut ended = true;
            let stages = self.stages.len();
            for i in 0..stages {
                let Some(stage) = self.stages.get_mut(i) else {
                    break;
                };
                stage.ended = stage.ended || (i > 0 && ended);
                let end = stage.ended;
                let rest = stage.buf.get_mut(stage.done..).unwrap_or_default();
                let converted = stage.filter.convert(rest, end);
                stage.done = stage.done.saturating_add(converted);
                ended = end && stage.done == stage.buf.len();
                if i.saturating_add(1) < stages && stage.done > 0 {
                    let done: Vec<u8> = stage.buf.drain(..stage.done).collect();
                    stage.done = 0;
                    if let Some(next) = self.stages.get_mut(i.saturating_add(1)) {
                        next.buf.extend_from_slice(&done);
                        moved = true;
                    }
                }
            }
            let ready = self
                .stages
                .last()
                .is_some_and(|last| last.done > 0 || last.ended);
            if pulled == 0 && !moved && !ready {
                // It needs more input, or LZMA2 more room, which a stage
                // that converts nothing yet cannot give.
                return Ok((consumed, produced));
            }
        }
    }
}

/// Returns `n` as a 64-bit count.
fn to_u64(n: usize) -> u64 {
    u64::try_from(n).unwrap_or(u64::MAX)
}

/// Returns `decode-lzma`'s `error` as one of ours.
fn lzma(error: decode_lzma::Error) -> Error {
    match error {
        decode_lzma::Error::Unsupported => unsupported(),
        _ => corrupt(),
    }
}

/// Where an index is, after its indicator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Index {
    /// In its number of records.
    Count,
    /// In a record's unpadded size, of this many records left.
    Unpadded(u64),
    /// In a record's uncompressed size, of this many records left.
    Uncompressed(u64),
    /// In its padding.
    Padding,
    /// In its CRC32, of 4 bytes, with this many read.
    Crc(u8),
}

/// Where a [`Decoder`] is.
#[derive(Clone, Debug)]
enum State {
    /// In a stream's header, the bytes read so far in `field`.
    StreamHeader,
    /// In a block's header, or at an index's indicator, the bytes read so
    /// far in `field`.
    BlockHeader,
    /// In a block's data.
    Data(Box<Block>),
    /// In a block's padding, with this many bytes left, then its check.
    BlockPadding(Box<Block>, usize),
    /// In a block's check, the bytes read so far in `field`.
    Check(Box<Block>),
    /// In the index.
    Index(Index),
    /// In a stream's footer, the bytes read so far in `field`.
    Footer,
    /// Past a stream, in padding of this many bytes so far.
    Padding(u64),
}

/// What a stream's index holds, and what its blocks came to: both of them
/// hashed, so that they compare in constant memory (§4.3).
#[derive(Clone, Debug)]
struct Records {
    count: u64,
    hash: Sha256,
}

impl Records {
    fn new() -> Self {
        Self {
            count: 0,
            hash: Sha256::new(),
        }
    }

    fn add(&mut self, unpadded: u64, uncompressed: u64) {
        self.count = self.count.saturating_add(1);
        self.hash.update(&unpadded.to_le_bytes());
        self.hash.update(&uncompressed.to_le_bytes());
    }
}

/// A decoder of xz (the `.xz` file format): streams, one after another,
/// with padding between and after them, each of blocks of LZMA2, through
/// Delta or branch converters, each checked against its CRC-32, CRC-64,
/// or SHA-256 as it ends, and each stream against its index.
///
/// A block whose filters are other than those, or that needs a dictionary
/// over 128 MiB, is [`Unsupported`](Error::Unsupported); a check of a
/// type that xz reserves is read past without checking, as xz does. What
/// follows the last stream must be another, or padding, as xz requires.
/// Between blocks, it offers a [`checkpoint`](Self::checkpoint), as each
/// block decodes alone; and it returns as each block ends, so that a
/// caller may take one there.
///
/// # Examples
///
/// ```
/// use decode_xz::Decoder;
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
/// let mut decoder = Decoder::new();
/// let (mut at, mut output) = (0, Vec::new());
/// loop {
///     let mut buf = [0; 16];
///     let (consumed, produced) = decoder.decode(&packed[at..], &mut buf)?;
///     at += consumed;
///     output.extend_from_slice(&buf[..produced]);
///     if (consumed, produced) == (0, 0) {
///         break;
///     }
/// }
/// assert_eq!(output, b"hello");
/// decoder.finish()?;
/// # Ok::<(), decode_xz::Error>(())
/// ```
#[derive(Clone, Debug)]
pub struct Decoder {
    state: State,
    /// The bytes of the header or field being read.
    field: Vec<u8>,
    /// The stream's flags, once its header is read.
    flags: [u8; 2],
    /// The stream's blocks, as they came, and as its index lists them.
    blocks: Records,
    listed: Records,
    /// The index's size so far, its CRC32, and the integer being read.
    index: (u64, Crc32, Varint),
    /// How many streams have ended.
    streams: u64,
}

impl Decoder {
    /// Returns a decoder at the start of an `.xz` file.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: State::StreamHeader,
            field: Vec::new(),
            flags: [0; 2],
            blocks: Records::new(),
            listed: Records::new(),
            index: (0, Crc32::new(), Varint::default()),
            streams: 0,
        }
    }

    /// Decodes what it can of `input` into `output`, and returns how many
    /// bytes of each it consumed and produced.
    ///
    /// It returns neither consumed nor produced once it can go no further:
    /// needing more input, or more room, or past the end of the stream,
    /// which [`finish`](Self::finish) then tells apart. It also returns as
    /// each block ends, so that a caller may take a
    /// [`checkpoint`](Self::checkpoint) there.
    ///
    /// # Errors
    ///
    /// [`Corrupt`](Error::Corrupt) for a malformed stream, and
    /// [`Unsupported`](Error::Unsupported) for a block it cannot decode.
    /// The decoder is not to be used after an error.
    pub fn decode(&mut self, input: &[u8], output: &mut [u8]) -> Result<(usize, usize), Error> {
        let (mut at, mut produced) = (0_usize, 0_usize);
        loop {
            let rest = input.get(at..).unwrap_or_default();
            let room = output.get_mut(produced..).unwrap_or_default();
            let (consumed, made, flow) = self.step(rest, room)?;
            at = at.saturating_add(consumed);
            produced = produced.saturating_add(made);
            match flow {
                Flow::Go => {},
                // Returning as a block ends, but not without progress.
                Flow::Block if (at, produced) == (0, 0) => {},
                Flow::Stop | Flow::Block => return Ok((at, produced)),
            }
        }
    }

    /// Checks that the input has ended where it may: past a stream, or
    /// padding a multiple of four bytes long after one.
    ///
    /// # Errors
    ///
    /// [`Corrupt`](Error::Corrupt) if it has not.
    pub fn finish(&self) -> Result<(), Error> {
        match self.state {
            State::Padding(padding) if self.streams > 0 && padding % 4 == 0 => Ok(()),
            _ => Err(corrupt()),
        }
    }

    /// Returns a decoder that goes on from where this one is, if it is
    /// between blocks or streams: as it was there, given again the bytes
    /// since, which [`buffered`](Self::buffered) counts.
    #[must_use]
    pub fn checkpoint(&self) -> Option<Self> {
        match self.state {
            State::StreamHeader | State::BlockHeader | State::Padding(_) => Some(Self {
                state: self.state.clone(),
                field: Vec::new(),
                flags: self.flags,
                blocks: self.blocks.clone(),
                listed: self.listed.clone(),
                index: self.index,
                streams: self.streams,
            }),
            _ => None,
        }
    }

    /// Returns how many of the bytes it has consumed a
    /// [`checkpoint`](Self::checkpoint) must be given again: those of the
    /// header it is reading.
    #[must_use]
    pub fn buffered(&self) -> usize {
        match self.state {
            State::StreamHeader | State::BlockHeader => self.field.len(),
            _ => 0,
        }
    }

    /// Decodes what it can in the current state, and returns how much it
    /// consumed and produced, and whether to go on.
    fn step(&mut self, input: &[u8], output: &mut [u8]) -> Result<(usize, usize, Flow), Error> {
        let state = core::mem::replace(&mut self.state, State::Padding(0));
        let (state, consumed, produced, flow) = match state {
            State::Data(mut block) => {
                let (consumed, produced, ended) = block.decode(input, output)?;
                if ended {
                    let state = self.ended(block)?;
                    (state, consumed, produced, Flow::Go)
                } else {
                    let flow = if (consumed, produced) == (0, 0) {
                        Flow::Stop
                    } else {
                        Flow::Go
                    };
                    (State::Data(block), consumed, produced, flow)
                }
            },
            State::BlockPadding(block, 0) => {
                self.field.clear();
                (State::Check(block), 0, 0, Flow::Go)
            },
            State::Check(block) if block.check.size() == 0 => {
                let (state, consumed, flow) = self.checked(&block)?;
                (state, consumed, 0, flow)
            },
            state => {
                let Some(&byte) = input.first() else {
                    self.state = state;
                    return Ok((0, 0, Flow::Stop));
                };
                let (state, consumed, flow) = self.byte(state, byte)?;
                (state, consumed, 0, flow)
            },
        };
        self.state = state;
        Ok((consumed, produced, flow))
    }

    /// Checks a block whose data has ended against its header, and returns
    /// the state at its padding.
    fn ended(&mut self, block: Box<Block>) -> Result<State, Error> {
        let (compressed, uncompressed) = block.declared;
        if compressed.is_some_and(|size| size != block.compressed)
            || uncompressed.is_some_and(|size| size != block.uncompressed)
        {
            return Err(corrupt());
        }
        let padding = usize::try_from(block.compressed.wrapping_neg() & 3).unwrap_or_default();
        Ok(State::BlockPadding(block, padding))
    }

    /// Reads `byte` in `state`, and returns the state after it, how many
    /// bytes it consumed, and whether to go on.
    fn byte(&mut self, state: State, byte: u8) -> Result<(State, usize, Flow), Error> {
        let state = match state {
            State::StreamHeader => {
                self.field.push(byte);
                if self.field.len() < HEADER {
                    State::StreamHeader
                } else {
                    self.stream_header()?;
                    State::BlockHeader
                }
            },
            State::BlockHeader if self.field.is_empty() && byte == 0 => {
                // The index's indicator, past the last block.
                self.index = (1, Crc32::new().update(&[0]), Varint::default());
                State::Index(Index::Count)
            },
            State::BlockHeader => {
                self.field.push(byte);
                let size = self.field.first().map_or(0, |&size| {
                    usize::from(size).saturating_add(1).saturating_mul(4)
                });
                if self.field.len() < size {
                    State::BlockHeader
                } else {
                    State::Data(Box::new(self.block_header()?))
                }
            },
            State::BlockPadding(block, left) => {
                if byte != 0 {
                    return Err(corrupt());
                }
                State::BlockPadding(block, left.saturating_sub(1))
            },
            State::Check(block) => {
                self.field.push(byte);
                if self.field.len() < block.check.size() {
                    State::Check(block)
                } else {
                    return self.checked(&block);
                }
            },
            State::Index(index) => self.index(index, byte)?,
            State::Footer => {
                self.field.push(byte);
                if self.field.len() < HEADER {
                    State::Footer
                } else {
                    self.footer()?;
                    State::Padding(0)
                }
            },
            State::Padding(padding) if byte == 0 => State::Padding(padding.saturating_add(1)),
            State::Padding(padding) => {
                // Another stream, after padding a multiple of four bytes.
                if padding % 4 != 0 {
                    return Err(corrupt());
                }
                self.field.clear();
                return Ok((State::StreamHeader, 0, Flow::Go));
            },
            State::Data(_) => return Err(corrupt()),
        };
        Ok((state, 1, Flow::Go))
    }

    /// Checks a block's check, which `field` holds, and records it.
    fn checked(&mut self, block: &Block) -> Result<(State, usize, Flow), Error> {
        if !block.check.matches(&self.field) {
            return Err(corrupt());
        }
        let unpadded = block
            .header
            .saturating_add(block.compressed)
            .saturating_add(to_u64(block.check.size()));
        self.blocks.add(unpadded, block.uncompressed);
        self.field.clear();
        let consumed = usize::from(block.check.size() > 0);
        Ok((State::BlockHeader, consumed, Flow::Block))
    }

    /// Checks a stream's header, which `field` holds (§2.1.1), and begins
    /// the stream.
    fn stream_header(&mut self) -> Result<(), Error> {
        let Some((magic, rest)) = self.field.split_first_chunk::<6>() else {
            return Err(corrupt());
        };
        let Some((&flags, rest)) = rest.split_first_chunk::<2>() else {
            return Err(corrupt());
        };
        let stored = rest.first_chunk::<4>().map(|crc| u32::from_le_bytes(*crc));
        if *magic != HEADER_MAGIC || stored != Some(crc32(&flags)) {
            return Err(corrupt());
        }
        // The first byte, and the second's high bits, are reserved.
        if flags[0] != 0 || flags[1] & 0xF0 != 0 {
            return Err(corrupt());
        }
        self.flags = flags;
        self.blocks = Records::new();
        self.listed = Records::new();
        self.field.clear();
        Ok(())
    }

    /// Reads a block's header, which `field` holds (§3.1), and returns the
    /// block it begins.
    fn block_header(&mut self) -> Result<Block, Error> {
        let header = core::mem::take(&mut self.field);
        let (body, stored) = header.split_last_chunk::<4>().ok_or_else(corrupt)?;
        if u32::from_le_bytes(*stored) != crc32(body) {
            return Err(corrupt());
        }
        let flags = body.get(1).copied().ok_or_else(corrupt)?;
        if flags & 0x3C != 0 {
            return Err(corrupt());
        }
        let mut at = 2;
        let compressed = if flags & 0x40 != 0 {
            match varint(body, &mut at)? {
                0 => return Err(corrupt()),
                size => Some(size),
            }
        } else {
            None
        };
        let uncompressed = if flags & 0x80 != 0 {
            Some(varint(body, &mut at)?)
        } else {
            None
        };
        let count = usize::from(flags & 3).saturating_add(1);
        let mut kinds = Vec::with_capacity(count);
        let mut lzma2 = None;
        for i in 0..count {
            let id = varint(body, &mut at)?;
            let size = usize::try_from(varint(body, &mut at)?).map_err(|_| corrupt())?;
            let properties = body.get(at..at.saturating_add(size)).ok_or_else(corrupt)?;
            at = at.saturating_add(size);
            let last = i.saturating_add(1) == count;
            match (id, last) {
                // LZMA2 is the last filter, and only the last.
                (LZMA2, true) => {
                    let &[property] = properties else {
                        return Err(corrupt());
                    };
                    if property & 0xC0 != 0 {
                        return Err(corrupt());
                    }
                    lzma2 = Some(decode_lzma::Decoder::lzma2(property).map_err(lzma)?);
                },
                (LZMA2, false) => return Err(corrupt()),
                (_, true) => return Err(unsupported()),
                (DELTA, false) => {
                    let &[distance] = properties else {
                        return Err(corrupt());
                    };
                    kinds.push(Filter::new(Kind::Delta(distance), 0));
                },
                (id, false) => {
                    let kind = Kind::branch(id).ok_or_else(unsupported)?;
                    let start = match properties {
                        [] => 0,
                        &[a, b, c, d] => u32::from_le_bytes([a, b, c, d]),
                        _ => return Err(corrupt()),
                    };
                    if start.checked_rem(kind.alignment()) != Some(0) {
                        return Err(corrupt());
                    }
                    kinds.push(Filter::new(kind, start));
                },
            }
        }
        // The rest is padding.
        if body
            .get(at..)
            .is_none_or(|padding| padding.iter().any(|&b| b != 0))
        {
            return Err(corrupt());
        }
        let stages = kinds
            .into_iter()
            .rev()
            .map(|filter| Stage {
                filter,
                buf: Vec::new(),
                done: 0,
                ended: false,
            })
            .collect();
        let [_, check] = self.flags;
        Ok(Block {
            lzma2: lzma2.ok_or_else(corrupt)?,
            stages,
            check: Check::new(check),
            header: to_u64(header.len()),
            declared: (compressed, uncompressed),
            compressed: 0,
            uncompressed: 0,
        })
    }

    /// Reads `byte` of the index, in `index`, and returns the state after
    /// it (§4).
    fn index(&mut self, index: Index, byte: u8) -> Result<State, Error> {
        let (size, crc, mut varint) = self.index;
        let (size, crc) = match index {
            Index::Crc(_) => (size, crc),
            _ => (size.saturating_add(1), crc.update(&[byte])),
        };
        let next = match index {
            Index::Count => match varint.read(byte)? {
                Some(0) => Index::Padding,
                Some(count) => Index::Unpadded(count),
                None => Index::Count,
            },
            Index::Unpadded(left) => match varint.read(byte)? {
                Some(unpadded) => {
                    self.listed.count = self.listed.count.saturating_add(1);
                    self.listed.hash.update(&unpadded.to_le_bytes());
                    Index::Uncompressed(left)
                },
                None => Index::Unpadded(left),
            },
            Index::Uncompressed(left) => match varint.read(byte)? {
                Some(uncompressed) => {
                    self.listed.hash.update(&uncompressed.to_le_bytes());
                    match left.saturating_sub(1) {
                        0 => Index::Padding,
                        left => Index::Unpadded(left),
                    }
                },
                None => Index::Uncompressed(left),
            },
            Index::Padding if byte != 0 => return Err(corrupt()),
            Index::Padding => Index::Padding,
            Index::Crc(seen) => {
                self.field.push(byte);
                if seen < 3 {
                    Index::Crc(seen.saturating_add(1))
                } else {
                    let stored = self
                        .field
                        .first_chunk::<4>()
                        .map(|crc| u32::from_le_bytes(*crc));
                    if stored != Some(crc.finish()) {
                        return Err(corrupt());
                    }
                    // The index lists the blocks there were.
                    let listed = core::mem::replace(&mut self.listed, Records::new());
                    let blocks = core::mem::replace(&mut self.blocks, Records::new());
                    if listed.count != blocks.count || listed.hash.finish() != blocks.hash.finish()
                    {
                        return Err(corrupt());
                    }
                    self.field.clear();
                    self.index = (size, crc, varint);
                    return Ok(State::Footer);
                }
            },
        };
        // Padding ends the index once a multiple of four, before its CRC32.
        let next = match next {
            Index::Padding if size % 4 == 0 => {
                self.field.clear();
                Index::Crc(0)
            },
            next => next,
        };
        self.index = (size, crc, varint);
        Ok(State::Index(next))
    }

    /// Checks a stream's footer, which `field` holds (§2.1.2), against its
    /// header and index, and ends the stream.
    fn footer(&mut self) -> Result<(), Error> {
        let footer = core::mem::take(&mut self.field);
        let Some((stored, rest)) = footer.split_first_chunk::<4>() else {
            return Err(corrupt());
        };
        let Some((backward, rest)) = rest.split_first_chunk::<4>() else {
            return Err(corrupt());
        };
        let Some((flags, magic)) = rest.split_first_chunk::<2>() else {
            return Err(corrupt());
        };
        let crc = Crc32::new().update(backward).update(flags).finish();
        let (index, _, _) = self.index;
        // The index's size, its CRC32 included.
        let index = index.saturating_add(4);
        let backward = u64::from(u32::from_le_bytes(*backward));
        let real = backward.saturating_add(1).saturating_mul(4);
        if u32::from_le_bytes(*stored) != crc
            || *flags != self.flags
            || magic != FOOTER_MAGIC
            || real != index
        {
            return Err(corrupt());
        }
        self.streams = self.streams.saturating_add(1);
        Ok(())
    }
}

impl Default for Decoder {
    fn default() -> Self {
        Self::new()
    }
}

/// Whether to go on decoding.
enum Flow {
    Go,
    /// It needs more input or room, or has ended.
    Stop,
    /// A block has ended.
    Block,
}

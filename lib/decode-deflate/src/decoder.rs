// This is free and unencumbered software released into the public domain.

use crate::{
    Error, corrupt,
    huffman::{Code, Kind, Table},
};
use alloc::{boxed::Box, vec::Vec};

/// The most a call decodes before it produces what it decoded.
const CHUNK: usize = 32 * 1024;

/// How far past its end a match may write, as it copies in chunks of up
/// to 16.
const SLACK: usize = 16;

/// What a decoder decodes: Deflate, or Deflate64, PKWARE's variant of it,
/// with a 64 KiB window, length code 285 as 3 and 16 extra bits, and
/// distance codes 30 and 31.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Format {
    #[default]
    Deflate,
    #[cfg(feature = "deflate64")]
    Deflate64,
}

impl Format {
    /// How far back a match may copy from.
    const fn window(self) -> usize {
        match self {
            Self::Deflate => 32 * 1024,
            #[cfg(feature = "deflate64")]
            Self::Deflate64 => 64 * 1024,
        }
    }

    /// The longest a match may be.
    const fn longest(self) -> usize {
        match self {
            Self::Deflate => 258,
            #[cfg(feature = "deflate64")]
            Self::Deflate64 => 0xFFFF_usize.saturating_add(3),
        }
    }

    /// How many distance codes may occur, and a dynamic block describe.
    const fn distances(self) -> usize {
        match self {
            Self::Deflate => 30,
            #[cfg(feature = "deflate64")]
            Self::Deflate64 => 32,
        }
    }

    /// What a window's buffer holds at most: the window, and room to
    /// decode three chunks before it must move the window to its start.
    const fn buffer(self) -> usize {
        self.window()
            .saturating_add(CHUNK.saturating_mul(3))
            .saturating_add(self.longest())
            .saturating_add(SLACK)
    }

    /// Returns what literal/length symbol `symbol` means, as its table
    /// holds it: a literal, the block's end, or a length's base and how
    /// many extra bits follow, or that it never occurs.
    fn literal(self, symbol: u16) -> (u16, u8) {
        let length = usize::from(symbol).checked_sub(257);
        match (self, symbol, length) {
            (_, 0..=255, _) => (symbol, LITERAL),
            (_, 256, _) => (0, END),
            #[cfg(feature = "deflate64")]
            (Self::Deflate64, 285, _) => (3, 16),
            (_, _, Some(code)) => LENGTHS.get(code).copied().unwrap_or((0, NEVER)),
            _ => (0, NEVER),
        }
    }

    /// Returns what distance symbol `symbol` means, as its table holds it:
    /// a distance's base and how many extra bits follow, or that it never
    /// occurs.
    fn distance(self, symbol: u16) -> (u16, u8) {
        let code = usize::from(symbol);
        let distance = DISTANCES.get(code).filter(|_| code < self.distances());
        distance.copied().unwrap_or((0, NEVER))
    }
}

/// The kinds of literal/length and distance symbols, beside how many extra
/// bits follow a length's or a distance's base, 16 at most: a literal, the
/// block's end, and a symbol that never occurs.
const LITERAL: u8 = 0x40;
const END: u8 = 0x20;
const NEVER: u8 = 0x80;

/// Each length code's base and extra bits, from code 257 (RFC 1951
/// §3.2.5), but for 285, which the format gives; codes 286 and 287 never
/// occur.
const LENGTHS: [(u16, u8); 29] = [
    (3, 0),
    (4, 0),
    (5, 0),
    (6, 0),
    (7, 0),
    (8, 0),
    (9, 0),
    (10, 0),
    (11, 1),
    (13, 1),
    (15, 1),
    (17, 1),
    (19, 2),
    (23, 2),
    (27, 2),
    (31, 2),
    (35, 3),
    (43, 3),
    (51, 3),
    (59, 3),
    (67, 4),
    (83, 4),
    (99, 4),
    (115, 4),
    (131, 5),
    (163, 5),
    (195, 5),
    (227, 5),
    (258, 0),
];

/// Each distance code's base and extra bits (§3.2.5); codes 30 and 31,
/// Deflate64's, never occur in Deflate.
const DISTANCES: [(u16, u8); 32] = [
    (1, 0),
    (2, 0),
    (3, 0),
    (4, 0),
    (5, 1),
    (7, 1),
    (9, 2),
    (13, 2),
    (17, 3),
    (25, 3),
    (33, 4),
    (49, 4),
    (65, 5),
    (97, 5),
    (129, 6),
    (193, 6),
    (257, 7),
    (385, 7),
    (513, 8),
    (769, 8),
    (1025, 9),
    (1537, 9),
    (2049, 10),
    (3073, 10),
    (4097, 11),
    (6145, 11),
    (8193, 12),
    (12289, 12),
    (16385, 13),
    (24577, 13),
    (32769, 14),
    (49153, 14),
];

/// The order in which a dynamic block lists its code length code's
/// lengths (§3.2.7).
const ORDER: [usize; 19] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
];

/// The bits that index each table before its tables of longer codes.
const LITERAL_BITS: u32 = 10;
const DISTANCE_BITS: u32 = 8;
const LENGTH_BITS: u32 = 7;

/// The most literal/length and distance codes a dynamic block may have.
const MAX_LITERALS: usize = 286;
const MAX_DISTANCES: usize = 32;

/// Returns the low `n` bits set.
fn mask(n: u32) -> u64 {
    1_u64
        .checked_shl(n)
        .map_or(u64::MAX, |bit| bit.wrapping_sub(1))
}

/// The bytes given to one call, and how many of them it has taken.
struct Input<'a> {
    bytes: &'a [u8],
    taken: usize,
}

/// The bits taken from the input and not yet read, the next in the
/// lowest; those above `count` are zero, but in `fast`, where they are the
/// input's next bits, as `refill_word` leaves them.
#[derive(Clone, Copy, Debug, Default)]
struct Bits {
    value: u64,
    count: u32,
}

impl Bits {
    /// Takes whole bytes from `input` until it holds more than 55 bits, or
    /// the input runs out.
    fn refill(&mut self, input: &mut Input<'_>) {
        self.refill_from(input.bytes, &mut input.taken);
    }

    /// Takes whole bytes from `bytes` past `taken`, as `refill` does.
    #[inline]
    fn refill_from(&mut self, bytes: &[u8], taken: &mut usize) {
        if self.count > 55 {
            return;
        }
        let rest = bytes.get(*taken..).unwrap_or_default();
        if let Some(word) = rest.first_chunk::<8>() {
            let take = 63_u32.saturating_sub(self.count) / 8;
            self.value |= u64::from_le_bytes(*word).wrapping_shl(self.count);
            self.count = self.count.saturating_add(take.saturating_mul(8));
            self.value &= mask(self.count);
            *taken = taken.saturating_add(usize::try_from(take).unwrap_or_default());
            return;
        }
        for &byte in rest {
            if self.count > 55 {
                break;
            }
            self.value |= u64::from(byte).wrapping_shl(self.count);
            self.count = self.count.saturating_add(8);
            *taken = taken.saturating_add(1);
        }
    }

    /// Takes whole bytes from `word`, the input's next eight past `taken`,
    /// until it holds 56 bits or more, as `refill` does, but without a
    /// branch, and leaving above them the word's next bits.
    #[inline]
    fn refill_word(&mut self, word: [u8; 8], taken: &mut usize) {
        self.value |= u64::from_le_bytes(word).wrapping_shl(self.count);
        let take = 63_u32.wrapping_sub(self.count) >> 3;
        *taken = taken.wrapping_add(usize::try_from(take).unwrap_or_default());
        self.count |= 56;
    }

    /// Reads past the next `n` bits, which it holds.
    fn consume(&mut self, n: u32) {
        self.value = self.value.wrapping_shr(n);
        self.count = self.count.saturating_sub(n);
    }

    /// Reads the next `n` bits, up to 32, which it holds, as a number.
    fn take(&mut self, n: u32) -> u32 {
        let low = self.value & !u64::MAX.wrapping_shl(n);
        self.consume(n);
        u32::try_from(low).unwrap_or_default()
    }

    /// Gives back to `input` the whole bytes it took and does not need.
    fn give_back(&mut self, input: &mut Input<'_>) {
        let whole = usize::try_from(self.count / 8).unwrap_or_default();
        let whole = whole.min(input.taken);
        input.taken = input.taken.saturating_sub(whole);
        let bits = u32::try_from(whole).unwrap_or_default().saturating_mul(8);
        self.count = self.count.saturating_sub(bits);
        self.value &= mask(self.count);
    }
}

/// What a stream has decoded: at least the window of the latest bytes,
/// which matches copy from, and those not yet produced, `end` of them, in
/// a buffer with room past them.
#[derive(Clone, Debug, Default)]
struct Window {
    format: Format,
    buf: Vec<u8>,
    end: usize,
    /// How many of the bytes have been produced.
    produced: usize,
}

impl Window {
    /// Returns how many bytes are decoded and not yet produced.
    fn held(&self) -> usize {
        self.end.saturating_sub(self.produced)
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

    /// Makes room to decode a chunk: moves the latest window, and what is
    /// not yet produced, to the buffer's start, if the buffer has no room
    /// past them; or grows the buffer, doubling, so that small streams
    /// take little.
    fn prepare(&mut self) {
        let need = CHUNK
            .saturating_add(self.format.longest())
            .saturating_add(SLACK);
        let buffer = self.format.buffer();
        if self.end.saturating_add(need) > buffer {
            self.compact();
        }
        let want = self.end.saturating_add(need);
        if self.buf.len() < want {
            let grown = want.max(self.buf.len().saturating_mul(2)).min(buffer);
            self.buf.resize(grown.max(want), 0);
        }
    }

    /// Moves the latest window, and what is not yet produced, to the
    /// buffer's start, dropping what is before them.
    fn compact(&mut self) {
        let keep = self
            .end
            .saturating_sub(self.format.window())
            .min(self.produced);
        self.buf.copy_within(keep..self.end, 0);
        self.end = self.end.saturating_sub(keep);
        self.produced = self.produced.saturating_sub(keep);
    }
}

/// A dynamic block's description of its codes (§3.2.7), read so far.
#[derive(Clone, Debug)]
struct Dynamic {
    /// The counts of literal/length, distance, and code length codes.
    counts: Option<(usize, usize, usize)>,
    /// The code length code's lengths, and then the code itself.
    lengths_of_lengths: [u8; 19],
    code: Option<Table>,
    /// The lengths of the literal/length and distance codes, as one.
    lengths: [u8; MAX_LITERALS + MAX_DISTANCES],
    /// How many lengths have been read, of either kind.
    read: usize,
}

impl Dynamic {
    fn new() -> Self {
        Self {
            counts: None,
            lengths_of_lengths: [0; 19],
            code: None,
            lengths: [0; MAX_LITERALS + MAX_DISTANCES],
            read: 0,
        }
    }

    /// Reads what it can of the description, and returns the block's
    /// literal/length and distance codes once it has read it all.
    fn read(
        &mut self,
        bits: &mut Bits,
        input: &mut Input<'_>,
        format: Format,
    ) -> Result<Option<(Table, Table)>, Error> {
        bits.refill(input);
        let (literals, distances, count) = match self.counts {
            Some(counts) => counts,
            None if bits.count < 14 => return Ok(None),
            None => {
                let literals = usize::try_from(bits.take(5)).map_err(|_| corrupt())?;
                let distances = usize::try_from(bits.take(5)).map_err(|_| corrupt())?;
                let count = usize::try_from(bits.take(4)).map_err(|_| corrupt())?;
                let counts = (
                    literals.saturating_add(257),
                    distances.saturating_add(1),
                    count.saturating_add(4),
                );
                if counts.0 > MAX_LITERALS || counts.1 > format.distances() {
                    return Err(corrupt());
                }
                self.counts = Some(counts);
                counts
            },
        };
        if self.code.is_none() {
            while self.read < count {
                bits.refill(input);
                if bits.count < 3 {
                    return Ok(None);
                }
                let symbol = ORDER.get(self.read).copied().ok_or_else(corrupt)?;
                let length = u8::try_from(bits.take(3)).map_err(|_| corrupt())?;
                *self
                    .lengths_of_lengths
                    .get_mut(symbol)
                    .ok_or_else(corrupt)? = length;
                self.read = self.read.saturating_add(1);
            }
            self.read = 0;
            self.code = Some(Table::new(
                &self.lengths_of_lengths,
                LENGTH_BITS,
                Kind::Lengths,
                |symbol| (symbol, 0),
            )?);
        }
        let code = self.code.as_ref().ok_or_else(corrupt)?;
        let total = literals.saturating_add(distances);
        while self.read < total {
            bits.refill(input);
            let (symbol, len) = match code.lookup(bits.value, bits.count) {
                Code::Symbol { value, bits, .. } => (value, bits),
                Code::Short => return Ok(None),
                Code::Invalid => return Err(corrupt()),
            };
            // A length, or a length repeated: the last, or zeros.
            let (extra, base) = match symbol {
                0..=15 => (0, 1),
                16 => (2, 3),
                17 => (3, 3),
                _ => (7, 11),
            };
            if bits.count < len.saturating_add(extra) {
                return Ok(None);
            }
            bits.consume(len);
            let repeat =
                usize::try_from(bits.take(extra).saturating_add(base)).map_err(|_| corrupt())?;
            let length = match symbol {
                0..=15 => u8::try_from(symbol).map_err(|_| corrupt())?,
                16 => {
                    let last = self.read.checked_sub(1).ok_or_else(corrupt)?;
                    self.lengths.get(last).copied().ok_or_else(corrupt)?
                },
                _ => 0,
            };
            let end = self.read.saturating_add(repeat);
            if end > total {
                return Err(corrupt());
            }
            self.lengths
                .get_mut(self.read..end)
                .ok_or_else(corrupt)?
                .fill(length);
            self.read = end;
        }
        let lengths = self.lengths.get(..total).ok_or_else(corrupt)?;
        let (literal_lengths, distance_lengths) = lengths.split_at(literals);
        // A block must be able to end.
        if literal_lengths.get(256).is_none_or(|&length| length == 0) {
            return Err(corrupt());
        }
        let literals = Table::new(literal_lengths, LITERAL_BITS, Kind::Literals, |symbol| {
            format.literal(symbol)
        })?;
        let distances = Table::new(distance_lengths, DISTANCE_BITS, Kind::Distances, |symbol| {
            format.distance(symbol)
        })?;
        Ok(Some((literals, distances)))
    }
}

/// Where a [`Decoder`] is in its stream.
#[derive(Clone, Debug)]
enum State {
    /// Before a block.
    Header,
    /// In a stored block, before its length (§3.2.4).
    StoredLength,
    /// In a stored block, with this many bytes left.
    Stored(u32),
    /// In a dynamic block's description of its codes.
    Dynamic(Box<Dynamic>),
    /// Before a literal/length code.
    Literal,
    /// Before the distance of a match of this length.
    Distance(u32),
    /// Past the last block.
    End,
}

/// Why a decoder stopped decoding.
enum Flow {
    /// It needs more input.
    More,
    /// It holds as much as it decodes at once, or it has ended.
    Done,
}

/// A decoder of a raw Deflate stream (RFC 1951), as ZIP stores members
/// and gzip and zlib frame them.
///
/// It consumes the stream to its end and no further, so that a container
/// reads on from what follows. It holds, of what it decodes, the 32 KiB
/// window and twice as much again at most; and of its input, at most the
/// bits of a code that its input has not yet given it whole.
///
/// # Examples
///
/// ```
/// use decode_deflate::Decoder;
///
/// // "hello", deflated as one block of fixed codes, and what follows.
/// let packed = [0xCB, 0x48, 0xCD, 0xC9, 0xC9, 0x07, 0x00, 0xFF];
/// let mut decoder = Decoder::new();
/// let mut output = [0; 8];
/// assert_eq!(decoder.decode(&packed, &mut output)?, (7, 5));
/// assert_eq!(&output[..5], b"hello");
/// decoder.finish()?;
/// # Ok::<(), decode_deflate::Error>(())
/// ```
#[derive(Clone)]
pub struct Decoder {
    bits: Bits,
    state: State,
    /// Whether the block being decoded is the last.
    last: bool,
    window: Window,
    literals: Table,
    distances: Table,
}

impl Decoder {
    /// Returns a decoder at the start of a stream.
    #[must_use]
    pub fn new() -> Self {
        Self {
            bits: Bits::default(),
            state: State::Header,
            last: false,
            window: Window::default(),
            literals: Table::default(),
            distances: Table::default(),
        }
    }

    /// Returns a decoder at the start of a Deflate64 stream: PKWARE's
    /// variant of Deflate, ZIP's method 9, with a 64 KiB window, length
    /// code 285 as 3 and 16 extra bits, and distance codes 30 and 31.
    #[cfg(feature = "deflate64")]
    #[must_use]
    pub fn deflate64() -> Self {
        let mut decoder = Self::new();
        decoder.window.format = Format::Deflate64;
        decoder
    }

    /// Decodes what it can of `input` into `output`, and returns how many
    /// bytes of each it consumed and produced.
    ///
    /// It returns neither consumed nor produced once it can go no further:
    /// needing more input, or more room, or past the end of the stream,
    /// which [`finish`](Self::finish) then tells apart. It consumes no
    /// byte past the stream's end, but for the bits of its last byte.
    ///
    /// # Errors
    ///
    /// [`Corrupt`](Error::Corrupt) for a malformed stream. The decoder is
    /// not to be used after an error.
    pub fn decode(&mut self, input: &[u8], output: &mut [u8]) -> Result<(usize, usize), Error> {
        let mut input = Input {
            bytes: input,
            taken: 0,
        };
        let mut produced = 0_usize;
        loop {
            let room = output.get_mut(produced..).unwrap_or_default();
            produced = produced.saturating_add(self.window.produce(room));
            if self.window.held() > 0 || matches!(self.state, State::End) {
                self.bits.give_back(&mut input);
                return Ok((input.taken, produced));
            }
            self.window.prepare();
            if let Flow::More = self.run(&mut input)? {
                // Every bit it holds is part of what it needs next.
                return Ok((input.taken, produced));
            }
        }
    }

    /// Checks that the stream has ended: its last block decoded, and all
    /// that it decoded to produced.
    ///
    /// # Errors
    ///
    /// [`Corrupt`](Error::Corrupt) if it has not.
    pub fn finish(&self) -> Result<(), Error> {
        if matches!(self.state, State::End) && self.window.held() == 0 {
            Ok(())
        } else {
            Err(corrupt())
        }
    }

    /// Returns a decoder that goes on from where this one is: a copy of
    /// it, with no more of its window than matches may copy from.
    #[must_use]
    pub fn checkpoint(&self) -> Option<Self> {
        let mut copy = self.clone();
        copy.window.compact();
        copy.window.buf.truncate(copy.window.end);
        copy.window.buf.shrink_to_fit();
        Some(copy)
    }

    /// Returns how many of the bytes it has consumed a
    /// [`checkpoint`](Self::checkpoint) must be given again: none, as it
    /// holds them.
    #[must_use]
    pub const fn buffered(&self) -> usize {
        0
    }

    /// Decodes until it holds a chunk to produce, the stream ends, or it
    /// needs more input.
    fn run(&mut self, input: &mut Input<'_>) -> Result<Flow, Error> {
        loop {
            if self.window.held() >= CHUNK {
                return Ok(Flow::Done);
            }
            match &mut self.state {
                State::Header => {
                    self.bits.refill(input);
                    if self.bits.count < 3 {
                        return Ok(Flow::More);
                    }
                    let header = self.bits.take(3);
                    self.last = header & 1 != 0;
                    self.state = match header >> 1 {
                        0 => State::StoredLength,
                        1 => {
                            self.fixed()?;
                            State::Literal
                        },
                        2 => State::Dynamic(Box::new(Dynamic::new())),
                        _ => return Err(corrupt()),
                    };
                },
                State::StoredLength => {
                    // The rest of the byte is ignored.
                    self.bits.consume(self.bits.count % 8);
                    self.bits.refill(input);
                    if self.bits.count < 32 {
                        return Ok(Flow::More);
                    }
                    let len = self.bits.take(16);
                    if self.bits.take(16) != !len & 0xFFFF {
                        return Err(corrupt());
                    }
                    self.state = State::Stored(len);
                },
                State::Stored(0) => self.ended(),
                State::Stored(left) => {
                    let room = CHUNK.saturating_sub(self.window.held());
                    let want = usize::try_from(*left).unwrap_or_default().min(room);
                    let window = &mut self.window;
                    // What the bits hold first, whole bytes as they are.
                    let n = if self.bits.count >= 8 {
                        let n = want.min(usize::try_from(self.bits.count / 8).unwrap_or_default());
                        for _ in 0..n {
                            let byte = u8::try_from(self.bits.take(8)).unwrap_or_default();
                            *window.buf.get_mut(window.end).ok_or_else(corrupt)? = byte;
                            window.end = window.end.saturating_add(1);
                        }
                        n
                    } else {
                        let rest = input.bytes.get(input.taken..).unwrap_or_default();
                        let bytes = rest.get(..want.min(rest.len())).unwrap_or_default();
                        let to = window.end..window.end.saturating_add(bytes.len());
                        window
                            .buf
                            .get_mut(to)
                            .ok_or_else(corrupt)?
                            .copy_from_slice(bytes);
                        window.end = window.end.saturating_add(bytes.len());
                        input.taken = input.taken.saturating_add(bytes.len());
                        bytes.len()
                    };
                    if n == 0 {
                        return Ok(Flow::More);
                    }
                    *left = left.saturating_sub(u32::try_from(n).unwrap_or(u32::MAX));
                },
                State::Dynamic(dynamic) => {
                    let Some((literals, distances)) =
                        dynamic.read(&mut self.bits, input, self.window.format)?
                    else {
                        return Ok(Flow::More);
                    };
                    (self.literals, self.distances) = (literals, distances);
                    self.state = State::Literal;
                },
                State::Literal | State::Distance(_) => {
                    if !self.codes(input)? {
                        return Ok(Flow::More);
                    }
                },
                State::End => return Ok(Flow::Done),
            }
        }
    }

    /// Decodes codes until it holds a chunk, the block ends, or it needs
    /// more input, and returns whether it can go on.
    fn codes(&mut self, input: &mut Input<'_>) -> Result<bool, Error> {
        // The bits, and a length decoded and its distance not, in locals,
        // which the loop keeps in registers.
        let bits = self.bits;
        let mut length = match self.state {
            State::Distance(length) => Some(length),
            _ => None,
        };
        let window = &mut self.window;
        let limit = window.produced.saturating_add(CHUNK);
        let at = Cursor {
            bits,
            end: window.end,
            taken: input.taken,
            length,
        };
        let tables = (&self.literals, &self.distances);
        let format = window.format;
        let (stop, at) = codes(at, tables, format, &mut window.buf, limit, input.bytes);
        (self.bits, window.end, input.taken) = (at.bits, at.end, at.taken);
        length = at.length;
        self.state = length.map_or(State::Literal, State::Distance);
        match stop? {
            Stop::More => Ok(false),
            Stop::Full => Ok(true),
            Stop::End => {
                self.ended();
                Ok(true)
            },
        }
    }

    /// Takes on the fixed codes (§3.2.6): the literal/length codes 286 and
    /// 287, and the distance codes 30 and 31, take part, but never occur.
    fn fixed(&mut self) -> Result<(), Error> {
        let mut literals = [8_u8; 288];
        for (symbol, length) in literals.iter_mut().enumerate() {
            *length = match symbol {
                0..=143 | 280.. => 8,
                144..=255 => 9,
                _ => 7,
            };
        }
        let format = self.window.format;
        self.literals = Table::new(&literals, LITERAL_BITS, Kind::Literals, |symbol| {
            format.literal(symbol)
        })?;
        self.distances = Table::new(&[5; 32], DISTANCE_BITS, Kind::Distances, |symbol| {
            format.distance(symbol)
        })?;
        Ok(())
    }

    /// Ends the block being decoded.
    fn ended(&mut self) {
        self.state = if self.last { State::End } else { State::Header };
    }
}

/// Why decoding codes stopped.
enum Stop {
    /// It needs more input.
    More,
    /// It holds a chunk.
    Full,
    /// The block ended.
    End,
}

/// Where decoding codes is: the bits, the end of what is decoded, the
/// input taken, and a match's length, decoded before its distance; by
/// value, so that the loop keeps them in registers.
#[derive(Clone, Copy)]
struct Cursor {
    bits: Bits,
    end: usize,
    taken: usize,
    length: Option<u32>,
}

/// Decodes literal/length and distance codes with `tables`, from `at`'s
/// bits, refilled from `input`, onto `buf`, until it reaches `limit`, the
/// block ends, or it needs more input; and returns why it stopped, and
/// where.
fn codes(
    mut at: Cursor,
    (literals, distances): (&Table, &Table),
    format: Format,
    buf: &mut [u8],
    limit: usize,
    input: &[u8],
) -> (Result<Stop, Error>, Cursor) {
    if at.length.is_none() {
        match fast(&mut at, (literals, distances), format, buf, limit, input) {
            Ok(None) => {},
            Ok(Some(stop)) => return (Ok(stop), at),
            Err(error) => return (Err(error), at),
        }
    }
    let stop = loop {
        if at.end >= limit {
            break Ok(Stop::Full);
        }
        let len = match at.length.take() {
            Some(len) => len,
            None => {
                at.bits.refill_from(input, &mut at.taken);
                let (value, kind, n) = match literals.lookup(at.bits.value, at.bits.count) {
                    Code::Symbol { value, kind, bits } => (value, kind, bits),
                    Code::Short => break Ok(Stop::More),
                    Code::Invalid => break Err(corrupt()),
                };
                if kind == u32::from(LITERAL) {
                    let Some(slot) = buf.get_mut(at.end) else {
                        break Err(corrupt());
                    };
                    [*slot, ..] = value.to_le_bytes();
                    at.bits.consume(n);
                    at.end = at.end.wrapping_add(1);
                    continue;
                }
                if kind == u32::from(END) {
                    at.bits.consume(n);
                    break Ok(Stop::End);
                }
                if kind & u32::from(NEVER) != 0 {
                    break Err(corrupt());
                }
                if at.bits.count < n.saturating_add(kind) {
                    break Ok(Stop::More);
                }
                at.bits.consume(n);
                value.wrapping_add(at.bits.take(kind))
            },
        };
        at.bits.refill_from(input, &mut at.taken);
        let (base, extra, n) = match distances.lookup(at.bits.value, at.bits.count) {
            Code::Symbol { value, kind, bits } => (value, kind, bits),
            Code::Short => {
                at.length = Some(len);
                break Ok(Stop::More);
            },
            Code::Invalid => break Err(corrupt()),
        };
        if extra & u32::from(NEVER) != 0 {
            break Err(corrupt());
        }
        if at.bits.count < n.saturating_add(extra) {
            at.length = Some(len);
            break Ok(Stop::More);
        }
        at.bits.consume(n);
        let distance = base.wrapping_add(at.bits.take(extra));
        let (Ok(distance), Ok(len)) = (usize::try_from(distance), usize::try_from(len)) else {
            break Err(corrupt());
        };
        match copy(buf, at.end, distance, len, format.window()) {
            Ok(end) => at.end = end,
            Err(error) => break Err(error),
        }
    };
    (stop, at)
}

/// Decodes codes as `codes` does, while the input holds two words past
/// what is taken, so that every refill leaves at least 56 bits: enough for
/// two literal/length codes, and a length's extra bits, and, refilled
/// again if they run short, a distance code and its own. So no code can
/// find too few. Its tables hold each symbol's meaning, so that a length
/// or distance takes no lookup of its own, and its refills take no branch,
/// leaving above the bits they need the input's next. It returns why it
/// stopped, or nothing once the input runs short, for `codes` to go on
/// carefully.
#[inline]
fn fast(
    at: &mut Cursor,
    (literals, distances): (&Table, &Table),
    format: Format,
    buf: &mut [u8],
    limit: usize,
    input: &[u8],
) -> Result<Option<Stop>, Error> {
    while at.end < limit {
        let rest = input.get(at.taken..).unwrap_or_default();
        let Some((word, more)) = rest.split_first_chunk::<8>() else {
            return Ok(None);
        };
        if more.len() < 8 {
            return Ok(None);
        }
        at.bits.refill_word(*word, &mut at.taken);
        let (mut value, mut kind, n) = literals.decode(at.bits.value).ok_or_else(corrupt)?;
        at.bits.consume(n);
        if kind == u32::from(LITERAL) {
            [*buf.get_mut(at.end).ok_or_else(corrupt)?, ..] = value.to_le_bytes();
            at.end = at.end.wrapping_add(1);
            // A second code needs no refill, leaving 26 bits at least.
            let n;
            (value, kind, n) = literals.decode(at.bits.value).ok_or_else(corrupt)?;
            at.bits.consume(n);
            if kind == u32::from(LITERAL) {
                [*buf.get_mut(at.end).ok_or_else(corrupt)?, ..] = value.to_le_bytes();
                at.end = at.end.wrapping_add(1);
                continue;
            }
        }
        if kind == u32::from(END) {
            return Ok(Some(Stop::End));
        }
        if kind & u32::from(NEVER) != 0 {
            return Err(corrupt());
        }
        let length = value.wrapping_add(at.bits.take(kind));
        // A distance code and its extra bits take up to 29.
        if at.bits.count < 29 {
            let rest = input.get(at.taken..).unwrap_or_default();
            let Some(word) = rest.first_chunk::<8>() else {
                at.length = Some(length);
                return Ok(None);
            };
            at.bits.refill_word(*word, &mut at.taken);
        }
        let (base, extra, n) = distances.decode(at.bits.value).ok_or_else(corrupt)?;
        if extra & u32::from(NEVER) != 0 {
            return Err(corrupt());
        }
        at.bits.consume(n);
        let distance = base.wrapping_add(at.bits.take(extra));
        let distance = usize::try_from(distance).map_err(|_| corrupt())?;
        let length = usize::try_from(length).map_err(|_| corrupt())?;
        at.end = copy(buf, at.end, distance, length, format.window())?;
    }
    Ok(Some(Stop::Full))
}

/// Copies to `buf` past `end` `length` bytes from `distance` back, no
/// further than `window`, which they may overlap, repeating what they
/// copy, and returns the new end. It may write up to `SLACK` bytes past
/// that end.
fn copy(
    buf: &mut [u8],
    end: usize,
    distance: usize,
    length: usize,
    window: usize,
) -> Result<usize, Error> {
    let from = end.checked_sub(distance);
    let mut from = from.filter(|_| distance <= window).ok_or_else(corrupt)?;
    let stop = end.saturating_add(length);
    let mut to = end;
    match distance {
        1 => {
            let byte = buf.get(from).copied().ok_or_else(corrupt)?;
            buf.get_mut(to..stop).ok_or_else(corrupt)?.fill(byte);
        },
        16.. => chunks::<16>(buf, from, to, stop)?,
        8.. => chunks::<8>(buf, from, to, stop)?,
        _ => {
            // Byte by byte until what is written repeats with a period of
            // 8 or more, a multiple of the distance; then in chunks.
            let period = 8_usize.div_ceil(distance).saturating_mul(distance);
            let head = stop.min(end.saturating_add(period));
            while to < head {
                *buf.get_mut(to).ok_or_else(corrupt)? =
                    buf.get(from).copied().ok_or_else(corrupt)?;
                (from, to) = (from.saturating_add(1), to.saturating_add(1));
            }
            chunks::<8>(buf, to.saturating_sub(period), to, stop)?;
        },
    }
    Ok(stop)
}

/// Copies to `buf` from `from` to `to`, until `stop`, in chunks of `N`
/// bytes, up to `N` past it: chunks that do not overlap what they copy, as
/// `to` is at least `N` past `from`, each after the last has written what
/// it reads.
fn chunks<const N: usize>(
    buf: &mut [u8],
    mut from: usize,
    mut to: usize,
    stop: usize,
) -> Result<(), Error> {
    while to < stop {
        let chunk: [u8; N] = *buf
            .get(from..)
            .and_then(|rest| rest.first_chunk())
            .ok_or_else(corrupt)?;
        *buf.get_mut(to..)
            .and_then(|rest| rest.first_chunk_mut())
            .ok_or_else(corrupt)? = chunk;
        (from, to) = (from.wrapping_add(N), to.wrapping_add(N));
    }
    Ok(())
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
            .field("last", &self.last)
            .finish_non_exhaustive()
    }
}

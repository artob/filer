// This is free and unencumbered software released into the public domain.

use crate::{
    Error, corrupt,
    huffman::{
        self, Code, DOUBLE_ENTRY, END, END_ENTRY, Entry, INVALID, Kind, LINK, LITERAL,
        LITERAL_ENTRY, NEVER, SPECIAL, Table, end_length, ends,
    },
};
use alloc::{boxed::Box, vec::Vec};

/// The most a call decodes before it produces what it decoded; and the
/// least of a decoder's first, which is as large as its caller has room
/// for, so that a small stream takes little.
const CHUNK: usize = 32 * 1024;
const FIRST_CHUNK: usize = 1024;

/// How much input left at a block's start has a decoder pair the block's
/// literals at once, as a block likely long enough for pairing them to
/// pay; and how much of a block it decodes before it pairs them, if it
/// did not: pairing takes about as long as decoding that much saves.
const PAIR_NOW: usize = 2048;
const PAIR_AFTER: usize = 2048;

/// How far past its end a match may write, as it copies in chunks of up
/// to 32.
const SLACK: usize = 32;

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
    /// decode three chunks before it must move the window to its start,
    /// or two and the longest match, if that is more, as it is for
    /// Deflate64, whose longest match is longer than a chunk.
    const fn buffer(self) -> usize {
        let longest = self.longest();
        let more = if longest > CHUNK { longest } else { CHUNK };
        self.window()
            .saturating_add(CHUNK.saturating_mul(2))
            .saturating_add(more)
            .saturating_add(longest)
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

/// The bits that index each table before its tables of longer codes, and
/// how many entries they index.
const LITERAL_BITS: u32 = 10;
const DISTANCE_BITS: u32 = 8;
const LENGTH_BITS: u32 = 7;
const LITERAL_SIZE: usize = 1 << LITERAL_BITS;
const DISTANCE_SIZE: usize = 1 << DISTANCE_BITS;

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

    /// Takes whole bytes from `bytes` past `taken`, as `refill` does, first
    /// dropping the bits above `count` that `refill_word` leaves: the next
    /// byte's, which a stored block, copying it straight from the input,
    /// would leave stale, to be read into the block after it.
    #[inline]
    fn refill_from(&mut self, bytes: &[u8], taken: &mut usize) {
        self.value &= mask(self.count);
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
        let take = usize::try_from((63_u32.wrapping_sub(self.count) >> 3) & 7).unwrap_or_default();
        *taken = taken.wrapping_add(take);
        self.count |= 56;
    }

    /// Reads past the next `n` bits, which it holds: every caller checks
    /// `count` first, or takes a code that a lookup found it holds. So
    /// `count` never wraps, and wrapping, not saturating, keeps it off the
    /// chain from code to code.
    fn consume(&mut self, n: u32) {
        self.value = self.value.wrapping_shr(n);
        self.count = self.count.wrapping_sub(n);
    }

    /// Reads past the bits that the symbol of `entry` takes, its length
    /// byte, as `fast` does: subtracting the whole entry from `count`,
    /// whose low byte is all that `fast` reads, and keeps true, so that
    /// none is taken out of the entry first. `fast` clears the rest once
    /// it is done.
    #[inline(always)]
    fn skip(&mut self, entry: Entry) {
        self.value = self.value.wrapping_shr(entry);
        self.count = self.count.wrapping_sub(entry);
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
    /// How much it decodes before it produces it, which `prepare` makes
    /// room for: `CHUNK`, or less for its first.
    chunk: usize,
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

    /// Makes room to decode a chunk, the first as large as `room`, what the
    /// caller has room for, within bounds: moves the latest window, and
    /// what is not yet produced, to the buffer's start, if the buffer has
    /// no room past them; or grows the buffer, doubling.
    fn prepare(&mut self, room: usize) {
        self.chunk = match self.buf.is_empty() {
            true => CHUNK.min(FIRST_CHUNK.max(room)),
            false => CHUNK,
        };
        let need = self
            .chunk
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

    /// Takes `bytes`, produced straight from the input, as the latest it
    /// decoded, while it holds none not yet produced: as many of the last
    /// of them as matches may copy from.
    fn keep(&mut self, bytes: &[u8]) {
        let window = self.format.window();
        match bytes.len().checked_sub(window) {
            Some(older) => {
                if self.buf.len() < window {
                    self.buf.resize(window, 0);
                }
                let (Some(to), Some(from)) = (self.buf.get_mut(..window), bytes.get(older..))
                else {
                    return;
                };
                to.copy_from_slice(from);
                (self.end, self.produced) = (window, window);
            },
            None => {
                self.prepare(bytes.len());
                let to = self.end..self.end.saturating_add(bytes.len());
                if let Some(to) = self.buf.get_mut(to) {
                    to.copy_from_slice(bytes);
                }
                self.end = self.end.saturating_add(bytes.len());
                self.produced = self.end;
            },
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

/// Adds `n` to the tally of `length`, 15 at most.
macro_rules! tally {
    ($tally:expr, $length:expr, $n:expr) => {
        if let Some(count) = $tally.get_mut(usize::from($length)) {
            *count = count.wrapping_add(u32::try_from($n).unwrap_or_default());
        }
    };
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
    /// How many of them are each length.
    tally: [u32; huffman::LENGTHS],
}

impl Dynamic {
    /// Reads a length, or a length repeated, from `bits`, which hold all
    /// its bits, into `lengths` at `read`, up to `total`, with `code`, the
    /// code length code, and returns where the next goes.
    #[inline(always)]
    fn length(
        (lengths, tally): (
            &mut [u8; MAX_LITERALS + MAX_DISTANCES],
            &mut [u32; huffman::LENGTHS],
        ),
        read: usize,
        total: usize,
        code: &Table,
        bits: &mut Bits,
    ) -> Result<usize, Error> {
        let (symbol, len) = match code.lookup(bits.value, bits.count) {
            Code::Symbol { value, bits, .. } => (value, bits),
            _ => return Err(corrupt()),
        };
        bits.consume(len);
        let (length, repeat) = match symbol {
            0..=15 => {
                let length = u8::try_from(symbol).map_err(|_| corrupt())?;
                *lengths
                    .get_mut(read)
                    .filter(|_| read < total)
                    .ok_or_else(corrupt)? = length;
                tally!(tally, length, 1);
                return Ok(read.saturating_add(1));
            },
            16 => {
                let last = read.checked_sub(1).ok_or_else(corrupt)?;
                let last = lengths.get(last).copied().ok_or_else(corrupt)?;
                (last, bits.take(2).saturating_add(3))
            },
            17 => (0, bits.take(3).saturating_add(3)),
            _ => (0, bits.take(7).saturating_add(11)),
        };
        let end = read.saturating_add(usize::try_from(repeat).map_err(|_| corrupt())?);
        if end > total {
            return Err(corrupt());
        }
        lengths.get_mut(read..end).ok_or_else(corrupt)?.fill(length);
        tally!(tally, length, repeat);
        Ok(end)
    }

    fn new() -> Self {
        Self {
            counts: None,
            lengths_of_lengths: [0; 19],
            code: None,
            lengths: [0; MAX_LITERALS + MAX_DISTANCES],
            read: 0,
            tally: [0; huffman::LENGTHS],
        }
    }

    /// Reads what it can of the description, and makes `literal_table` and
    /// `distance_table` the block's literal/length and distance codes once
    /// it has read it all, and returns whether it has.
    fn read(
        &mut self,
        bits: &mut Bits,
        input: &mut Input<'_>,
        format: Format,
        (literal_table, distance_table): (&mut Table, &mut Table),
    ) -> Result<bool, Error> {
        bits.refill(input);
        let (literals, distances, count) = match self.counts {
            Some(counts) => counts,
            None if bits.count < 14 => return Ok(false),
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
                    return Ok(false);
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
            // While a word of input is left, four symbols to a refill without
            // a branch, each 14 bits at most.
            if let Some(word) = input
                .bytes
                .get(input.taken..)
                .and_then(<[u8]>::first_chunk::<8>)
            {
                bits.refill_word(*word, &mut input.taken);
                for _ in 0..4 {
                    if self.read >= total {
                        break;
                    }
                    let into = (&mut self.lengths, &mut self.tally);
                    self.read = Self::length(into, self.read, total, code, bits)?;
                }
                // What `refill_word` left above the count's bits.
                bits.value &= mask(bits.count);
                continue;
            }
            bits.refill(input);
            let (symbol, len) = match code.lookup(bits.value, bits.count) {
                Code::Symbol { value, bits, .. } => (value, bits),
                Code::Short => return Ok(false),
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
                return Ok(false);
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
            tally!(self.tally, length, repeat);
            self.read = end;
        }
        let lengths = self.lengths.get(..total).ok_or_else(corrupt)?;
        let (literal_lengths, distance_lengths) = lengths.split_at(literals);
        // A block must be able to end.
        if literal_lengths.get(256).is_none_or(|&length| length == 0) {
            return Err(corrupt());
        }
        // The tally less the distances' lengths, which are few.
        let mut literal_counts = self.tally;
        for &length in distance_lengths {
            if let Some(count) = literal_counts.get_mut(usize::from(length)) {
                *count = count.wrapping_sub(1);
            }
        }
        literal_table.build_counted(
            literal_lengths,
            literal_counts,
            LITERAL_BITS,
            Kind::Literals,
            |symbol| format.literal(symbol),
        )?;
        distance_table.build(distance_lengths, DISTANCE_BITS, Kind::Distances, |symbol| {
            format.distance(symbol)
        })?;
        Ok(true)
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
    /// How much the block being decoded has decoded, while its literals
    /// are not paired.
    unpaired: usize,
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
            unpaired: 0,
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
        // How many of the bytes produced last went straight to the output,
        // which the window is yet to keep the latest of.
        let mut owed = 0_usize;
        loop {
            let room = output.get_mut(produced..).unwrap_or_default();
            produced = produced.saturating_add(self.window.produce(room));
            if self.window.held() > 0 || matches!(self.state, State::End) {
                self.settle(output, produced, owed);
                self.bits.give_back(&mut input);
                return Ok((input.taken, produced));
            }
            let room = output.get_mut(produced..).unwrap_or_default();
            let went = self.straight(&mut input, room);
            if went > 0 {
                produced = produced.saturating_add(went);
                owed = owed.saturating_add(went);
                continue;
            }
            // Codes straight into the output, once it holds a window of what
            // this call produced, from which every match then copies.
            if let Some((end, more)) = self.direct(&mut input, output, produced)? {
                owed = owed.saturating_add(end.saturating_sub(produced));
                produced = end;
                if more {
                    self.settle(output, produced, owed);
                    return Ok((input.taken, produced));
                }
                continue;
            }
            // The window keeps the latest bytes before it takes more, of a
            // stored block's or of codes, which copy from it.
            let takes = match self.state {
                State::Stored(left) => left > 0,
                State::Literal | State::Distance(_) => true,
                _ => false,
            };
            if takes {
                self.settle(output, produced, owed);
                owed = 0;
            }
            // Whether a stored block may go straight to the output.
            let room = output.len().saturating_sub(produced);
            let straight = room >= self.window.format.window() && input.taken < input.bytes.len();
            self.window.prepare(room);
            if let Flow::More = self.run(&mut input, straight, owed > 0)? {
                self.settle(output, produced, owed);
                // Every bit it holds is part of what it needs next; what it
                // decoded before it ran short goes out all the same.
                let room = output.get_mut(produced..).unwrap_or_default();
                produced = produced.saturating_add(self.window.produce(room));
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

    /// Takes what it can of a stored block straight from `input` to
    /// `output`, once it holds nothing not yet produced, the bits hold none
    /// of the block's bytes, and `output` has room for a window at least:
    /// copied once, the window keeping the latest of them only once codes
    /// may copy from them, or the call returns; and returns how many. With
    /// less room, copying them through the window costs no more.
    fn straight(&mut self, input: &mut Input<'_>, output: &mut [u8]) -> usize {
        let State::Stored(left) = &mut self.state else {
            return 0;
        };
        let few = output.len() < self.window.format.window();
        if few || self.bits.count > 0 || self.window.held() > 0 {
            return 0;
        }
        let rest = input.bytes.get(input.taken..).unwrap_or_default();
        let left_now = usize::try_from(*left).unwrap_or(usize::MAX);
        let n = left_now.min(rest.len()).min(output.len());
        let (Some(from), Some(to)) = (rest.get(..n), output.get_mut(..n)) else {
            return 0;
        };
        to.copy_from_slice(from);
        input.taken = input.taken.saturating_add(n);
        *left = left.saturating_sub(u32::try_from(n).unwrap_or(u32::MAX));
        n
    }

    /// Has the window keep the latest of the `owed` bytes produced last,
    /// straight to `output`, below `produced`.
    fn settle(&mut self, output: &[u8], produced: usize, owed: usize) {
        if owed > 0 {
            let from = produced.saturating_sub(owed)..produced;
            self.window.keep(output.get(from).unwrap_or_default());
        }
    }

    /// Decodes until it holds a chunk to produce, the stream ends, or it
    /// needs more input; or until a stored block may go `straight` to the
    /// output, the caller having room for it and input; or until it would
    /// take bytes into the window, if it is `owed` bytes produced straight.
    fn run(&mut self, input: &mut Input<'_>, straight: bool, owed: bool) -> Result<Flow, Error> {
        loop {
            if self.window.held() >= self.window.chunk {
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
                    // The block's bytes the bits took, back to the input, to
                    // go straight to the output with the rest.
                    self.bits.give_back(input);
                    self.state = State::Stored(len);
                },
                State::Stored(0) => self.ended(),
                State::Stored(_) if straight && self.bits.count == 0 && self.window.held() == 0 => {
                    return Ok(Flow::Done);
                },
                State::Stored(_) if owed => return Ok(Flow::Done),
                State::Stored(left) => {
                    let room = self.window.chunk.saturating_sub(self.window.held());
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
                    let tables = (&mut self.literals, &mut self.distances);
                    if !dynamic.read(&mut self.bits, input, self.window.format, tables)? {
                        return Ok(Flow::More);
                    }
                    self.unpaired = 0;
                    // At once, where the input given holds enough for a block
                    // whose pairs pay; else once it has decoded enough.
                    let left = input.bytes.len().saturating_sub(input.taken);
                    if left >= PAIR_NOW {
                        self.literals.pair();
                    }
                    self.state = State::Literal;
                },
                State::Literal | State::Distance(_) if owed => return Ok(Flow::Done),
                State::Literal | State::Distance(_) => {
                    if !self.codes(input)? {
                        return Ok(Flow::More);
                    }
                },
                State::End => return Ok(Flow::Done),
            }
        }
    }

    /// Decodes codes straight into `output` past `produced`, where it has
    /// produced a window at least in this call, and room for a match past
    /// a chunk more: until a match may not fit, the block ends, or it needs
    /// more input; and returns the new end of what it produced, and whether
    /// it needs more input; or `None` where it may not.
    fn direct(
        &mut self,
        input: &mut Input<'_>,
        output: &mut [u8],
        produced: usize,
    ) -> Result<Option<(usize, bool)>, Error> {
        let length = match self.state {
            State::Literal => None,
            State::Distance(length) => Some(length),
            _ => return Ok(None),
        };
        let format = self.window.format;
        let reach = format.longest().saturating_add(SLACK);
        let room = output.len().saturating_sub(produced);
        if produced < format.window() || room < reach.saturating_add(CHUNK) {
            return Ok(None);
        }
        let at = Cursor {
            bits: self.bits,
            end: produced,
            taken: input.taken,
            length,
        };
        let limit = self.pairing(output.len().saturating_sub(reach), produced);
        let tables = (&self.literals, &self.distances);
        let (stop, at) = codes(at, tables, output, limit, input.bytes);
        self.unpaired = self
            .unpaired
            .saturating_add(at.end.saturating_sub(produced));
        (self.bits, input.taken) = (at.bits, at.taken);
        self.state = at.length.map_or(State::Literal, State::Distance);
        let more = match stop? {
            Stop::More => true,
            Stop::Full => false,
            Stop::End => {
                self.ended();
                false
            },
        };
        Ok(Some((at.end, more)))
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
        let begun = self.window.end;
        let chunk = self.window.chunk;
        let limit = self.pairing(self.window.produced.saturating_add(chunk), begun);
        let window = &mut self.window;
        let at = Cursor {
            bits,
            end: window.end,
            taken: input.taken,
            length,
        };
        let tables = (&self.literals, &self.distances);
        let (stop, at) = codes(at, tables, &mut window.buf, limit, input.bytes);
        (self.bits, window.end, input.taken) = (at.bits, at.end, at.taken);
        self.unpaired = self.unpaired.saturating_add(at.end.saturating_sub(begun));
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

    /// Returns how far to decode, up to `limit`, from `end`: no further
    /// than `PAIR_AFTER` into a block whose literals are not paired, which
    /// it pairs once the block has decoded that much.
    fn pairing(&mut self, limit: usize, end: usize) -> usize {
        if self.literals.paired() {
            return limit;
        }
        match PAIR_AFTER.checked_sub(self.unpaired) {
            Some(left @ 1..) => limit.min(end.saturating_add(left)),
            _ => {
                self.literals.pair();
                limit
            },
        }
    }

    /// Takes on the fixed codes (§3.2.6): the literal/length codes 286 and
    /// 287, and the distance codes 30 and 31, take part, but never occur.
    /// Deflate's tables are made at compile time, Deflate64's here.
    fn fixed(&mut self) -> Result<(), Error> {
        if self.window.format == Format::Deflate {
            self.literals = Table::fixed(&FIXED_LITERALS, LITERAL_BITS);
            self.distances = Table::fixed(&FIXED_DISTANCES, DISTANCE_BITS);
            return Ok(());
        }
        let mut literals = [8_u8; 288];
        for (symbol, length) in literals.iter_mut().enumerate() {
            *length = match symbol {
                0..=143 | 280.. => 8,
                144..=255 => 9,
                _ => 7,
            };
        }
        let format = self.window.format;
        self.literals
            .build(&literals, LITERAL_BITS, Kind::Literals, |symbol| {
                format.literal(symbol)
            })?;
        self.distances
            .build(&[5; 32], DISTANCE_BITS, Kind::Distances, |symbol| {
                format.distance(symbol)
            })?;
        Ok(())
    }

    /// Ends the block being decoded.
    fn ended(&mut self) {
        self.state = if self.last { State::End } else { State::Header };
    }
}

/// The tables of Deflate's fixed codes (§3.2.6), made at compile time as
/// `Table::new` makes them: no code is longer than the index, and none of a
/// literal so short that two fit it.
#[allow(
    clippy::as_conversions,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "made at compile time, of the fixed codes alone"
)]
static FIXED_LITERALS: [Entry; LITERAL_SIZE] = {
    let mut entries = [INVALID; LITERAL_SIZE];
    let mut symbol = 0;
    while symbol < 288 {
        // Each length's codes in turn, from the first of its length.
        let (len, code) = match symbol {
            0..=143 => (8, 0x30 + symbol),
            144..=255 => (9, 0x190 + symbol - 144),
            256..=279 => (7, symbol - 256),
            _ => (8, 0xC0 + symbol - 280),
        };
        let entry = match symbol {
            0..=255 => (symbol as u32) << 16 | (LITERAL as u32) << 8 | len,
            256 => END_ENTRY | len,
            257..=285 => {
                let (base, extra) = LENGTHS[symbol - 257];
                (base as u32) << 16 | len << 8 | (len + extra as u32)
            },
            _ => INVALID,
        };
        let mut at = (code as u16).reverse_bits() as usize >> (16 - len);
        while at < LITERAL_SIZE {
            entries[at] = entry;
            at += 1 << len;
        }
        symbol += 1;
    }
    entries
};

#[allow(
    clippy::as_conversions,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "made at compile time, of the fixed codes alone"
)]
static FIXED_DISTANCES: [Entry; DISTANCE_SIZE] = {
    let mut entries = [INVALID; DISTANCE_SIZE];
    let mut symbol = 0;
    while symbol < 32 {
        let entry = match symbol {
            0..30 => {
                let (base, extra) = DISTANCES[symbol];
                (base as u32) << 16 | 5 << 8 | (5 + extra as u32)
            },
            _ => INVALID,
        };
        let mut at = (symbol as u8).reverse_bits() as usize >> 3;
        while at < DISTANCE_SIZE {
            entries[at] = entry;
            at += 32;
        }
        symbol += 1;
    }
    entries
};

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
    buf: &mut [u8],
    limit: usize,
    input: &[u8],
) -> (Result<Stop, Error>, Cursor) {
    // The last place a write may begin, as `fast` finds it.
    let Some(edge) = buf.len().checked_sub(SLACK) else {
        return (Err(corrupt()), at);
    };
    // Fast, once no length waits for its distance, as one may from input
    // given before; and once only, as it stops short only where the input
    // runs short.
    let mut tried = false;
    let stop = loop {
        if !tried && at.length.is_none() {
            tried = true;
            match fast(&mut at, (literals, distances), buf, limit, input) {
                Ok(None) => {},
                Ok(Some(stop)) => break Ok(stop),
                Err(error) => break Err(error),
            }
        }
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
                // A literal, or two.
                if kind & u32::from(LITERAL) != 0 {
                    let entry = value.wrapping_shl(16) | kind.wrapping_shl(8);
                    if let Err(error) = literal(buf, &mut at.end, entry, edge) {
                        break Err(error);
                    }
                    at.bits.consume(n);
                    continue;
                }
                if kind == u32::from(END) {
                    at.bits.consume(n);
                    break Ok(Stop::End);
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
        if at.bits.count < n.saturating_add(extra) {
            at.length = Some(len);
            break Ok(Stop::More);
        }
        at.bits.consume(n);
        let distance = base.wrapping_add(at.bits.take(extra));
        let (Ok(distance), Ok(len)) = (usize::try_from(distance), usize::try_from(len)) else {
            break Err(corrupt());
        };
        match copy(buf, at.end, distance, len, edge) {
            Ok(end) => at.end = end,
            Err(error) => break Err(error),
        }
    };
    (stop, at)
}

/// Decodes codes as `codes` does, while the input holds a word past what
/// is taken, so that every refill leaves 56 bits at least, and 64 that are
/// the stream's: enough for four literal codes of the table's index or
/// shorter, then the index of the next code; or for a code of any length,
/// a length and its extra bits, then a distance code and its own, 49 bits
/// at most, then the next index. So no code can find too few, and each
/// turn takes literals or a match, after a refill. Its tables hold each
/// symbol's meaning, so that a length or distance takes no lookup of its
/// own, and how many bits it takes, extra bits too, so that one shift
/// reads past them; a literal's entry says so in one bit, and one test
/// tells every other entry that is not a length's or a distance's; and its
/// refills take no branch, leaving above the bits they need the input's
/// next. It looks up the next code before it copies a match, so that the
/// two overlap. It returns why it stopped, or nothing once the input runs
/// short, for `codes` to go on carefully. Out of line, so that its loop
/// has registers enough for what it carries. With `intrinsics`, it is
/// built for BMI2 too, whose shifts take any register, and taken where the
/// CPU has it.
#[cfg_attr(
    all(
        feature = "intrinsics",
        target_arch = "x86_64",
        target_feature = "sse2"
    ),
    multiversion::multiversion(targets("x86_64+bmi1+bmi2+lzcnt+popcnt"))
)]
#[inline(never)]
fn fast(
    at: &mut Cursor,
    (literals, distances): (&Table, &Table),
    buf: &mut [u8],
    limit: usize,
    input: &[u8],
) -> Result<Option<Stop>, Error> {
    let (Some(lit), Some(dist)) = (
        literals.primary::<LITERAL_SIZE>(),
        distances.primary::<DISTANCE_SIZE>(),
    ) else {
        return Ok(None);
    };
    // In locals, which the loop keeps in registers.
    let (mut bits, mut end, mut taken) = (at.bits, at.end, at.taken);
    // The last place a word may be read from, which bounds `taken` once a
    // turn, so that reading it takes no other check.
    let Some(last) = input.len().checked_sub(8).filter(|&last| taken <= last) else {
        return Ok(None);
    };
    // The last place a write may begin, the most it writes past it being
    // `SLACK`, which bounds each write once, rather than its end too.
    let Some(edge) = buf.len().checked_sub(SLACK) else {
        return Ok(None);
    };
    // The next code's entry, looked up ahead, once the bits are refilled.
    bits.refill_word(word(input, taken), &mut taken);
    let mut entry = huffman::at(lit, bits.value);
    let stop = loop {
        if end >= limit {
            break Ok(Some(Stop::Full));
        }
        if taken > last {
            break Ok(None);
        }
        bits.refill_word(word(input, taken), &mut taken);
        if entry & LITERAL_ENTRY != 0 {
            literal(buf, &mut end, entry, edge)?;
            bits.skip(entry);
            entry = huffman::at(lit, bits.value);
            if entry & LITERAL_ENTRY != 0 {
                literal(buf, &mut end, entry, edge)?;
                bits.skip(entry);
                entry = huffman::at(lit, bits.value);
                if entry & LITERAL_ENTRY != 0 {
                    literal(buf, &mut end, entry, edge)?;
                    bits.skip(entry);
                    entry = huffman::at(lit, bits.value);
                    if entry & LITERAL_ENTRY != 0 {
                        literal(buf, &mut end, entry, edge)?;
                        bits.skip(entry);
                        entry = huffman::at(lit, bits.value);
                    }
                }
            }
            continue;
        }
        // A code longer than the index, which may be a literal's too, the
        // block's end, or none.
        if entry & SPECIAL != 0 {
            if entry & LINK != 0 {
                entry = literals.sub(entry, bits.value);
            }
            if entry & LITERAL_ENTRY != 0 {
                literal(buf, &mut end, entry, edge)?;
                bits.skip(entry);
                entry = huffman::at(lit, bits.value);
                continue;
            }
            if ends(entry) {
                bits.consume(end_length(entry));
                break Ok(Some(Stop::End));
            }
            if entry & SPECIAL != 0 {
                break Err(corrupt());
            }
        }
        let len = huffman::meaning(entry, bits.value);
        bits.skip(entry);
        let mut entry_of_distance = huffman::at(dist, bits.value);
        if entry_of_distance & SPECIAL != 0 {
            if entry_of_distance & LINK != 0 {
                entry_of_distance = distances.sub(entry_of_distance, bits.value);
            }
            if entry_of_distance & SPECIAL != 0 {
                break Err(corrupt());
            }
        }
        let distance = huffman::meaning(entry_of_distance, bits.value);
        bits.skip(entry_of_distance);
        entry = huffman::at(lit, bits.value);
        let distance = usize::try_from(distance).map_err(|_| corrupt())?;
        let len = usize::try_from(len).map_err(|_| corrupt())?;
        end = copy(buf, end, distance, len, edge)?;
    };
    // What `skip` left above the count's byte.
    bits.count &= 0xFF;
    (at.bits, at.end, at.taken) = (bits, end, taken);
    stop
}

/// Returns the word of `input` at `at`, which holds one there.
#[inline(always)]
fn word(input: &[u8], at: usize) -> [u8; 8] {
    let word = input.get(at..).and_then(<[u8]>::first_chunk::<8>);
    word.copied().unwrap_or_default()
}

/// Writes the literal of `entry`, or its two, at `end`, and moves `end`
/// past them. It writes two bytes either way, the second, of one, past
/// `end`, to be written over.
#[inline(always)]
fn literal(buf: &mut [u8], end: &mut usize, entry: Entry, last: usize) -> Result<(), Error> {
    let [_, _, first, second] = entry.to_le_bytes();
    *chunk_mut::<2>(buf, *end, last).ok_or_else(corrupt)? = [first, second];
    let count = usize::from(entry & DOUBLE_ENTRY != 0);
    *end = end.wrapping_add(count).wrapping_add(1);
    Ok(())
}

/// Copies to `buf` past `end` `length` bytes from `distance` back, which
/// they may overlap, repeating what they copy, and returns the new end. It
/// may write up to `SLACK` bytes past that end. A distance is never past
/// the window, as no distance code reaches further, those of Deflate64
/// never occurring in Deflate.
#[inline(always)]
fn copy(
    buf: &mut [u8],
    end: usize,
    distance: usize,
    length: usize,
    edge: usize,
) -> Result<usize, Error> {
    if distance > end {
        return Err(corrupt());
    }
    let mut from = end.wrapping_sub(distance);
    // No match reaches past the buffer's length, which bounds `end`.
    let stop = end.wrapping_add(length);
    let mut to = end;
    match distance {
        // On x86-64, by 32 at once where they may, so that a match of up to
        // 32 bytes takes one turn: 1 to 2% faster there, and 2% slower on
        // Neoverse V3, which keeps to 16.
        #[cfg(target_arch = "x86_64")]
        32.. => chunks::<32>(buf, from, to, stop, edge)?,
        // A long match from fewer than 32 bytes back, whose chunks would
        // each read what the one before wrote: by a pattern of its period.
        16..32 if length > 32 => repeat(buf, from, distance, to, stop, edge)?,
        16.. => chunks::<16>(buf, from, to, stop, edge)?,
        // A run of one byte: short ones in a chunk or two, without the call
        // that filling takes, and long ones by it.
        1 => {
            let byte = buf.get(from).copied().ok_or_else(corrupt)?;
            match length {
                ..=32 => fill(buf, [byte; 16], to, stop, edge)?,
                _ => buf.get_mut(to..stop).ok_or_else(corrupt)?.fill(byte),
            }
        },
        8.. => chunks::<8>(buf, from, to, stop, edge)?,
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
            chunks::<8>(buf, to.saturating_sub(period), to, stop, edge)?;
        },
    }
    Ok(stop)
}

/// Copies to `buf` from `from` to `to`, until `stop`, in chunks of `N`
/// bytes, up to `N` past it, one at least: chunks that do not overlap what
/// they copy, as `to` is at least `N` past `from`, each after the last has
/// written what it reads.
#[inline(always)]
fn chunks<const N: usize>(
    buf: &mut [u8],
    mut from: usize,
    mut to: usize,
    stop: usize,
    edge: usize,
) -> Result<(), Error> {
    loop {
        let chunk = *self::chunk::<N>(buf, from, edge).ok_or_else(corrupt)?;
        *chunk_mut::<N>(buf, to, edge).ok_or_else(corrupt)? = chunk;
        (from, to) = (from.wrapping_add(N), to.wrapping_add(N));
        if to >= stop {
            return Ok(());
        }
    }
}

/// Copies to `buf` from `to` until `stop` what repeats from `from` with a
/// period of `distance`, 16 to 31: 32 bytes of it laid out once, from two
/// chunks before `to` and the first again a period on, then written over
/// and over a period apart, so that no write waits on reading the one
/// before; up to 32 bytes past `stop`, once at least.
fn repeat(
    buf: &mut [u8],
    from: usize,
    distance: usize,
    mut to: usize,
    stop: usize,
    edge: usize,
) -> Result<(), Error> {
    let head = *chunk::<16>(buf, from, edge).ok_or_else(corrupt)?;
    let tail = *chunk::<16>(buf, from.wrapping_add(16), edge).ok_or_else(corrupt)?;
    let mut pattern = [0_u8; 48];
    let (low, high) = pattern.split_at_mut(16);
    low.copy_from_slice(&head);
    *high.first_chunk_mut::<16>().ok_or_else(corrupt)? = tail;
    let again = pattern
        .get_mut(distance & 31..)
        .and_then(<[u8]>::first_chunk_mut::<16>);
    *again.ok_or_else(corrupt)? = head;
    let pattern = *pattern.first_chunk::<32>().ok_or_else(corrupt)?;
    loop {
        *chunk_mut::<32>(buf, to, edge).ok_or_else(corrupt)? = pattern;
        to = to.wrapping_add(distance);
        if to >= stop {
            return Ok(());
        }
    }
}

/// Writes `chunk` to `buf` from `to` until `stop`, over and over, up to
/// its length past it, once at least.
#[inline(always)]
fn fill<const N: usize>(
    buf: &mut [u8],
    chunk: [u8; N],
    mut to: usize,
    stop: usize,
    edge: usize,
) -> Result<(), Error> {
    loop {
        *chunk_mut::<N>(buf, to, edge).ok_or_else(corrupt)? = chunk;
        to = to.wrapping_add(N);
        if to >= stop {
            return Ok(());
        }
    }
}

/// Returns the `N` bytes of `buf` at `at`, `SLACK` at most, if `at` is no
/// further than `edge`, the last place a write may begin, which `buf`
/// holds `SLACK` bytes past: one comparison, so that taking them takes no
/// other check.
#[inline(always)]
fn chunk<const N: usize>(buf: &[u8], at: usize, edge: usize) -> Option<&[u8; N]> {
    match at <= edge {
        true => buf.get(at..)?.first_chunk(),
        false => None,
    }
}

/// Returns the `N` bytes of `buf` at `at`, as [`chunk`] does, to write.
#[inline(always)]
fn chunk_mut<const N: usize>(buf: &mut [u8], at: usize, edge: usize) -> Option<&mut [u8; N]> {
    match at <= edge {
        true => buf.get_mut(at..)?.first_chunk_mut(),
        false => None,
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
            .field("last", &self.last)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deflate's fixed codes' tables, made at compile time, are those that
    /// `Table::new` makes of the fixed lengths.
    #[test]
    fn makes_the_fixed_tables() {
        let lengths: Vec<u8> = (0..288)
            .map(|symbol| match symbol {
                0..=143 | 280.. => 8,
                144..=255 => 9,
                _ => 7,
            })
            .collect();
        let format = Format::Deflate;
        let literals = Table::new(&lengths, LITERAL_BITS, Kind::Literals, |s| {
            format.literal(s)
        })
        .unwrap();
        assert_eq!(literals.primary::<LITERAL_SIZE>(), Some(&FIXED_LITERALS));
        let distances = Table::new(&[5; 32], DISTANCE_BITS, Kind::Distances, |s| {
            format.distance(s)
        })
        .unwrap();
        assert_eq!(distances.primary::<DISTANCE_SIZE>(), Some(&FIXED_DISTANCES));
    }

    /// The most a run decodes past the window, once compacted: a chunk
    /// and the longest match, less one; and then room for as much again.
    #[test]
    fn bounds_the_buffer() {
        let formats = [
            Format::Deflate,
            #[cfg(feature = "deflate64")]
            Format::Deflate64,
        ];
        for format in formats {
            let held = CHUNK + format.longest() - 1;
            let want = format.window() + held + CHUNK + format.longest() + SLACK;
            assert!(want <= format.buffer(), "{format:?}");
        }
    }
}

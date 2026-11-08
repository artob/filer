// This is free and unencumbered software released into the public domain.

//! A compressed block's sequences (RFC 8878 §3.1.1.3.2): literal lengths,
//! offsets, and match lengths, each FSE-coded, interleaved in one stream;
//! and their execution (§3.1.1.4), which copies literals and then earlier
//! bytes, from as far back as the window reaches.

use super::{
    bits::Backward,
    corrupt,
    fse::{MAX_STATES, Table},
    window::Cursor,
};
use crate::Error;

/// Each literal length code's baseline and extra bits (Table 16).
const LITERAL_LENGTHS: [(u32, u8); 36] = [
    (0, 0),
    (1, 0),
    (2, 0),
    (3, 0),
    (4, 0),
    (5, 0),
    (6, 0),
    (7, 0),
    (8, 0),
    (9, 0),
    (10, 0),
    (11, 0),
    (12, 0),
    (13, 0),
    (14, 0),
    (15, 0),
    (16, 1),
    (18, 1),
    (20, 1),
    (22, 1),
    (24, 2),
    (28, 2),
    (32, 3),
    (40, 3),
    (48, 4),
    (64, 6),
    (128, 7),
    (256, 8),
    (512, 9),
    (1024, 10),
    (2048, 11),
    (4096, 12),
    (8192, 13),
    (16384, 14),
    (32768, 15),
    (65536, 16),
];

/// Each match length code's baseline and extra bits (Table 17).
const MATCH_LENGTHS: [(u32, u8); 53] = [
    (3, 0),
    (4, 0),
    (5, 0),
    (6, 0),
    (7, 0),
    (8, 0),
    (9, 0),
    (10, 0),
    (11, 0),
    (12, 0),
    (13, 0),
    (14, 0),
    (15, 0),
    (16, 0),
    (17, 0),
    (18, 0),
    (19, 0),
    (20, 0),
    (21, 0),
    (22, 0),
    (23, 0),
    (24, 0),
    (25, 0),
    (26, 0),
    (27, 0),
    (28, 0),
    (29, 0),
    (30, 0),
    (31, 0),
    (32, 0),
    (33, 0),
    (34, 0),
    (35, 1),
    (37, 1),
    (39, 1),
    (41, 1),
    (43, 2),
    (47, 2),
    (51, 3),
    (59, 3),
    (67, 4),
    (83, 4),
    (99, 5),
    (131, 7),
    (259, 8),
    (515, 9),
    (1027, 10),
    (2051, 11),
    (4099, 12),
    (8195, 13),
    (16387, 14),
    (32771, 15),
    (65539, 16),
];

/// The most offset codes: an offset code is its number of extra bits,
/// and those of 31 reach farther than any window decoded.
const OFFSET_CODES: usize = 32;

/// The distributions of predefined mode (RFC 8878 §3.1.1.3.2.2), and
/// their accuracies.
const LITERAL_LENGTH_DEFAULT: [i16; 36] = [
    4, 3, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 3, 2, 1, 1, 1, 1, 1,
    -1, -1, -1, -1,
];
const MATCH_LENGTH_DEFAULT: [i16; 53] = [
    1, 4, 3, 2, 2, 2, 2, 2, 2, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, -1, -1, -1, -1, -1, -1, -1,
];
const OFFSET_DEFAULT: [i16; 29] = [
    1, 1, 1, 1, 1, 1, 2, 2, 2, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, -1, -1, -1, -1, -1,
];

/// One of the three kinds of code a sequence is made of.
struct Kind {
    /// Its place in the compression modes' byte, from the top.
    shift: u8,
    default: &'static [i16],
    default_log: u8,
    max_log: u8,
    symbols: usize,
    /// Each symbol's baseline and extra bits.
    value: fn(u8) -> (u32, u8),
}

const LITERAL_LENGTH: Kind = Kind {
    shift: 6,
    default: &LITERAL_LENGTH_DEFAULT,
    default_log: 6,
    max_log: 9,
    symbols: LITERAL_LENGTHS.len(),
    value: |symbol| lookup(&LITERAL_LENGTHS, symbol),
};
const OFFSET: Kind = Kind {
    shift: 4,
    default: &OFFSET_DEFAULT,
    default_log: 5,
    max_log: 8,
    symbols: OFFSET_CODES,
    // An offset code is its extra bits, which add to its power of two.
    value: |code| (1_u32.wrapping_shl(u32::from(code)), code),
};
const MATCH_LENGTH: Kind = Kind {
    shift: 2,
    default: &MATCH_LENGTH_DEFAULT,
    default_log: 6,
    max_log: 9,
    symbols: MATCH_LENGTHS.len(),
    value: |symbol| lookup(&MATCH_LENGTHS, symbol),
};

/// Returns `symbol`'s baseline and extra bits in `codes`, if it has any.
fn lookup(codes: &[(u32, u8)], symbol: u8) -> (u32, u8) {
    codes.get(usize::from(symbol)).copied().unwrap_or_default()
}

/// A state of a kind of code's decoding table, as sequences read it: the
/// baseline of its symbol's value, and the extra bits that add to it; and
/// how to find the next state, from `bits` more read and added to `next`.
#[derive(Clone, Copy, Debug, Default)]
struct State {
    baseline: u32,
    extra: u8,
    bits: u8,
    next: u16,
}

/// A kind of code's decoding table, each state's symbol resolved to its
/// value, so that a sequence looks up no more than its state: held in an
/// array of the most any table takes, so that it is never allocated.
#[derive(Clone, Debug)]
pub(super) struct Decoding {
    log: u8,
    /// Whether it is the kind's predefined table, which it need not be
    /// built again to be.
    predefined: bool,
    states: [State; MAX_STATES],
}

impl Decoding {
    /// Returns `table`, of `kind`, resolved.
    fn new(table: &Table, kind: &Kind, predefined: bool) -> Self {
        let mut states = [State::default(); MAX_STATES];
        for (state, entry) in states.iter_mut().zip(table.entries()) {
            let (baseline, extra) = (kind.value)(entry.symbol);
            *state = State {
                baseline,
                extra,
                bits: entry.bits,
                next: entry.base,
            };
        }
        Self {
            log: table.log(),
            predefined,
            states,
        }
    }

    /// Returns the state that `bits` begin with.
    fn start(&self, bits: &mut Backward<'_>) -> usize {
        usize::try_from(bits.read(u32::from(self.log))).unwrap_or_default()
    }

    /// Returns `state`'s entry: of the states of the finest table, as
    /// every state is, which spares checking where it lies.
    fn state(&self, state: usize) -> State {
        let state = state & MAX_STATES.wrapping_sub(1);
        self.states.get(state).copied().unwrap_or_default()
    }
}

impl State {
    /// Returns its value: its baseline and its extra bits, from `bits`,
    /// which must hold them without a refill.
    fn value(self, bits: &mut Backward<'_>) -> usize {
        // Up to 31 bits onto a baseline of 32, which no `u64` overflows.
        let more = bits.take(u32::from(self.extra));
        let value = u64::from(self.baseline).wrapping_add(more);
        usize::try_from(value).unwrap_or(usize::MAX)
    }

    /// Returns the next state, from `bits`, which must hold them, up to 9,
    /// without a refill.
    fn next(self, bits: &mut Backward<'_>) -> usize {
        // Up to 9 bits onto a base of 16, which no `usize` overflows.
        let more = usize::try_from(bits.take(u32::from(self.bits))).unwrap_or_default();
        usize::from(self.next).wrapping_add(more)
    }
}

/// What a frame's blocks of sequences pass on, each to the next: the
/// tables each kind of code was last decoded with, and the three offsets
/// last used, the latest first.
#[derive(Clone, Debug)]
pub(super) struct Codes {
    literal_lengths: Option<Decoding>,
    offsets: Option<Decoding>,
    match_lengths: Option<Decoding>,
    repeats: [usize; 3],
}

impl Default for Codes {
    fn default() -> Self {
        Self {
            literal_lengths: None,
            offsets: None,
            match_lengths: None,
            repeats: [1, 4, 8],
        }
    }
}

/// Decodes the sequences `section` and runs them with `literals` onto
/// `output`, within its limit.
pub(super) fn decode(
    section: &[u8],
    codes: &mut Codes,
    literals: &[u8],
    output: &mut Cursor<'_>,
) -> Result<(), Error> {
    let (count, rest) = count(section)?;
    if count == 0 {
        // The section ends with its count, and the block with literals.
        if !rest.is_empty() {
            return Err(corrupt());
        }
        return output.extend(literals);
    }
    let (&modes, mut rest) = rest.split_first().ok_or_else(corrupt)?;
    if modes & 0x03 != 0 {
        return Err(corrupt());
    }
    for (kind, table) in [
        (LITERAL_LENGTH, &mut codes.literal_lengths),
        (OFFSET, &mut codes.offsets),
        (MATCH_LENGTH, &mut codes.match_lengths),
    ] {
        rest = kind.table(modes, rest, table)?;
    }
    let (Some(lengths), Some(offsets), Some(matches)) =
        (&codes.literal_lengths, &codes.offsets, &codes.match_lengths)
    else {
        return Err(corrupt());
    };

    // The states start as literal lengths', offsets', and match lengths',
    // in that order; each sequence reads its offset's bits, then its match
    // length's, then its literal length's; and then, but for the last, the
    // states move on, literal lengths' first, then match lengths', then
    // offsets'.
    let mut bits = Backward::new(rest)?;
    let mut states = [
        lengths.start(&mut bits),
        offsets.start(&mut bits),
        matches.start(&mut bits),
    ];
    let mut literals = literals;
    // The repeat offsets, held apart for the block, in registers, as is
    // the cursor, whose end the block's takes back, unless it fails.
    let mut repeats = codes.repeats;
    let mut cursor = output.apart();
    for left in (0..count).rev() {
        let [length, offset, matched] = states;
        let (length, offset, matched) = (
            lengths.state(length),
            offsets.state(offset),
            matches.state(matched),
        );
        // A refill holds 57 bits: the states' 26, and the values' extra
        // bits, unless they take more than 31, when it takes another
        // between an offset's and a match length's, up to 47, and a literal
        // length's, up to 16.
        bits.refill();
        let extra = offset.extra.wrapping_add(matched.extra);
        let value = offset.value(&mut bits);
        let match_len = matched.value(&mut bits);
        if extra.wrapping_add(length.extra) > 31 {
            bits.refill();
        }
        let literal_len = length.value(&mut bits);
        if left > 0 {
            let length = length.next(&mut bits);
            let matched = matched.next(&mut bits);
            states = [length, offset.next(&mut bits), matched];
        }
        cursor.literals(literals, literal_len)?;
        literals = literals.get(literal_len..).ok_or_else(corrupt)?;
        let offset = resolve(value, literal_len, &mut repeats)?;
        cursor.copy(offset, match_len)?;
    }
    let end = cursor.end;
    output.end = end;
    if !bits.done() {
        return Err(corrupt());
    }
    codes.repeats = repeats;
    // The literals left over end the block.
    output.extend(literals)
}

impl Kind {
    /// Reads the table this kind of code is decoded with from `section`,
    /// as `modes` say, into `table`, and returns the section past it.
    fn table<'a>(
        &self,
        modes: u8,
        section: &'a [u8],
        table: &mut Option<Decoding>,
    ) -> Result<&'a [u8], Error> {
        match (modes >> self.shift) & 0x03 {
            // Predefined, as it may be already.
            0 if table.as_ref().is_some_and(|table| table.predefined) => {},
            0 => {
                let predefined = Table::new(self.default, self.default_log)?;
                *table = Some(Decoding::new(&predefined, self, true));
            },
            // One symbol, every time.
            1 => {
                let (&symbol, rest) = section.split_first().ok_or_else(corrupt)?;
                if usize::from(symbol) >= self.symbols {
                    return Err(corrupt());
                }
                *table = Some(Decoding::new(&Table::rle(symbol), self, false));
                return Ok(rest);
            },
            // Described.
            2 => {
                let (read, used) = Table::read(section, self.max_log, self.symbols)?;
                *table = Some(Decoding::new(&read, self, false));
                return section.get(used..).ok_or_else(corrupt);
            },
            // The last block's, which must have been.
            _ if table.is_none() => return Err(corrupt()),
            _ => {},
        }
        Ok(section)
    }
}

/// Reads the count of sequences that `section` begins with, in one, two,
/// or three bytes, and returns it and the rest.
fn count(section: &[u8]) -> Result<(usize, &[u8]), Error> {
    match *section {
        [first @ 0..=127, ref rest @ ..] => Ok((usize::from(first), rest)),
        [255, low, high, ref rest @ ..] => {
            let count = u16::from_le_bytes([low, high]);
            Ok((usize::from(count).saturating_add(0x7F00), rest))
        },
        [first @ 128..=254, low, ref rest @ ..] => {
            let count = u16::from_be_bytes([first.saturating_sub(128), low]);
            Ok((usize::from(count), rest))
        },
        _ => Err(corrupt()),
    }
}

/// Returns the offset that a sequence's offset `value` means, after its
/// `literal_len` literals, and updates the `repeats` (RFC 8878 §3.1.1.5).
///
/// Values past 3 are offsets, less 3; 1 to 3 repeat the latest offsets,
/// but for a sequence without literals, whose 1 and 2 repeat the second
/// and third latest, and whose 3 is one less than the latest. An offset
/// used goes first among the repeats, and those before it move back one.
fn resolve(value: usize, literal_len: usize, repeats: &mut [usize; 3]) -> Result<usize, Error> {
    let [first, second, third] = *repeats;
    let (offset, after) = match value.checked_sub(3) {
        Some(offset @ 1..) => (offset, [offset, first, second]),
        _ => match value
            .saturating_sub(1)
            .saturating_add(usize::from(literal_len == 0))
        {
            0 => (first, *repeats),
            1 => (second, [second, first, third]),
            2 => (third, [third, first, second]),
            _ => {
                let offset = first.saturating_sub(1);
                (offset, [offset, first, second])
            },
        },
    };
    // One less than an offset of 1 is none.
    if offset == 0 {
        return Err(corrupt());
    }
    *repeats = after;
    Ok(offset)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{fse::Entry, window::Window};

    const LITERAL_LENGTH_STATES: [(u8, u8, u16); 64] = [
        (0, 4, 0),
        (0, 4, 16),
        (1, 5, 32),
        (3, 5, 0),
        (4, 5, 0),
        (6, 5, 0),
        (7, 5, 0),
        (9, 5, 0),
        (10, 5, 0),
        (12, 5, 0),
        (14, 6, 0),
        (16, 5, 0),
        (18, 5, 0),
        (19, 5, 0),
        (21, 5, 0),
        (22, 5, 0),
        (24, 5, 0),
        (25, 5, 32),
        (26, 5, 0),
        (27, 6, 0),
        (29, 6, 0),
        (31, 6, 0),
        (0, 4, 32),
        (1, 4, 0),
        (2, 5, 0),
        (4, 5, 32),
        (5, 5, 0),
        (7, 5, 32),
        (8, 5, 0),
        (10, 5, 32),
        (11, 5, 0),
        (13, 6, 0),
        (16, 5, 32),
        (17, 5, 0),
        (19, 5, 32),
        (20, 5, 0),
        (22, 5, 32),
        (23, 5, 0),
        (25, 4, 0),
        (25, 4, 16),
        (26, 5, 32),
        (28, 6, 0),
        (30, 6, 0),
        (0, 4, 48),
        (1, 4, 16),
        (2, 5, 32),
        (3, 5, 32),
        (5, 5, 32),
        (6, 5, 32),
        (8, 5, 32),
        (9, 5, 32),
        (11, 5, 32),
        (12, 5, 32),
        (15, 6, 0),
        (17, 5, 32),
        (18, 5, 32),
        (20, 5, 32),
        (21, 5, 32),
        (23, 5, 32),
        (24, 5, 32),
        (35, 6, 0),
        (34, 6, 0),
        (33, 6, 0),
        (32, 6, 0),
    ];
    const MATCH_LENGTH_STATES: [(u8, u8, u16); 64] = [
        (0, 6, 0),
        (1, 4, 0),
        (2, 5, 32),
        (3, 5, 0),
        (5, 5, 0),
        (6, 5, 0),
        (8, 5, 0),
        (10, 6, 0),
        (13, 6, 0),
        (16, 6, 0),
        (19, 6, 0),
        (22, 6, 0),
        (25, 6, 0),
        (28, 6, 0),
        (31, 6, 0),
        (33, 6, 0),
        (35, 6, 0),
        (37, 6, 0),
        (39, 6, 0),
        (41, 6, 0),
        (43, 6, 0),
        (45, 6, 0),
        (1, 4, 16),
        (2, 4, 0),
        (3, 5, 32),
        (4, 5, 0),
        (6, 5, 32),
        (7, 5, 0),
        (9, 6, 0),
        (12, 6, 0),
        (15, 6, 0),
        (18, 6, 0),
        (21, 6, 0),
        (24, 6, 0),
        (27, 6, 0),
        (30, 6, 0),
        (32, 6, 0),
        (34, 6, 0),
        (36, 6, 0),
        (38, 6, 0),
        (40, 6, 0),
        (42, 6, 0),
        (44, 6, 0),
        (1, 4, 32),
        (1, 4, 48),
        (2, 4, 16),
        (4, 5, 32),
        (5, 5, 32),
        (7, 5, 32),
        (8, 5, 32),
        (11, 6, 0),
        (14, 6, 0),
        (17, 6, 0),
        (20, 6, 0),
        (23, 6, 0),
        (26, 6, 0),
        (29, 6, 0),
        (52, 6, 0),
        (51, 6, 0),
        (50, 6, 0),
        (49, 6, 0),
        (48, 6, 0),
        (47, 6, 0),
        (46, 6, 0),
    ];
    const OFFSET_STATES: [(u8, u8, u16); 32] = [
        (0, 5, 0),
        (6, 4, 0),
        (9, 5, 0),
        (15, 5, 0),
        (21, 5, 0),
        (3, 5, 0),
        (7, 4, 0),
        (12, 5, 0),
        (18, 5, 0),
        (23, 5, 0),
        (5, 5, 0),
        (8, 4, 0),
        (14, 5, 0),
        (20, 5, 0),
        (2, 5, 0),
        (7, 4, 16),
        (11, 5, 0),
        (17, 5, 0),
        (22, 5, 0),
        (4, 5, 0),
        (8, 4, 16),
        (13, 5, 0),
        (19, 5, 0),
        (1, 5, 0),
        (6, 4, 16),
        (10, 5, 0),
        (16, 5, 0),
        (28, 5, 0),
        (27, 5, 0),
        (26, 5, 0),
        (25, 5, 0),
        (24, 5, 0),
    ];

    /// RFC 8878 Appendix A: the decoding tables of the predefined
    /// distributions, each state's symbol, bits, and base.
    #[test]
    fn builds_the_predefined_tables_as_the_rfc_does() {
        for (kind, states) in [
            (LITERAL_LENGTH, &LITERAL_LENGTH_STATES[..]),
            (MATCH_LENGTH, &MATCH_LENGTH_STATES[..]),
            (OFFSET, &OFFSET_STATES[..]),
        ] {
            let table = Table::new(kind.default, kind.default_log).unwrap();
            for (state, &(symbol, bits, base)) in states.iter().enumerate() {
                let expected = Entry { symbol, bits, base };
                assert_eq!(table.entry(state), expected, "{} {state}", kind.shift);
            }
        }
    }

    /// RFC 8878 Table 18, as its prose has it: its last two rows have 3,
    /// not 1, in the first, and 1111, not 3333, in the last.
    #[test]
    fn repeats_offsets_as_the_rfc_does() {
        let mut repeats = [1, 4, 8];
        for (value, literal_len, offset, after) in [
            (1114, 11, 1111, [1111, 1, 4]),
            (1, 22, 1111, [1111, 1, 4]),
            (2225, 22, 2222, [2222, 1111, 1]),
            (1114, 111, 1111, [1111, 2222, 1111]),
            (3336, 33, 3333, [3333, 1111, 2222]),
            (2, 22, 1111, [1111, 3333, 2222]),
            (3, 33, 2222, [2222, 1111, 3333]),
            (3, 0, 2221, [2221, 2222, 1111]),
            (1, 0, 2222, [2222, 2221, 1111]),
        ] {
            assert_eq!(resolve(value, literal_len, &mut repeats).unwrap(), offset);
            assert_eq!(repeats, after, "{value} after {literal_len}");
        }
        // One less than an offset of 1 is none.
        let mut repeats = [1, 4, 8];
        assert!(resolve(3, 0, &mut repeats).is_err());
        assert_eq!(repeats, [1, 4, 8]);
    }

    #[test]
    fn counts_sequences_in_one_to_three_bytes() {
        assert_eq!(count(&[0]).unwrap().0, 0);
        assert_eq!(count(&[127, 9]).unwrap(), (127, &[9][..]));
        assert_eq!(count(&[128, 200]).unwrap().0, 200);
        assert_eq!(count(&[254, 255]).unwrap().0, 0x7EFF);
        assert_eq!(count(&[255, 0, 0]).unwrap().0, 0x7F00);
        assert_eq!(count(&[255, 0xFF, 0xFF]).unwrap().0, 0x7F00 + 0xFFFF);
        assert!(count(&[]).is_err() && count(&[128]).is_err() && count(&[255, 0]).is_err());
    }

    #[test]
    fn copies_overlapping_what_it_repeats() {
        let mut window = Window::new(1 << 10, 32, None);
        window.prepare().unwrap();
        window.extend(b"abc").unwrap();
        let mut output = window.cursor(10);
        output.copy(2, 7).unwrap();
        assert!(output.copy(11, 1).is_err());
        output.copy(1, 3).unwrap();
        assert!(output.copy(1, 1).is_err());
        let end = output.end;
        window.advance(end);
        assert_eq!(window.since(0), b"abcbcbcbcbbbb");
        // Farther back than the window reaches.
        let mut short = Window::new(2, 2, None);
        short.prepare().unwrap();
        short.extend(b"ab").unwrap();
        short.prepare().unwrap();
        short.extend(b"cd").unwrap();
        short.prepare().unwrap();
        let mut output = short.cursor(2);
        assert!(output.copy(3, 1).is_err());
        output.copy(2, 1).unwrap();
    }
}

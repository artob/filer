// This is free and unencumbered software released into the public domain.

//! A compressed block's sequences (RFC 8878 §3.1.1.3.2): literal lengths,
//! offsets, and match lengths, each FSE-coded, interleaved in one stream;
//! and their execution (§3.1.1.4), which copies literals and then earlier
//! bytes, from as far back as the window reaches.

use super::{
    bits::{Backward, Counted},
    corrupt,
    fse::{Distribution, MAX_STATES, MAX_SYMBOLS, Numbers},
    window::Cursor,
};
use crate::Error;
use alloc::{boxed::Box, vec::Vec};
use core::hint::select_unpredictable;

/// How many sequences are decoded before they run: decoding waits on
/// each state's bits, and running on each copy's bytes, so that apart,
/// each a loop of few values, the copies of several are under way at
/// once, however far back they reach.
const BATCH: usize = 64;

/// A sequence decoded, and yet to run: its literals' length, its match's,
/// and its offset, as its code has it until [`settle`] makes it how far
/// back the match lies.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Sequence {
    pub(super) literals: usize,
    pub(super) matched: usize,
    pub(super) offset: usize,
}

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
    /// The table of its predefined distribution.
    preset: &'static Preset,
    max_log: u8,
    /// Each symbol's baseline and extra bits.
    values: &'static [(u32, u8)],
}

/// Each offset code's baseline and extra bits: a code is its extra bits,
/// which add to its power of two.
#[allow(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::indexing_slicing,
    reason = "evaluated when the crate is built, where a panic fails the build"
)]
const OFFSETS: [(u32, u8); OFFSET_CODES] = {
    let mut offsets = [(0, 0); OFFSET_CODES];
    let mut code = 0_u8;
    while (code as usize) < OFFSET_CODES {
        offsets[code as usize] = (1 << code, code);
        code += 1;
    }
    offsets
};

/// How many bits a state of a kind of code's decoding table has a
/// sequence read: its symbol's `extra` bits; and what those leave of a
/// container's 64 bits, and what they and the state's own bits do, the
/// `short` count and the `long`. Counts so held add up, two or three of
/// them, to how far from a container's bottom a field ends, as far as a
/// shift takes it, in its count's low six bits: each field is shifted
/// down by a sum of them, and none by another field's bits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Counts {
    extra: u8,
    short: u8,
    long: u8,
}

/// What a count of no bits leaves of a container, and three of them.
const NO_BITS: u32 = 64;
const NO_COUNTS: u32 = 3 * NO_BITS;

impl Counts {
    /// Returns the counts of a state that reads `bits`, of a symbol of
    /// `extra` bits: up to 31 and 9.
    const fn new(extra: u8, bits: u8) -> Self {
        let short = 64_u8.wrapping_sub(extra);
        Self {
            extra,
            short,
            long: short.wrapping_sub(bits),
        }
    }
}

const LITERAL_LENGTH: Kind = Kind {
    shift: 6,
    preset: &Preset::new(&LITERAL_LENGTH_DEFAULT, 6, &LITERAL_LENGTHS),
    max_log: 9,
    values: &LITERAL_LENGTHS,
};
const OFFSET: Kind = Kind {
    shift: 4,
    preset: &Preset::new(&OFFSET_DEFAULT, 5, &OFFSETS),
    max_log: 8,
    values: &OFFSETS,
};
const MATCH_LENGTH: Kind = Kind {
    shift: 2,
    preset: &Preset::new(&MATCH_LENGTH_DEFAULT, 6, &MATCH_LENGTHS),
    max_log: 9,
    values: &MATCH_LENGTHS,
};

/// The most states of a predefined distribution's table.
const PRESET_STATES: usize = 64;

/// A predefined distribution's decoding table, built when the crate is,
/// so that a block that takes it copies it: small blocks do, whose
/// tables would take long to build beside decoding them.
struct Preset {
    log: u8,
    steps: [Step; PRESET_STATES],
    counts: [Counts; PRESET_STATES],
    worths: [Worth; PRESET_STATES],
}

impl Preset {
    /// Returns the table of the distribution `counts`, of an accuracy of
    /// `log`, each symbol worth what `values` has: as RFC 8878 §4.1.1
    /// defines it, a symbol's states numbered from its count, in their
    /// order, which [`Decoding::build`] is tested to make the same.
    #[allow(
        clippy::arithmetic_side_effects,
        clippy::as_conversions,
        clippy::indexing_slicing,
        reason = "evaluated when the crate is built, where a panic fails the build"
    )]
    const fn new(counts: &[i16], log: u8, values: &[(u32, u8)]) -> Self {
        let size = 1_usize << log;
        // Symbols of less than 1 take the last states, from the end, and
        // the others are spread over the rest.
        let mut symbols = [0_u8; PRESET_STATES];
        let (mut high, mut symbol) = (size, 0);
        while symbol < counts.len() {
            if counts[symbol] == -1 {
                high -= 1;
                symbols[high] = symbol as u8;
            }
            symbol += 1;
        }
        let step = (size >> 1) + (size >> 3) + 3;
        let (mut at, mut symbol) = (0, 0);
        while symbol < counts.len() {
            let mut state = 0;
            while state < counts[symbol] {
                symbols[at] = symbol as u8;
                at = (at + step) & (size - 1);
                while at >= high {
                    at = (at + step) & (size - 1);
                }
                state += 1;
            }
            symbol += 1;
        }
        assert!(at == 0, "the counts take every state");
        let mut numbers = [0_u32; MAX_SYMBOLS];
        let mut symbol = 0;
        while symbol < counts.len() {
            numbers[symbol] = counts[symbol].unsigned_abs() as u32;
            symbol += 1;
        }
        let mut preset = Self {
            log,
            steps: [Step { next: 0, mask: 0 }; PRESET_STATES],
            counts: [Counts::new(0, 0); PRESET_STATES],
            worths: [Worth {
                baseline: 0,
                mask: 0,
            }; PRESET_STATES],
        };
        let mut state = 0;
        while state < size {
            let symbol = symbols[state] as usize;
            let number = numbers[symbol];
            numbers[symbol] += 1;
            let bits = log as u32 - number.ilog2();
            let (baseline, extra) = values[symbol];
            preset.steps[state] = Step {
                next: (number << bits) - size as u32,
                mask: mask(bits as u8),
            };
            preset.counts[state] = Counts::new(extra, bits as u8);
            preset.worths[state] = Worth {
                baseline,
                mask: mask(extra),
            };
            state += 1;
        }
        preset
    }
}

/// Returns `symbol`'s baseline and extra bits in `codes`, if it has any.
fn lookup(codes: &[(u32, u8)], symbol: u8) -> (u32, u8) {
    codes.get(usize::from(symbol)).copied().unwrap_or_default()
}

/// Returns the low `n` bits set: up to 31, which a `u32` holds.
#[allow(
    clippy::as_conversions,
    reason = "a widening conversion, which the build evaluates too, where `From` does not serve"
)]
const fn mask(n: u8) -> u32 {
    1_u32.wrapping_shl(n as u32).wrapping_sub(1)
}

/// How a state of a kind of code's decoding table moves on, as sequences
/// read it: by the bits of `mask` more read, its low bits set, and added
/// to `next`; so that the next state waits on no load but this one's,
/// each a word that the bits are masked by and added to where it lies.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Step {
    next: u32,
    mask: u32,
}

impl Step {
    /// Returns how many bits the next state reads.
    const fn bits(self) -> u32 {
        width(self.mask)
    }

    /// Returns the next state, from `bits`, which must hold them, up to 9,
    /// without a refill.
    fn next(self, bits: &mut Backward<'_>) -> usize {
        // Up to 9 bits onto a base of 16, which no `u32` overflows.
        let more = u32::try_from(bits.take(self.bits())).unwrap_or_default();
        usize::try_from(self.next.wrapping_add(more)).unwrap_or(usize::MAX)
    }
}

/// What a state's symbol is worth, held apart from how the state moves
/// on, which alone the next state waits for: its baseline, and the low
/// bits set that its extra bits take, which add to it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Worth {
    baseline: u32,
    mask: u32,
}

impl Worth {
    /// Returns how many extra bits the value reads.
    const fn extra(self) -> u32 {
        width(self.mask)
    }

    /// Returns the value: the baseline and its extra bits, from `bits`,
    /// which must hold them without a refill.
    fn value(self, bits: &mut Backward<'_>) -> usize {
        // Up to 31 bits onto a baseline of 32, which no `u64` overflows.
        let more = bits.take(self.extra());
        let value = u64::from(self.baseline).wrapping_add(more);
        usize::try_from(value).unwrap_or(usize::MAX)
    }
}

/// Returns how many low bits `mask` has set, and no others: up to 31.
const fn width(mask: u32) -> u32 {
    mask.wrapping_add(1).trailing_zeros()
}

/// A kind of code's decoding table, each state's symbol resolved to its
/// value, so that a sequence looks up no more than its state: of the most
/// states any table takes, so that it is never allocated. How a state
/// moves on, how many bits it reads, and what it is worth are held apart,
/// each loaded by what waits on it; and each of a state's [`Counts`] in
/// an array of bytes, which a state indexes as it is: the next states
/// wait on them, and a load that scales its index takes a cycle more.
#[derive(Clone, Debug)]
struct Decoding {
    steps: [Step; MAX_STATES],
    extras: [u8; MAX_STATES],
    shorts: [u8; MAX_STATES],
    longs: [u8; MAX_STATES],
    worths: [Worth; MAX_STATES],
}

impl Decoding {
    const NONE: Self = Self {
        steps: [Step { next: 0, mask: 0 }; MAX_STATES],
        extras: [0; MAX_STATES],
        shorts: [0; MAX_STATES],
        longs: [0; MAX_STATES],
        worths: [Worth {
            baseline: 0,
            mask: 0,
        }; MAX_STATES],
    };

    /// Sets it to the table of `distribution`, of `kind`: each state as
    /// its symbol's number has it, and worth what its symbol is, which is
    /// looked up once a symbol, not once a state.
    fn build(&mut self, distribution: &Distribution, kind: &Kind) -> Result<(), Error> {
        let mut symbols = [0; MAX_STATES];
        let states = distribution.spread(&mut symbols)?;
        let mut numbers = distribution.numbers();
        // What each symbol is worth, and its counts but for a state's
        // bits: of as many symbols as numbers are, so that none is
        // checked.
        let mut values = [(Worth::default(), Counts::default()); Numbers::ROOM];
        for (symbol, value) in (0_u8..).zip(&mut values).take(distribution.symbols()) {
            let (worth, extra) = kind.worth(symbol);
            *value = (worth, Counts::new(extra, 0));
        }
        // A state's number, under a bit that marks its top, shifted up
        // by its bits, is its base, past the table's size, under the bit
        // past its mask: both follow from one shift, and are stored as
        // one word.
        let log = distribution.log();
        let top = 1_u64 << u32::BITS;
        let whole = top | 1_u64.wrapping_shl(u32::from(log));
        let slots = self.steps.iter_mut().zip(&mut self.worths);
        let counts = self.extras.iter_mut().zip(&mut self.shorts);
        let slots = slots.zip(counts.zip(&mut self.longs));
        for (((step, worth), ((extra, short), long)), &symbol) in slots.zip(states) {
            let number = numbers.take(symbol);
            let bits = Numbers::bits(number, log);
            let both = (top | u64::from(number)).wrapping_shl(u32::from(bits));
            let both = both.wrapping_sub(whole);
            let at = Numbers::index(symbol);
            let (value, counts) = values.get(at).copied().unwrap_or_default();
            *step = Step {
                next: u32::try_from(both & u64::from(u32::MAX)).unwrap_or_default(),
                mask: u32::try_from(both >> u32::BITS).unwrap_or_default(),
            };
            (*worth, *extra, *short) = (value, counts.extra, counts.short);
            *long = counts.short.wrapping_sub(bits);
        }
        Ok(())
    }

    /// Sets what `state` looks up.
    #[inline(always)]
    fn set(&mut self, state: usize, step: Step, counts: Counts, worth: Worth) {
        if let (Some(to), Some(extra), Some(short), Some(long), Some(value)) = (
            self.steps.get_mut(state),
            self.extras.get_mut(state),
            self.shorts.get_mut(state),
            self.longs.get_mut(state),
            self.worths.get_mut(state),
        ) {
            (*to, *value) = (step, worth);
            (*extra, *short, *long) = (counts.extra, counts.short, counts.long);
        }
    }

    /// Returns the counts of `state`, if it is one.
    #[cfg(test)]
    fn counts(&self, state: usize) -> Option<Counts> {
        Some(Counts {
            extra: *self.extras.get(state)?,
            short: *self.shorts.get(state)?,
            long: *self.longs.get(state)?,
        })
    }

    /// Sets it to `preset`, a table built already.
    fn preset(&mut self, preset: &Preset) {
        let states = 1_usize.wrapping_shl(u32::from(preset.log));
        let entries = preset.steps.iter().zip(&preset.counts);
        let entries = entries.zip(&preset.worths).take(states);
        for (state, ((&step, &counts), &worth)) in entries.enumerate() {
            self.set(state, step, counts, worth);
        }
    }

    /// Sets it to the table of one `symbol`, of `kind`, which its one
    /// state decodes every time, and reads no bits to stay.
    fn single(&mut self, symbol: u8, kind: &Kind) {
        let (value, extra) = kind.worth(symbol);
        self.set(0, Step { next: 0, mask: 0 }, Counts::new(extra, 0), value);
    }

    /// Returns `state`'s entries: of the states of the finest table, as
    /// every state is, which spares checking where it lies.
    fn state(&self, state: usize) -> (Step, Worth) {
        let state = state & MAX_STATES.wrapping_sub(1);
        (
            self.steps.get(state).copied().unwrap_or_default(),
            self.worths.get(state).copied().unwrap_or_default(),
        )
    }
}

/// What is known of a kind of code's table but its states: its accuracy,
/// as a log; whether a block has set it; and whether it is the kind's
/// predefined table, which it need not be built again to be.
#[derive(Clone, Copy, Debug, Default)]
struct Set {
    log: u8,
    set: bool,
    predefined: bool,
}

/// What a frame's blocks of sequences pass on, each to the next: the
/// tables each kind of code was last decoded with, of literal lengths,
/// offsets, and match lengths, in the order their states start, one after
/// another, so that one pointer reaches all three; and the three offsets
/// last used, the latest first. The tables, some 29 KiB, are allocated
/// by the first block that has sequences, and written where they are:
/// held in the frame, they are written twice, there and where the frame
/// is boxed, which takes a frame of a few kilobytes as long as decoding
/// it.
#[derive(Clone, Debug)]
pub(super) struct Codes {
    tables: Option<Box<[Decoding; 3]>>,
    sets: [Set; 3],
    repeats: [usize; 3],
}

impl Default for Codes {
    fn default() -> Self {
        Self {
            tables: None,
            sets: [Set::default(); 3],
            repeats: [1, 4, 8],
        }
    }
}

impl Codes {
    /// Returns the bytes it holds, allocated.
    pub(super) fn footprint(&self) -> usize {
        self.tables
            .as_ref()
            .map_or(0, |_| size_of::<[Decoding; 3]>())
    }
}

/// Returns the tables of the three kinds of code, none yet built, each
/// written where it is allocated.
fn tables() -> Result<Box<[Decoding; 3]>, Error> {
    let mut tables = Vec::with_capacity(3);
    for _ in 0..3 {
        tables.push(Decoding::NONE);
    }
    tables.into_boxed_slice().try_into().map_err(|_| corrupt())
}

/// Decodes the sequences `section` and runs them with the first `len` of
/// `literals`, which may hold more bytes past them, onto `output`, within
/// its limit.
pub(super) fn decode(
    section: &[u8],
    codes: &mut Codes,
    literals: &[u8],
    len: usize,
    output: &mut Cursor<'_>,
) -> Result<(), Error> {
    let (count, rest) = count(section)?;
    if count == 0 {
        // The section ends with its count, and the block with literals.
        if !rest.is_empty() {
            return Err(corrupt());
        }
        return output.extend(literals.get(..len).ok_or_else(corrupt)?);
    }
    let (&modes, mut rest) = rest.split_first().ok_or_else(corrupt)?;
    if modes & 0x03 != 0 {
        return Err(corrupt());
    }
    let tables = match &mut codes.tables {
        Some(tables) => tables,
        none => none.insert(tables()?),
    };
    let kinds = [LITERAL_LENGTH, OFFSET, MATCH_LENGTH];
    for ((kind, table), set) in kinds.iter().zip(tables.iter_mut()).zip(&mut codes.sets) {
        rest = kind.table(modes, rest, table, set)?;
    }

    // The states start as literal lengths', offsets', and match lengths',
    // in that order.
    let mut bits = Backward::new(rest)?;
    let mut states = [0_usize; 3];
    for (state, set) in states.iter_mut().zip(&codes.sets) {
        *state = usize::try_from(bits.read(u32::from(set.log))).unwrap_or_default();
    }
    // The repeat offsets and the cursor are held apart for the block,
    // which takes them back at its end, unless it fails.
    let mut repeats = codes.repeats;
    let mut cursor = output.apart();
    let (mut literals, mut spare) = (literals, len);
    let mut batch = [Sequence::default(); BATCH];
    let (mut left, mut mixed) = (count, false);
    while left > 0 {
        let batch = batch.get_mut(..left.min(BATCH)).unwrap_or_default();
        left = left.saturating_sub(batch.len());
        read(&mut bits, tables, &mut states, batch, left == 0);
        // What they take in all is checked before any runs, which fails
        // no later than the first that takes too much would.
        let (taken, farthest) = settle(batch, &mut repeats, &mut mixed);
        spare = spare.checked_sub(taken).ok_or_else(corrupt)?;
        if !cursor.reaches(farthest) {
            return Err(corrupt());
        }
        literals = cursor.run(batch, literals)?;
    }
    let end = cursor.end;
    output.end = end;
    if !bits.done() {
        return Err(corrupt());
    }
    codes.repeats = repeats;
    // The literals left over end the block.
    output.extend(literals.get(..spare).ok_or_else(corrupt)?)
}

/// Decodes as many sequences as `batch` holds from `bits`, the `last` of
/// their block or not, by `tables`, from `states`, which it moves on;
/// their offsets as they are coded, yet to be [settled](settle).
///
/// Each whole from one container, while the stream holds one whole and a
/// sequence takes no more than it holds; and one by one, with refills,
/// near the stream's start, and where a sequence takes more. A function
/// of its own, of few values, so that they stay in registers: the bits
/// left, and the states.
#[cfg_attr(
    all(feature = "intrinsics", target_arch = "x86_64"),
    multiversion::multiversion(targets("x86_64+avx2+bmi1+bmi2+lzcnt+popcnt"))
)]
#[inline(never)]
fn read(
    bits: &mut Backward<'_>,
    tables: &[Decoding; 3],
    states: &mut [usize; 3],
    batch: &mut [Sequence],
    last: bool,
) {
    let mut now = *states;
    // The block's last sequence moves no state on.
    let (mut batch, ending) = match batch.split_last_mut() {
        Some((ending, batch)) if last => (batch, Some(ending)),
        _ => (batch, None),
    };
    loop {
        if let Some(mut counted) = bits.counted() {
            let (len, mut each) = (batch.len(), batch.iter_mut());
            let mut whole = len;
            while let Some(sequence) = each.next() {
                if !swift(&mut counted, tables, &mut now, sequence) {
                    // All before this one.
                    whole = len.wrapping_sub(each.len()).wrapping_sub(1);
                    break;
                }
            }
            if whole > 0 {
                bits.recount(&counted);
            }
            batch = batch.get_mut(whole..).unwrap_or_default();
        }
        // The next by refills, and then whole again, if any are left.
        let Some((sequence, rest)) = batch.split_first_mut() else {
            break;
        };
        *sequence = one(bits, tables, &mut now, true);
        batch = rest;
    }
    if let Some(sequence) = ending {
        *sequence = one(bits, tables, &mut now, false);
    }
    *states = now;
}

/// Decodes a sequence whole from one container of `bits`, by `tables`,
/// from `states`, which it moves on to the next, into `sequence`, and
/// returns whether it did; or else has moved nothing on: if the stream
/// holds no container whole, or the sequence takes more than one holds,
/// or a state is none of a table's.
///
/// A container holds 57 bits, which hold a sequence's but for a few:
/// the states' 26, and the values' extra bits, most often far fewer than
/// the 31 left. Each field is shifted down from where
/// it ends, by a sum of the states' [`Counts`], so that the next states
/// wait on little but their tables, and no field on another's bits. A
/// core shifts two words a cycle, so the fields are shifted by turns:
/// the container past the offset's bits serves match lengths' state and
/// literal lengths', each by a sum of two counts, a cycle before offsets'
/// state, whose sum takes three; and the values, which nothing waits on,
/// are read after the states, so that their shifts make way for them.
#[inline(always)]
fn swift(
    bits: &mut Counted<'_>,
    tables: &[Decoding; 3],
    states: &mut [usize; 3],
    sequence: &mut Sequence,
) -> bool {
    let Some(container) = bits.container() else {
        return false;
    };
    let [lengths, offsets, matches] = tables;
    let [length, offset, matched] = *states;
    // What the next states wait on is loaded first, as a core loads only
    // so much a cycle, the earliest asked for first: the counts that
    // their fields are shifted by, and the offset's bits, which the
    // container is shifted past.
    let (Some(&l_long), Some(&m_long), Some(&m_short), Some(&o_extra), Some(&o_long)) = (
        lengths.longs.get(length),
        matches.longs.get(matched),
        matches.shorts.get(matched),
        offsets.extras.get(offset),
        offsets.longs.get(offset),
    ) else {
        return false;
    };
    let (l_long, m_long, o_long) = (u32::from(l_long), u32::from(m_long), u32::from(o_long));
    let m_short = u32::from(m_short);
    let (Some(l), Some(o), Some(m)) = (
        lengths.steps.get(length),
        offsets.steps.get(offset),
        matches.steps.get(matched),
    ) else {
        return false;
    };
    // A shift takes its count's low six bits, and the low half of what
    // is shifted down holds any field whole, below the bits read before
    // it, which its mask clears; as it clears a field of no bits, which
    // may end where no shift reaches.
    let low = |word: u64, by: u32, mask: u32| {
        let low = word.wrapping_shr(by) & u64::from(u32::MAX);
        u32::try_from(low).unwrap_or_default() & mask
    };
    // None overflows: up to 9 bits onto a base of 16, or 31 onto a
    // baseline of 32; and each is summed in a word, which clears the
    // half above it.
    let size = |value: u32| usize::try_from(value).unwrap_or(usize::MAX);
    // The container past the offset's bits.
    let past = container.wrapping_shl(u32::from(o_extra));
    let both = m_long.wrapping_add(l_long);
    let matched_next = m.next.wrapping_add(low(past, both, m.mask));
    let length_next = l
        .next
        .wrapping_add(low(past, m_short.wrapping_add(l_long), l.mask));
    let all = both.wrapping_add(o_long);
    let offset_next = o.next.wrapping_add(low(container, all, o.mask));
    // More bits than a container holds leave less of the three counts,
    // and nothing has moved on: checked by what the states wait on
    // already, so that no load comes before theirs.
    if all < NO_COUNTS.wrapping_sub(Counted::HELD) {
        return false;
    }
    // The values, which nothing waits on.
    let (Some(&l_short), Some(&o_short)) = (lengths.shorts.get(length), offsets.shorts.get(offset))
    else {
        return false;
    };
    let (l_short, o_short) = (u32::from(l_short), u32::from(o_short));
    let values = m_short.wrapping_add(l_short);
    let (Some(length), Some(offset), Some(matched)) = (
        lengths.worths.get(length),
        offsets.worths.get(offset),
        matches.worths.get(matched),
    ) else {
        return false;
    };
    *states = [size(length_next), size(offset_next), size(matched_next)];
    bits.consume(o_long, both);
    let value = |worth: &Worth, word: u64, by: u32| {
        size(worth.baseline.wrapping_add(low(word, by, worth.mask)))
    };
    sequence.offset = value(offset, container, o_short);
    sequence.matched = value(matched, past, m_short);
    sequence.literals = value(length, past, values);
    true
}

/// Decodes a sequence from `bits`, by `tables`, from `states`, and moves
/// them on to the next, if there is a `next`, as [`swift`] does, by a
/// refill, and another where it takes more than one holds.
///
/// It reads its offset's bits, then its match length's, then its literal
/// length's; and then the states move on, literal lengths' first, then
/// match lengths', then offsets'.
#[inline(always)]
fn one(
    bits: &mut Backward<'_>,
    tables: &[Decoding; 3],
    states: &mut [usize; 3],
    next: bool,
) -> Sequence {
    let [lengths, offsets, matches] = tables;
    let [length, offset, matched] = *states;
    let ((l, length), (o, offset), (m, matched)) = (
        lengths.state(length),
        offsets.state(offset),
        matches.state(matched),
    );
    // A refill holds 57 bits: the states' 26, and the values' extra bits,
    // unless they take more than 31, when it takes another between a
    // match length's, up to 47 with an offset's, and a literal length's,
    // up to 16.
    bits.refill();
    let extra = offset.extra().wrapping_add(matched.extra());
    let extra = extra.wrapping_add(length.extra());
    let value = offset.value(bits);
    let match_len = matched.value(bits);
    if extra > 31 {
        bits.refill();
    }
    let literal_len = length.value(bits);
    if next {
        let length = l.next(bits);
        let matched = m.next(bits);
        *states = [length, o.next(bits), matched];
    }
    Sequence {
        literals: literal_len,
        matched: match_len,
        offset: value,
    }
}

/// Settles the offsets of `batch`, each as its sequence codes it, after
/// `repeats`, which it moves on; and returns how many literals the batch
/// takes, and the farthest back that it reaches.
///
/// A pass of its own, between decoding and running, so that neither
/// holds the repeats, which only wait on each other. Whether an offset
/// repeats is branched on where a batch's almost all do, or almost none,
/// as text's; and selected where they are `mixed`, as the last batch's
/// were, which a branch would mistake.
fn settle(batch: &mut [Sequence], repeats: &mut [usize; 3], mixed: &mut bool) -> (usize, usize) {
    let mut latest = *repeats;
    let (mut taken, mut farthest, mut repeated) = (0_usize, 0_usize, 0_usize);
    // None wraps: a batch's lengths are of 17 bits each.
    let mut tally = |sequence: &Sequence| {
        taken = taken.wrapping_add(sequence.literals);
        farthest = farthest.max(sequence.offset);
    };
    if *mixed {
        for sequence in batch.iter_mut() {
            repeated = repeated.wrapping_add(usize::from(sequence.offset <= 3));
            sequence.offset = resolve(sequence.offset, sequence.literals, &mut latest);
            tally(sequence);
        }
    } else {
        for sequence in batch.iter_mut() {
            let [first, second, _] = latest;
            match sequence.offset.checked_sub(3) {
                Some(offset @ 1..) => {
                    sequence.offset = offset;
                    latest = [offset, first, second];
                },
                _ => {
                    repeated = repeated.wrapping_add(1);
                    sequence.offset = resolve(sequence.offset, sequence.literals, &mut latest);
                },
            }
            tally(sequence);
        }
    }
    *repeats = latest;
    // Between an eighth of them and seven.
    let (some, most) = (batch.len() / 8, batch.len().saturating_sub(batch.len() / 8));
    *mixed = repeated > some && repeated < most;
    (taken, farthest)
}

impl Kind {
    /// Returns what `symbol` is worth, and its extra bits.
    fn worth(&self, symbol: u8) -> (Worth, u8) {
        let (baseline, extra) = lookup(self.values, symbol);
        let mask = mask(extra);
        (Worth { baseline, mask }, extra)
    }

    /// Reads the table this kind of code is decoded with from `section`,
    /// as `modes` say, into `table`, which `set` tells of, and returns
    /// the section past it.
    fn table<'a>(
        &self,
        modes: u8,
        section: &'a [u8],
        table: &mut Decoding,
        set: &mut Set,
    ) -> Result<&'a [u8], Error> {
        let mode = (modes >> self.shift) & 0x03;
        let (distribution, rest) = match mode {
            // Predefined, as it may be already.
            0 if set.set && set.predefined => return Ok(section),
            0 => {
                table.preset(self.preset);
                *set = Set {
                    log: self.preset.log,
                    set: true,
                    predefined: true,
                };
                return Ok(section);
            },
            // One symbol, every time.
            1 => {
                let (&symbol, rest) = section.split_first().ok_or_else(corrupt)?;
                if usize::from(symbol) >= self.values.len() {
                    return Err(corrupt());
                }
                table.single(symbol, self);
                *set = Set {
                    log: 0,
                    set: true,
                    predefined: false,
                };
                return Ok(rest);
            },
            // Described.
            2 => {
                let symbols = self.values.len();
                let (read, used) = Distribution::read(section, self.max_log, symbols)?;
                (read, section.get(used..).ok_or_else(corrupt)?)
            },
            // The last block's, which must have been.
            _ if set.set => return Ok(section),
            _ => return Err(corrupt()),
        };
        table.build(&distribution, self)?;
        *set = Set {
            log: distribution.log(),
            set: true,
            predefined: false,
        };
        Ok(rest)
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
/// `literal_len` literals, and updates the `repeats` (RFC 8878 §3.1.1.5);
/// or none, zero, for one less than a repeat of 1, which no copy takes.
///
/// Values past 3 are offsets, less 3; 1 to 3 repeat the latest offsets,
/// but for a sequence without literals, whose 1 and 2 repeat the second
/// and third latest, and whose 3 is one less than the latest. An offset
/// used goes first among the repeats, and those before it move back one.
/// Whether one repeats varies as data does, so each is selected, not
/// branched to: by a subtraction and two selections in turn, which is
/// all that the next sequence's waits on.
#[inline(always)]
fn resolve(value: usize, literal_len: usize, repeats: &mut [usize; 3]) -> usize {
    let [first, second, third] = *repeats;
    let repeat = value <= 3;
    let which = value
        .wrapping_sub(1)
        .wrapping_add(usize::from(literal_len == 0));
    // The latest, or one less than it, or the second; and the third, or
    // an offset of its own.
    let near = first.wrapping_sub(usize::from(which == 3));
    let near = select_unpredictable(which == 1, second, near);
    let far = select_unpredictable(repeat, third, value.wrapping_sub(3));
    let offset = select_unpredictable(repeat & (which != 2), near, far);
    *repeats = [
        offset,
        select_unpredictable(repeat & (which == 0), second, first),
        select_unpredictable(repeat & (which <= 1), third, second),
    ];
    offset
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        fse::{Entry, Table},
        window::Window,
    };
    use alloc::vec::Vec;

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
    /// distributions, each state's symbol, bits, and base; and what a
    /// sequence looks up of each state, its symbol resolved.
    #[test]
    fn builds_the_predefined_tables_as_the_rfc_does() {
        for (kind, default, states) in [
            (
                LITERAL_LENGTH,
                &LITERAL_LENGTH_DEFAULT[..],
                &LITERAL_LENGTH_STATES[..],
            ),
            (
                MATCH_LENGTH,
                &MATCH_LENGTH_DEFAULT[..],
                &MATCH_LENGTH_STATES[..],
            ),
            (OFFSET, &OFFSET_DEFAULT[..], &OFFSET_STATES[..]),
        ] {
            let distribution = Distribution::new(default, kind.preset.log).unwrap();
            let table = Table::new(&distribution).unwrap();
            let mut decoding = Decoding::NONE;
            decoding.build(&distribution, &kind).unwrap();
            // What is built when the crate is, is what a block builds.
            let mut preset = Decoding::NONE;
            preset.preset(kind.preset);
            assert_eq!(states.len(), 1 << kind.preset.log);
            assert_eq!(preset.steps[..states.len()], decoding.steps[..states.len()]);
            assert_eq!(
                preset.worths[..states.len()],
                decoding.worths[..states.len()]
            );
            for state in 0..states.len() {
                assert_eq!(preset.counts(state), decoding.counts(state));
            }
            for (state, &(symbol, bits, base)) in states.iter().enumerate() {
                let expected = Entry { symbol, bits, base };
                assert_eq!(table.entry(state), expected, "{} {state}", kind.shift);
                let (step, worth) = decoding.state(state);
                let (baseline, extra) = kind.values[usize::from(symbol)];
                assert_eq!((step.next, step.bits()), (base.into(), bits.into()));
                assert_eq!((worth.baseline, worth.extra()), (baseline, extra.into()));
                let counts = decoding.counts(state).unwrap();
                assert_eq!(counts.extra, extra);
                assert_eq!((counts.short, counts.long), (64 - extra, 64 - extra - bits));
            }
            // One symbol's table decodes it from its one state, which
            // reads no bits to stay.
            decoding.single(3, &kind);
            let (step, worth) = decoding.state(0);
            let (baseline, extra) = kind.values[3];
            assert_eq!((step.next, step.bits()), (0, 0));
            assert_eq!(decoding.longs[0], 64 - extra);
            assert_eq!(worth.baseline, baseline);
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
            assert_eq!(resolve(value, literal_len, &mut repeats), offset);
            assert_eq!(repeats, after, "{value} after {literal_len}");
        }
        // One less than an offset of 1 is none, which no copy takes.
        let mut repeats = [1, 4, 8];
        assert_eq!(resolve(3, 0, &mut repeats), 0);
        let mut window = Window::new(1 << 10, 32, None);
        window.prepare().unwrap();
        window.extend(b"abc").unwrap();
        let none = Sequence {
            literals: 0,
            matched: 3,
            offset: 0,
        };
        assert!(window.cursor(10).run(&[none], &[]).is_err());
        // Farther back than the window reaches.
        assert!(!window.cursor(10).reaches((1 << 10) + 1));
    }

    /// A batch settles as its offsets resolve one by one, whether it
    /// branches on those that repeat or selects them.
    #[test]
    fn settles_as_it_resolves() {
        let mut seed = 0x2545_F491_4F6C_DD1D_u64;
        let mut below = move |n: usize| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % n as u64) as usize
        };
        for often in [1, 2, 9] {
            let batch: Vec<Sequence> = (0..BATCH)
                .map(|_| Sequence {
                    literals: below(3),
                    matched: 3,
                    offset: if below(often) == 0 {
                        1 + below(3)
                    } else {
                        4 + below(900)
                    },
                })
                .collect();
            let mut repeats = [1, 4, 8];
            let resolved: Vec<usize> = batch
                .iter()
                .map(|sequence| resolve(sequence.offset, sequence.literals, &mut repeats))
                .collect();
            let literals: usize = batch.iter().map(|sequence| sequence.literals).sum();
            let farthest = resolved.iter().copied().max().unwrap();
            for mixed in [false, true] {
                let (mut settled, mut latest, mut mixed) = (batch.clone(), [1, 4, 8], mixed);
                assert_eq!(
                    settle(&mut settled, &mut latest, &mut mixed),
                    (literals, farthest)
                );
                let offsets: Vec<usize> = settled.iter().map(|sequence| sequence.offset).collect();
                assert_eq!((offsets, latest), (resolved.clone(), repeats), "{often}");
                // Mixed where one in two repeats, but not one in nine, nor all.
                assert_eq!(mixed, often == 2, "{often}");
            }
        }
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

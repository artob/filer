// This is free and unencumbered software released into the public domain.

//! LZMA's model: the probabilities its bits are coded with, the state of
//! what was decoded last, and the distances of the latest matches; and
//! the packets it decodes with them, each a literal, a match, or a repeat
//! of a recent match.

use crate::range::{Coder, EVEN};
use alloc::vec::Vec;

/// How many states the model tells what came last by: of the last few
/// packets, which were literals, matches, and repeats.
const STATES: usize = 12;

/// The first state after which a literal is coded against the byte at the
/// latest match's distance.
const AFTER_MATCH: usize = 7;

/// How many positions lengths and packets may be coded by, at most.
const POSITIONS: usize = 1 << 4;

/// The shortest a match is, which its coded length counts from.
const SHORTEST: u32 = 2;

/// The longest a match is.
pub(crate) const LONGEST: usize = 273;

/// The distance that a match ending the stream has, rather than one.
pub(crate) const END: u32 = u32::MAX;

/// The probabilities of a literal, at each of its contexts.
const LITERAL: usize = 0x300;

/// The distance slots below which distances are themselves, and below
/// which the rest of a distance is coded by probabilities, rather than
/// directly and by the align bits.
const SLOT_EXACT: u32 = 4;
const SLOT_DIRECT: u32 = 14;

/// How many probabilities code the low bits of distances below
/// `SLOT_DIRECT`'s, from 1.
const SPECIAL: usize = 1 + 128 - 14;

/// How many of a distance's lowest bits the align probabilities code.
const ALIGN_BITS: u32 = 4;

/// A stream's properties: how many of the previous byte's high bits, and
/// of the position's low bits, select a literal's probabilities, and how
/// many of the position's low bits select a packet's and a length's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Properties {
    lc: u32,
    lp: u32,
    pb: u32,
}

impl Properties {
    /// Returns the properties that `byte` packs, as `(pb * 5 + lp) * 9 +
    /// lc`, or `None` if it packs none.
    pub(crate) fn new(byte: u8) -> Option<Self> {
        let lc = byte.checked_rem(9)?;
        let rest = byte.checked_div(9)?;
        let lp = rest.checked_rem(5)?;
        let pb = rest.checked_div(5)?;
        (pb <= 4).then_some(Self {
            lc: u32::from(lc),
            lp: u32::from(lp),
            pb: u32::from(pb),
        })
    }

    /// Returns how many of the position's bits and the previous byte's
    /// select a literal's probabilities, together.
    pub(crate) const fn literal_bits(self) -> u32 {
        self.lc.saturating_add(self.lp)
    }
}

/// The probabilities of a length: whether it is short, middling, or long,
/// and its bits in each case, the short and middling by position.
#[derive(Clone, Debug)]
struct Lengths {
    choice: u16,
    choice2: u16,
    low: [[u16; 8]; POSITIONS],
    mid: [[u16; 8]; POSITIONS],
    high: [u16; 256],
}

impl Lengths {
    const fn new() -> Self {
        Self {
            choice: EVEN,
            choice2: EVEN,
            low: [[EVEN; 8]; POSITIONS],
            mid: [[EVEN; 8]; POSITIONS],
            high: [EVEN; 256],
        }
    }

    /// Decodes a match's length at `position`, from the shortest.
    #[inline(always)]
    fn decode<C: Coder>(&mut self, coder: &mut C, position: usize) -> Option<u32> {
        let length = if coder.bit(&mut self.choice)? == 0 {
            tree(coder, self.low.get_mut(position)?)?
        } else if coder.bit(&mut self.choice2)? == 0 {
            tree(coder, self.mid.get_mut(position)?)?.wrapping_add(8)
        } else {
            tree(coder, &mut self.high)?.wrapping_add(16)
        };
        Some(length.wrapping_add(SHORTEST))
    }
}

/// Decodes as many bits as index `N` probabilities by a tree of them, the
/// first highest.
#[inline(always)]
fn tree<C: Coder, const N: usize>(coder: &mut C, probabilities: &mut [u16; N]) -> Option<u32> {
    // Each node is below `N`, which the mask shows.
    let mut node = 1_usize;
    while node < N {
        let probability = probabilities.get_mut(node & N.wrapping_sub(1))?;
        node = node.wrapping_shl(1) | usize::try_from(coder.even_bit(probability)?).ok()?;
    }
    u32::try_from(node.wrapping_sub(N)).ok()
}

/// Decodes `bits` bits by a tree of probabilities, the first lowest.
#[inline(always)]
fn reverse<C: Coder>(coder: &mut C, probabilities: &mut [u16], bits: u32) -> Option<u32> {
    let (mut node, mut value) = (1_u32, 0_u32);
    for bit in 0..bits {
        let probability = probabilities.get_mut(usize::try_from(node).ok()?)?;
        let decoded = coder.even_bit(probability)?;
        node = node.wrapping_shl(1) | decoded;
        value |= decoded.wrapping_shl(bit);
    }
    Some(value)
}

/// What a packet decodes to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Packet {
    /// A byte.
    Literal(u8),
    /// The byte at the latest match's distance.
    ShortRep,
    /// A match at the distance of one of the four latest, by its index.
    Rep { index: usize, length: u32 },
    /// A match at a new distance, less one, which is `END` for the end.
    Match { length: u32, distance: u32 },
}

/// What a packet is decoded in: its position's low bits, for its own
/// probabilities and a length's; the index of its literal's probabilities,
/// by the position and the previous byte; and, after a match, the byte at
/// its distance, which a literal is coded against.
///
/// Its fields are as wide as registers, so that building it costs no
/// partial store.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Context {
    pub(crate) position: u32,
    pub(crate) previous: u32,
    pub(crate) matched: Option<u32>,
}

/// LZMA's model, with its properties.
#[derive(Clone, Debug)]
pub(crate) struct Model {
    properties: Properties,
    state: usize,
    /// The four latest distances, less one, the latest first.
    pub(crate) reps: [u32; 4],
    is_match: [u16; STATES * POSITIONS],
    is_rep: [u16; STATES],
    is_rep_g0: [u16; STATES],
    is_rep_g1: [u16; STATES],
    is_rep_g2: [u16; STATES],
    is_rep0_long: [u16; STATES * POSITIONS],
    slots: [[u16; 64]; 4],
    special: [u16; SPECIAL],
    align: [u16; 1 << ALIGN_BITS],
    lengths: Lengths,
    rep_lengths: Lengths,
    literals: Vec<u16>,
}

impl Model {
    /// Returns a model with `properties`, at the start of a stream.
    pub(crate) fn new(properties: Properties) -> Self {
        Self::with(properties, Vec::new())
    }

    /// Starts over, as at the start of a stream, with `properties`.
    pub(crate) fn reset(&mut self, properties: Properties) {
        let literals = core::mem::take(&mut self.literals);
        *self = Self::with(properties, literals);
    }

    /// Returns a model with `properties`, at the start of a stream, whose
    /// literals' probabilities reuse `literals`.
    fn with(properties: Properties, mut literals: Vec<u16>) -> Self {
        let len = LITERAL
            .checked_shl(properties.literal_bits())
            .unwrap_or_default();
        literals.clear();
        literals.resize(len, EVEN);
        Self {
            properties,
            state: 0,
            reps: [0; 4],
            is_match: [EVEN; STATES * POSITIONS],
            is_rep: [EVEN; STATES],
            is_rep_g0: [EVEN; STATES],
            is_rep_g1: [EVEN; STATES],
            is_rep_g2: [EVEN; STATES],
            is_rep0_long: [EVEN; STATES * POSITIONS],
            slots: [[EVEN; 64]; 4],
            special: [EVEN; SPECIAL],
            align: [EVEN; 1 << ALIGN_BITS],
            lengths: Lengths::new(),
            rep_lengths: Lengths::new(),
            literals,
        }
    }

    /// Returns its properties.
    pub(crate) const fn properties(&self) -> Properties {
        self.properties
    }

    /// Returns whether a literal now is coded against the byte at the
    /// latest match's distance.
    pub(crate) const fn after_match(&self) -> bool {
        self.state >= AFTER_MATCH
    }

    /// Decodes a packet in `context`.
    #[inline(always)]
    pub(crate) fn packet<C: Coder>(&mut self, coder: &mut C, context: Context) -> Option<Packet> {
        let state = self.state;
        let position = usize::try_from(context.position & mask(self.properties.pb)).ok()?;
        let at = state.wrapping_mul(POSITIONS).wrapping_add(position);
        if coder.bit(self.is_match.get_mut(at)?)? == 0 {
            return self.literal(coder, context).map(Packet::Literal);
        }
        if coder.bit(self.is_rep.get_mut(state)?)? == 0 {
            let length = self.lengths.decode(coder, position)?;
            let distance = self.distance(coder, length)?;
            return Some(Packet::Match { length, distance });
        }
        let index = if coder.bit(self.is_rep_g0.get_mut(state)?)? == 0 {
            if coder.bit(self.is_rep0_long.get_mut(at)?)? == 0 {
                return Some(Packet::ShortRep);
            }
            0
        } else if coder.bit(self.is_rep_g1.get_mut(state)?)? == 0 {
            1
        } else if coder.bit(self.is_rep_g2.get_mut(state)?)? == 0 {
            2
        } else {
            3
        };
        let length = self.rep_lengths.decode(coder, position)?;
        Some(Packet::Rep { index, length })
    }

    /// Decodes a literal in `context`.
    #[inline(always)]
    fn literal<C: Coder>(&mut self, coder: &mut C, context: Context) -> Option<u8> {
        let Properties { lc, lp, .. } = self.properties;
        let low = context.position & mask(lp);
        let high = context
            .previous
            .checked_shr(8_u32.wrapping_sub(lc))
            .unwrap_or_default();
        let index = usize::try_from(low.wrapping_shl(lc).wrapping_add(high)).ok()?;
        let start = index.checked_mul(LITERAL)?;
        let probabilities = self.literals.get_mut(start..start.checked_add(LITERAL)?)?;
        let probabilities: &mut [u16; LITERAL] = probabilities.try_into().ok()?;
        let mut symbol = 1_usize;
        if let Some(mut matched) = context.matched {
            while symbol < 0x100 {
                let bit = usize::try_from((matched >> 7) & 1).ok()?;
                matched <<= 1;
                let at = bit.wrapping_add(1).wrapping_shl(8).wrapping_add(symbol);
                let decoded = coder.even_bit(probabilities.get_mut(at)?)?;
                symbol = symbol.wrapping_shl(1) | usize::try_from(decoded).ok()?;
                if usize::try_from(decoded).ok()? != bit {
                    break;
                }
            }
        }
        while symbol < 0x100 {
            let decoded = coder.even_bit(probabilities.get_mut(symbol)?)?;
            symbol = symbol.wrapping_shl(1) | usize::try_from(decoded).ok()?;
        }
        let [byte, ..] = symbol.to_le_bytes();
        Some(byte)
    }

    /// Decodes the distance, less one, of a match of `length`.
    #[inline(always)]
    fn distance<C: Coder>(&mut self, coder: &mut C, length: u32) -> Option<u32> {
        let by = usize::try_from(length.wrapping_sub(SHORTEST).min(3)).ok()?;
        let slot = tree(coder, self.slots.get_mut(by)?)?;
        if slot < SLOT_EXACT {
            return Some(slot);
        }
        let bits = (slot >> 1).wrapping_sub(1);
        let base = (2 | (slot & 1)).wrapping_shl(bits);
        if slot < SLOT_DIRECT {
            let from = usize::try_from(base.wrapping_sub(slot)).ok()?;
            let low = reverse(coder, self.special.get_mut(from..)?, bits)?;
            return Some(base.wrapping_add(low));
        }
        let direct = coder.direct(bits.wrapping_sub(ALIGN_BITS))?;
        let align = reverse(coder, &mut self.align, ALIGN_BITS)?;
        Some(
            base.wrapping_add(direct.wrapping_shl(ALIGN_BITS))
                .wrapping_add(align),
        )
    }

    /// Moves on past a literal.
    pub(crate) fn literal_done(&mut self) {
        self.state = match self.state {
            0..4 => 0,
            4..10 => self.state.wrapping_sub(3),
            _ => self.state.wrapping_sub(6),
        };
    }

    /// Moves on past a match at a new distance, less one, `distance`.
    pub(crate) fn match_done(&mut self, distance: u32) {
        self.state = if self.after_match() { 10 } else { 7 };
        let [r0, r1, r2, _] = self.reps;
        self.reps = [distance, r0, r1, r2];
    }

    /// Moves on past a repeat of the match whose distance is at `index`,
    /// which becomes the latest.
    pub(crate) fn rep_done(&mut self, index: usize) {
        self.state = if self.after_match() { 11 } else { 8 };
        let [r0, r1, r2, r3] = self.reps;
        self.reps = match index {
            0 => [r0, r1, r2, r3],
            1 => [r1, r0, r2, r3],
            2 => [r2, r0, r1, r3],
            _ => [r3, r0, r1, r2],
        };
    }

    /// Moves on past a repeat of the latest match's byte.
    pub(crate) fn short_rep_done(&mut self) {
        self.state = if self.after_match() { 11 } else { 9 };
    }
}

/// Returns the low `bits` bits set.
const fn mask(bits: u32) -> u32 {
    match 1_u32.checked_shl(bits) {
        Some(bit) => bit.wrapping_sub(1),
        None => u32::MAX,
    }
}

// This is free and unencumbered software released into the public domain.

//! The range decoder, which LZMA codes every bit with: each bit either by
//! an adaptive probability, or directly, as likely one as the other.

/// How many bits a probability has: it is of a 0, out of `1 << 11`.
const PROBABILITY_BITS: u32 = 11;

/// How fast a probability adapts: by a 32nd of what it lacks.
const ADAPT_BITS: u32 = 5;

/// A probability, before any bit has been coded with it: even odds.
pub(crate) const EVEN: u16 = 1 << (PROBABILITY_BITS - 1);

/// Below this, the range takes another byte of input.
const TOP: u32 = 1 << 24;

/// The bytes a stream begins with: a zero, then the code's initial
/// value.
pub(crate) const INIT: usize = 5;

/// A range decoder's state: the width of its range, and where in it the
/// code lies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Range {
    range: u32,
    code: u32,
}

impl Range {
    /// Returns the decoder that `bytes`, a stream's first, begin, or `None`
    /// if they cannot begin one, as they must begin with a zero.
    pub(crate) fn new(bytes: [u8; INIT]) -> Option<Self> {
        let [zero, code @ ..] = bytes;
        (zero == 0).then_some(Self {
            range: u32::MAX,
            code: u32::from_be_bytes(code),
        })
    }

    /// Returns whether the stream can end here: the code at the bottom of
    /// the range.
    pub(crate) const fn finished(self) -> bool {
        self.code == 0
    }
}

/// Decodes bits with a [`Range`] from input: as they are, updating the
/// probabilities, or, in a dry run, only to learn whether the input holds
/// enough for them. Either returns `None` when the input runs out.
pub(crate) trait Coder {
    /// Decodes a bit, with `probability` the chance of a 0.
    fn bit(&mut self, probability: &mut u16) -> Option<u32>;

    /// Decodes a bit as `bit` does, but without a branch, for bits as
    /// likely one as the other, which a branch would mispredict, as a
    /// literal's are.
    fn even_bit(&mut self, probability: &mut u16) -> Option<u32> {
        self.bit(probability)
    }

    /// Decodes `count` bits, up to 26, directly, the first highest.
    fn direct(&mut self, count: u32) -> Option<u32>;
}

/// A [`Coder`] decoding for real, from `input` past `at`.
pub(crate) struct Real<'a> {
    pub(crate) range: Range,
    pub(crate) input: &'a [u8],
    pub(crate) at: usize,
}

/// A [`Coder`] in a dry run, from `input` past `at`: it changes no
/// probability, and stops when the input runs out.
pub(crate) struct Dry<'a> {
    pub(crate) range: Range,
    pub(crate) input: &'a [u8],
    pub(crate) at: usize,
}

/// Takes the next byte of `input` past `at` into `range`, if its range is
/// narrow enough to need it; or returns `None` if there is none.
#[inline(always)]
fn normalize(range: &mut Range, input: &[u8], at: &mut usize) -> Option<()> {
    if range.range < TOP {
        let byte = *input.get(*at)?;
        *at = at.wrapping_add(1);
        range.range = range.range.wrapping_shl(8);
        range.code = range.code.wrapping_shl(8) | u32::from(byte);
    }
    Some(())
}

/// Returns where the code is split between a 0 and a 1 of `probability`.
#[inline]
fn bound(range: Range, probability: u16) -> u32 {
    range
        .range
        .wrapping_shr(PROBABILITY_BITS)
        .wrapping_mul(u32::from(probability))
}

impl Coder for Real<'_> {
    #[inline(always)]
    fn bit(&mut self, probability: &mut u16) -> Option<u32> {
        let bound = bound(self.range, *probability);
        let bit = if self.range.code < bound {
            self.range.range = bound;
            let rise = (1_u16 << PROBABILITY_BITS).wrapping_sub(*probability) >> ADAPT_BITS;
            *probability = probability.wrapping_add(rise);
            0
        } else {
            self.range.range = self.range.range.wrapping_sub(bound);
            self.range.code = self.range.code.wrapping_sub(bound);
            *probability = probability.wrapping_sub(*probability >> ADAPT_BITS);
            1
        };
        normalize(&mut self.range, self.input, &mut self.at)?;
        Some(bit)
    }

    #[inline(always)]
    fn even_bit(&mut self, probability: &mut u16) -> Option<u32> {
        let p = *probability;
        let bound = bound(self.range, p);
        // All ones for a 1, which takes the upper part of the range.
        let bit = u32::from(self.range.code >= bound);
        let mask = bit.wrapping_neg();
        let upper = self.range.range.wrapping_sub(bound);
        self.range.range = (bound & !mask) | (upper & mask);
        self.range.code = self.range.code.wrapping_sub(bound & mask);
        let rise = p.wrapping_add((1_u16 << PROBABILITY_BITS).wrapping_sub(p) >> ADAPT_BITS);
        let fall = p.wrapping_sub(p >> ADAPT_BITS);
        let [low, ..] = mask.to_le_bytes();
        let mask = u16::from_le_bytes([low, low]);
        *probability = (rise & !mask) | (fall & mask);
        normalize(&mut self.range, self.input, &mut self.at)?;
        Some(bit)
    }

    #[inline(always)]
    fn direct(&mut self, count: u32) -> Option<u32> {
        direct(&mut self.range, self.input, &mut self.at, count)
    }
}

impl Coder for Dry<'_> {
    fn bit(&mut self, probability: &mut u16) -> Option<u32> {
        let bound = bound(self.range, *probability);
        let bit = if self.range.code < bound {
            self.range.range = bound;
            0
        } else {
            self.range.range = self.range.range.wrapping_sub(bound);
            self.range.code = self.range.code.wrapping_sub(bound);
            1
        };
        normalize(&mut self.range, self.input, &mut self.at)?;
        Some(bit)
    }

    fn direct(&mut self, count: u32) -> Option<u32> {
        direct(&mut self.range, self.input, &mut self.at, count)
    }
}

/// Decodes `count` bits directly, as [`Coder::direct`] does.
#[inline(always)]
fn direct(range: &mut Range, input: &[u8], at: &mut usize, count: u32) -> Option<u32> {
    let mut value = 0_u32;
    for _ in 0..count {
        range.range >>= 1;
        let bit = u32::from(range.code >= range.range);
        if bit == 1 {
            range.code = range.code.wrapping_sub(range.range);
        }
        value = value.wrapping_shl(1) | bit;
        normalize(range, input, at)?;
    }
    Some(value)
}

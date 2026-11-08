// This is free and unencumbered software released into the public domain.

//! Huffman coding (RFC 8878 §4.2), of literals: the prefix codes that a
//! tree's description gives each byte, and the streams coded with them.

use super::{
    bits::{Backward, Marked},
    corrupt,
    fse::Table,
};
use crate::Error;

/// The longest a code may be, and so the heaviest a weight.
const MAX_BITS: u8 = 11;

/// The most weights a description lists: of each byte but the last,
/// whose weight is deduced from the others.
const MAX_WEIGHTS: usize = 255;

/// Room for one more weight than may be listed, to tell too many.
const ROOM: usize = 256;

/// What a stream's next bits decode to: a symbol, and the length of its
/// code.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Code {
    symbol: u8,
    bits: u8,
}

/// The most codes a table indexes: by the longest code's bits.
const MAX_CODES: usize = 1 << MAX_BITS;

/// The codes read from a refill, of up to 11 bits each, of the 57 it
/// holds.
const PER_REFILL: usize = 5;

/// The most symbols a run decodes to, and the room a refill's runs take,
/// as each writes four bytes.
const RUN: u32 = 3;
const RUNS_ROOM: usize = PER_REFILL * 3 + 1;

/// The fewest literals for which a table's runs are worth building, and
/// the most bits each may take, on average, for its runs to be used.
const RUNS_WORTH: usize = 16 << 10;
const RUNS_BITS: usize = 6;

/// A decoding table, indexed by a stream's next `bits` bits, the length of
/// the longest code: held in an array of the most any table takes, so
/// that it is never allocated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Huffman {
    bits: u8,
    codes: [Code; MAX_CODES],
}

/// A table's runs, indexed by a stream's next 11 bits: what they decode
/// to whole, up to three symbols, from the lowest byte, then the bits of
/// their codes, and how many, in the top byte's low and high nibbles; so
/// that short codes decode several at a time. Built from a table once
/// enough literals use it to be worth it, and held apart from it, in an
/// array a frame keeps, so that a table read costs no more for them.
#[derive(Clone, Debug)]
pub(super) struct Runs {
    runs: [u32; MAX_CODES],
    built: bool,
}

impl Default for Runs {
    fn default() -> Self {
        Self {
            runs: [0; MAX_CODES],
            built: false,
        }
    }
}

impl Runs {
    /// Forgets the table they were built from, which another replaces.
    pub(super) const fn clear(&mut self) {
        self.built = false;
    }

    /// Returns them, built from `huffman`, unless built already, if
    /// `literals`, coded in `bytes`, are worth it: enough of them, of
    /// codes short enough that runs hold several.
    pub(super) fn of(&mut self, huffman: &Huffman, literals: usize, bytes: usize) -> Option<&Self> {
        if bytes.saturating_mul(8) > literals.saturating_mul(RUNS_BITS) {
            return None;
        }
        if !self.built && literals >= RUNS_WORTH {
            self.runs = runs(&huffman.codes, huffman.bits);
            self.built = true;
        }
        self.built.then_some(self)
    }

    /// Decodes a refill's runs from `bits` onto `output` from `at`, which
    /// must have room for them, and returns where they end.
    fn decode(&self, bits: &mut Backward<'_>, output: &mut [u8], at: usize) -> usize {
        bits.refill();
        let mut at = at;
        for _ in 0..PER_REFILL {
            at = self.run(bits, output, at);
        }
        at
    }

    /// Decodes the next run from `bits`, as [`run`](Self::run) does.
    #[inline(always)]
    fn marked(&self, bits: &mut Marked, output: &mut [u8], at: usize) -> usize {
        let index = usize::try_from(bits.peek(u32::from(MAX_BITS))).unwrap_or_default();
        let index = index & MAX_CODES.wrapping_sub(1);
        let run = self.runs.get(index).copied().unwrap_or_default();
        let slots = output.get_mut(at..).and_then(<[u8]>::first_chunk_mut);
        if let Some(slots) = slots {
            *slots = run.to_le_bytes();
        }
        bits.consume((run >> 24) & 0x0F);
        at.wrapping_add(usize::try_from(run >> 28).unwrap_or_default())
    }

    /// Decodes the next run from `bits`, which must hold its codes, onto
    /// `output` at `at`, which must have room for four bytes, and returns
    /// where it ends.
    #[inline(always)]
    fn run(&self, bits: &mut Backward<'_>, output: &mut [u8], at: usize) -> usize {
        let index = usize::try_from(bits.peek(u32::from(MAX_BITS))).unwrap_or_default();
        let index = index & MAX_CODES.wrapping_sub(1);
        let run = self.runs.get(index).copied().unwrap_or_default();
        // Four bytes, the last of which the next run overwrites.
        let slots = output.get_mut(at..).and_then(<[u8]>::first_chunk_mut);
        if let Some(slots) = slots {
            *slots = run.to_le_bytes();
        }
        bits.consume((run >> 24) & 0x0F);
        at.wrapping_add(usize::try_from(run >> 28).unwrap_or_default())
    }
}

impl Huffman {
    /// Reads a tree's description (RFC 8878 §4.2.1) from the start of
    /// `bytes`, and returns its table and the bytes it took.
    pub(super) fn read(bytes: &[u8]) -> Result<(Self, usize), Error> {
        let (&header, rest) = bytes.split_first().ok_or_else(corrupt)?;
        let mut all = [0_u8; ROOM];
        let (len, count) = match header.checked_sub(127) {
            // Coded with FSE, in `header` bytes.
            None | Some(0) => {
                let len = usize::from(header);
                (len, fse(rest.get(..len).ok_or_else(corrupt)?, &mut all)?)
            },
            // Or four bits each, `header - 127` of them, at most 128.
            Some(count) => {
                let count = usize::from(count);
                let len = count.div_ceil(2);
                let packed = rest.get(..len).ok_or_else(corrupt)?;
                let weights = packed.iter().flat_map(|&byte| [byte >> 4, byte & 0x0F]);
                for (slot, weight) in all.iter_mut().zip(weights.take(count)) {
                    *slot = weight;
                }
                (len, count)
            },
        };
        let weights = all.get(..count).ok_or_else(corrupt)?;
        Ok((Self::new(weights)?, len.saturating_add(1)))
    }

    /// Returns the table of codes of `weights`, each byte's in turn, but
    /// for the last listed's successor, whose weight brings their total
    /// to a power of two.
    fn new(weights: &[u8]) -> Result<Self, Error> {
        if weights.len() > MAX_WEIGHTS || weights.iter().any(|&weight| weight > MAX_BITS) {
            return Err(corrupt());
        }
        let share = |weight: u8| 1_u32.wrapping_shl(u32::from(weight)) >> 1;
        let total: u32 = weights.iter().map(|&weight| share(weight)).sum();
        let bits = total.checked_ilog2().ok_or_else(corrupt)?.saturating_add(1);
        let whole = 1_u32.wrapping_shl(bits);
        let rest = whole.saturating_sub(total);
        if bits > u32::from(MAX_BITS) || !rest.is_power_of_two() {
            return Err(corrupt());
        }
        let last = u8::try_from(rest.ilog2().saturating_add(1)).map_err(|_| corrupt())?;
        let bits = u8::try_from(bits).map_err(|_| corrupt())?;
        let weights = || weights.iter().copied().chain([last]).enumerate();

        // The codes are dealt out lightest weight first, each weight's in
        // the order of their symbols: where each weight's first is.
        let mut starts = [0_u32; 12];
        for (_, weight) in weights() {
            if let Some(start) = starts.get_mut(usize::from(weight)) {
                *start = start.saturating_add(share(weight));
            }
        }
        let mut at = 0_u32;
        for start in starts.iter_mut().skip(1) {
            (*start, at) = (at, at.saturating_add(*start));
        }
        let mut codes = [Code::default(); MAX_CODES];
        for (symbol, weight) in weights().filter(|&(_, weight)| weight > 0) {
            let start = starts.get_mut(usize::from(weight)).ok_or_else(corrupt)?;
            let from = usize::try_from(*start).map_err(|_| corrupt())?;
            *start = start.saturating_add(share(weight));
            let to = usize::try_from(*start).map_err(|_| corrupt())?;
            codes.get_mut(from..to).ok_or_else(corrupt)?.fill(Code {
                symbol: u8::try_from(symbol).map_err(|_| corrupt())?,
                bits: bits.saturating_add(1).saturating_sub(weight),
            });
        }
        Ok(Self { bits, codes })
    }

    /// Decodes `stream` into `output`, which it must fill exactly, by
    /// `runs`, if built.
    pub(super) fn decode(
        &self,
        runs: Option<&Runs>,
        stream: &[u8],
        output: &mut [u8],
    ) -> Result<(), Error> {
        let mut bits = Backward::new(stream)?;
        let mut at = 0_usize;
        if let Some(runs) = runs {
            while at.saturating_add(RUNS_ROOM) <= output.len() {
                at = runs.decode(&mut bits, output, at);
            }
        }
        self.rest(&mut bits, output.get_mut(at..).unwrap_or_default());
        if bits.done() { Ok(()) } else { Err(corrupt()) }
    }

    /// Decodes four streams, which lie one after another in `streams`,
    /// the first three of `lens`, into the quarters of `output`, rounded
    /// up but for the last, which each must fill exactly, by `runs`, if
    /// built: a code of each in turn, their readers held apart, so that
    /// they stay in registers, and the processor looks up each one's codes
    /// while it looks up the others'; and while far from their starts, by
    /// [`Marked`] readers, which hold fewer.
    pub(super) fn decode4(
        &self,
        runs: Option<&Runs>,
        streams: &[u8],
        lens: [usize; 3],
        output: &mut [u8],
    ) -> Result<(), Error> {
        let [one, two, three] = lens;
        let starts = [0, one, one.saturating_add(two)];
        let [s, t, u] = starts;
        let v = u.saturating_add(three);
        let stream = |from: usize, to: usize| streams.get(from..to).ok_or_else(corrupt);
        let (mut a, mut b) = (Backward::new(stream(s, t)?)?, Backward::new(stream(t, u)?)?);
        let (mut c, mut d) = (
            Backward::new(stream(u, v)?)?,
            Backward::new(streams.get(v..).ok_or_else(corrupt)?)?,
        );
        let quarter = output.len().div_ceil(4);
        let [one, two, three] = [1, 2, 3].map(|n| quarter.saturating_mul(n).min(output.len()));
        let four = output.len();
        let (mut w, mut x, mut y, mut z) = (0, one, two, three);
        let room = |w: usize, x: usize, y: usize, z: usize, room: usize| {
            w.saturating_add(room) <= one
                && x.saturating_add(room) <= two
                && y.saturating_add(room) <= three
                && z.saturating_add(room) <= four
        };
        let marked = (a.mark(s), b.mark(t), c.mark(u), d.mark(v));
        if let (Some(mut e), Some(mut f), Some(mut g), Some(mut h)) = marked {
            let far = |e: &Marked, f: &Marked, g: &Marked, h: &Marked| {
                e.at >= s.saturating_add(8)
                    && f.at >= t.saturating_add(8)
                    && g.at >= u.saturating_add(8)
                    && h.at >= v.saturating_add(8)
            };
            if let Some(runs) = runs {
                while room(w, x, y, z, RUNS_ROOM) && far(&e, &f, &g, &h) {
                    e.refill(streams);
                    f.refill(streams);
                    g.refill(streams);
                    h.refill(streams);
                    for _ in 0..PER_REFILL {
                        w = runs.marked(&mut e, output, w);
                        x = runs.marked(&mut f, output, x);
                        y = runs.marked(&mut g, output, y);
                        z = runs.marked(&mut h, output, z);
                    }
                }
            } else {
                while room(w, x, y, z, PER_REFILL) && far(&e, &f, &g, &h) {
                    e.refill(streams);
                    f.refill(streams);
                    g.refill(streams);
                    h.refill(streams);
                    for _ in 0..PER_REFILL {
                        w = self.marked(&mut e, output, w);
                        x = self.marked(&mut f, output, x);
                        y = self.marked(&mut g, output, y);
                        z = self.marked(&mut h, output, z);
                    }
                }
            }
            a.unmark(e, s);
            b.unmark(f, t);
            c.unmark(g, u);
            d.unmark(h, v);
        }
        if let Some(runs) = runs {
            while room(w, x, y, z, RUNS_ROOM) {
                a.refill();
                b.refill();
                c.refill();
                d.refill();
                for _ in 0..PER_REFILL {
                    w = runs.run(&mut a, output, w);
                    x = runs.run(&mut b, output, x);
                    y = runs.run(&mut c, output, y);
                    z = runs.run(&mut d, output, z);
                }
            }
        } else {
            while room(w, x, y, z, PER_REFILL) {
                a.refill();
                b.refill();
                c.refill();
                d.refill();
                for _ in 0..PER_REFILL {
                    w = self.next(&mut a, output, w);
                    x = self.next(&mut b, output, x);
                    y = self.next(&mut c, output, y);
                    z = self.next(&mut d, output, z);
                }
            }
        }
        let each = [(a, w, one), (b, x, two), (c, y, three), (d, z, four)];
        for (mut bits, at, end) in each {
            self.rest(&mut bits, output.get_mut(at..end).unwrap_or_default());
            if !bits.done() {
                return Err(corrupt());
            }
        }
        Ok(())
    }

    /// Decodes the next symbol from `bits`, which must hold its code, onto
    /// `output` at `at`, and returns where it ends.
    #[inline(always)]
    fn marked(&self, bits: &mut Marked, output: &mut [u8], at: usize) -> usize {
        let index = usize::try_from(bits.peek(u32::from(self.bits))).unwrap_or_default();
        let index = index & MAX_CODES.wrapping_sub(1);
        let code = self.codes.get(index).copied().unwrap_or_default();
        bits.consume(u32::from(code.bits));
        if let Some(slot) = output.get_mut(at) {
            *slot = code.symbol;
        }
        at.wrapping_add(1)
    }

    /// Decodes the next symbol from `bits`, which must hold its code, onto
    /// `output` at `at`, and returns where it ends.
    #[inline(always)]
    fn next(&self, bits: &mut Backward<'_>, output: &mut [u8], at: usize) -> usize {
        let symbol = self.symbol(bits);
        if let Some(slot) = output.get_mut(at) {
            *slot = symbol;
        }
        at.wrapping_add(1)
    }

    /// Decodes from `bits` into `output`, which they fill.
    fn rest(&self, bits: &mut Backward<'_>, output: &mut [u8]) {
        for slots in output.chunks_mut(PER_REFILL) {
            bits.refill();
            for slot in slots {
                *slot = self.symbol(bits);
            }
        }
    }

    /// Decodes the next symbol from `bits`, which must hold its code.
    fn symbol(&self, bits: &mut Backward<'_>) -> u8 {
        let index = usize::try_from(bits.peek(u32::from(self.bits))).unwrap_or_default();
        // The index is of at most 11 bits, which spares checking it.
        let index = index & MAX_CODES.wrapping_sub(1);
        let code = self.codes.get(index).copied().unwrap_or_default();
        bits.consume(u32::from(code.bits));
        code.symbol
    }
}

/// Returns the runs of `codes`, of up to `bits` bits each: for each 11
/// bits, the codes they hold whole, one at least, up to three.
fn runs(codes: &[Code; MAX_CODES], bits: u8) -> [u32; MAX_CODES] {
    let mut runs = [0_u32; MAX_CODES];
    let shift = u32::from(MAX_BITS.saturating_sub(bits));
    let mask = u32::try_from(MAX_CODES.wrapping_sub(1)).unwrap_or_default();
    for (index, run) in (0_u32..).zip(&mut runs) {
        // The bits left, from the top, with zeros after them.
        let (mut window, mut left) = (index, u32::from(MAX_BITS));
        let (mut symbols, mut used, mut count) = (0_u32, 0_u32, 0_u32);
        while count < RUN {
            let at = usize::try_from(window >> shift).unwrap_or_default();
            let code = codes.get(at).copied().unwrap_or_default();
            let len = u32::from(code.bits);
            if count > 0 && (len > left || len == 0) {
                break;
            }
            symbols |= u32::from(code.symbol).wrapping_shl(count.wrapping_mul(8));
            (used, left) = (used.wrapping_add(len), left.saturating_sub(len));
            count = count.wrapping_add(1);
            window = window.wrapping_shl(len) & mask;
        }
        *run = symbols | used << 24 | count << 28;
    }
    runs
}

/// Decodes weights coded with FSE (RFC 8878 §4.2.1.2) from `bytes` onto
/// `weights`: a table's description, and a stream that two states read in
/// turn, until one's next state would read past its start, after which
/// the other's symbol is the last.
fn fse(bytes: &[u8], weights: &mut [u8; ROOM]) -> Result<usize, Error> {
    let (table, used) = Table::read(bytes, 6, usize::from(MAX_BITS).saturating_add(1))?;
    let mut bits = Backward::new(bytes.get(used..).unwrap_or_default())?;
    let mut states = [table.start(&mut bits), table.start(&mut bits)];
    let mut len = 0_usize;
    for turn in [0, 1].into_iter().cycle() {
        if len >= MAX_WEIGHTS {
            return Err(corrupt());
        }
        let state = states.get_mut(turn).ok_or_else(corrupt)?;
        let entry = table.entry(*state);
        *weights.get_mut(len).ok_or_else(corrupt)? = entry.symbol;
        len = len.saturating_add(1);
        bits.refill();
        *state = Table::next(entry, &mut bits);
        if bits.overflowed() {
            let other = states
                .get(1_usize.saturating_sub(turn))
                .ok_or_else(corrupt)?;
            *weights.get_mut(len).ok_or_else(corrupt)? = table.entry(*other).symbol;
            len = len.saturating_add(1);
            break;
        }
    }
    Ok(len)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{vec, vec::Vec};

    /// RFC 8878 §4.2.1's example: the weights of bytes 0 to 4, 4, 3, 2, 0,
    /// and 1, and so 1 for byte 5, coded directly.
    #[test]
    fn deals_codes_as_the_rfc_does() {
        let (huffman, used) = Huffman::read(&[127 + 5, 0x43, 0x20, 0x10]).unwrap();
        assert_eq!(used, 4);
        assert_eq!(huffman.bits, 4);
        let code = |symbol: u8| {
            let at = huffman.codes.iter().position(|code| code.symbol == symbol);
            (at.unwrap(), huffman.codes[at.unwrap()].bits)
        };
        // Table 25: 4 is 0000, 5 0001, 2 001, 1 01, and 0 1.
        assert_eq!(
            [4, 5, 2, 1, 0].map(code),
            [(0, 4), (1, 4), (2, 3), (4, 2), (8, 1)]
        );
        assert!(!huffman.codes.iter().any(|code| code.symbol == 3));
        // "0154", as Table 25's codes have it, from the stream that Table
        // 26 codes "0145" in, as it swaps the codes of 4 and 5.
        let mut output = [0; 4];
        huffman
            .decode(None, &[0b0001_0000, 0b0000_1101], &mut output)
            .unwrap();
        assert_eq!(output, [0, 1, 5, 4]);
    }

    /// Codes `symbols` with `huffman`'s codes, as a stream read backward:
    /// the first code first, after the marker, from the top.
    fn encode(huffman: &Huffman, symbols: &[u8]) -> Vec<u8> {
        let mut bits = vec![true];
        for &symbol in symbols {
            let at = huffman.codes.iter().position(|code| code.symbol == symbol);
            let len = huffman.codes[at.unwrap()].bits;
            let code = at.unwrap() >> (huffman.bits - len);
            bits.extend((0..len).rev().map(|bit| code >> bit & 1 == 1));
        }
        let mut stream = vec![0_u8; bits.len().div_ceil(8)];
        for (at, _) in bits.iter().rev().enumerate().filter(|(_, bit)| **bit) {
            stream[at / 8] |= 1 << (at % 8);
        }
        stream
    }

    /// Short codes and long, each decoded by symbols and by runs, from one
    /// stream and from four.
    #[test]
    fn decodes_by_runs_as_by_symbols() {
        // Four symbols of two bits, as DNA's; and 32 of 1 to 11 bits.
        let short = Huffman::new(&[1, 1, 1]).unwrap();
        let long = Huffman::new(&[
            5, 5, 3, 3, 7, 1, 7, 7, 1, 7, 7, 7, 7, 2, 4, 5, 6, 3, 7, 6, 6, 7, 7, 3, 4, 7, 4, 7, 3,
            6, 6,
        ])
        .unwrap();
        assert_eq!(long.bits, 11);
        for (huffman, symbols) in [(short, 4), (long, 32)] {
            let mut seed = 1_u32;
            let original: Vec<u8> = (0..RUNS_WORTH + 13)
                .map(|_| {
                    seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                    u8::try_from((seed >> 16) % symbols).unwrap()
                })
                .collect();
            let mut runs = Runs::default();
            let built = runs.of(&huffman, RUNS_WORTH, 0).cloned();
            assert!(built.is_some());
            for runs in [None, built.as_ref()] {
                let stream = encode(&huffman, &original);
                let mut output = vec![0; original.len()];
                huffman.decode(runs, &stream, &mut output).unwrap();
                assert_eq!(output, original);
                // Too short a stream for what it decodes to.
                let mut output = vec![0; original.len() + 1];
                assert!(huffman.decode(runs, &stream, &mut output).is_err());
                // Four streams, the last shorter.
                let quarter = original.len().div_ceil(4);
                let parts: Vec<&[u8]> = original.chunks(quarter).collect();
                let streams: Vec<Vec<u8>> =
                    parts.iter().map(|part| encode(&huffman, part)).collect();
                let mut output = vec![0; original.len()];
                let lens = [0, 1, 2].map(|n| streams[n].len());
                let streams = streams.concat();
                huffman.decode4(runs, &streams, lens, &mut output).unwrap();
                assert_eq!(output, original);
            }
        }
    }

    #[test]
    fn refuses_weights_that_make_no_tree() {
        // Weights that leave no power of two to complete, or none at all.
        assert!(Huffman::new(&[2, 2, 1]).is_err());
        assert!(Huffman::new(&[0, 0]).is_err());
        // Codes longer than 11 bits.
        assert!(Huffman::new(&[12]).is_err());
        assert!(Huffman::new(&[11]).is_ok());
    }
}

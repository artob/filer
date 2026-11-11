// This is free and unencumbered software released into the public domain.

//! Huffman coding (RFC 8878 §4.2), of literals: the prefix codes that a
//! tree's description gives each byte, and the streams coded with them.

use super::{
    bits::{Backward, Marked},
    corrupt,
    fse::{Distribution, Table},
};
use crate::Error;
use alloc::{boxed::Box, vec};

/// The longest a code may be, and so the heaviest a weight.
const MAX_BITS: u8 = 11;

/// The most weights a description lists: of each byte but the last,
/// whose weight is deduced from the others.
const MAX_WEIGHTS: usize = 255;

/// Room for one more weight than may be listed, to tell too many.
const ROOM: usize = 256;

/// What a stream's next bits decode to: the length of the code they begin
/// with, in the low byte, and its symbol, in the high one. A shift takes
/// only its count's low bits, so a stream is shifted past a code by the
/// code itself, as it is loaded, and waits on nothing else.
type Code = u16;

/// Returns the code of `symbol`, of `bits`.
const fn code(symbol: u8, bits: u8) -> Code {
    u16::from_le_bytes([bits, symbol])
}

/// Returns the symbol of `code`, and the length it is shifted past by.
fn symbol(code: Code) -> (u8, u32) {
    let [bits, symbol] = code.to_le_bytes();
    (symbol, u32::from(bits))
}

/// The most codes a table indexes: by the longest code's bits.
const MAX_CODES: usize = 1 << MAX_BITS;

/// The codes read from a refill, of up to 11 bits each, of the 57 it
/// holds.
const PER_REFILL: usize = 5;

/// The room a refill's runs of a stream take: each up to three symbols,
/// written as four bytes.
const RUNS_ROOM: usize = PER_REFILL * 3 + 1;

/// Where a run's symbols lie, above the bits of their codes, and where
/// their count does.
const RUN_BITS: u32 = 6;
const RUN_COUNT: u32 = 30;

/// The fewest literals for which a table's runs are worth building, and
/// the most bits each may take, on average, for its runs to be used.
const RUNS_WORTH: usize = 16 << 10;
const RUNS_BITS: usize = 6;

/// A decoding table, indexed by a stream's next 11 bits, the longest a
/// code may be, each code's entry repeated for the bits past it, so that
/// a shift by as many as any table takes indexes it: held in an array,
/// so that it is never allocated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Huffman {
    codes: [Code; MAX_CODES],
}

/// A table's runs, indexed by a stream's next 11 bits: what they decode
/// to whole, up to three symbols, so that short codes decode several at a
/// time. In a run's low six bits, the bits of its codes, which a stream
/// is shifted past by the run itself, as by a [`Code`]; above them, its
/// symbols, the first lowest; and in its two high bits, how many they
/// are: each a shift away, and no more. Built from a table once enough
/// literals use it to be worth it, and held apart from it, in an array
/// that a frame allocates when it first builds them and then keeps, so
/// that a table read costs no more for them, nor a frame that never
/// builds any.
#[derive(Clone, Debug, Default)]
pub(super) struct Runs {
    runs: Option<Box<Several>>,
    built: bool,
}

/// A table's runs, as its loops take them.
type Several = [u32; MAX_CODES];

impl Runs {
    /// Forgets the table they were built from, which another replaces.
    pub(super) const fn clear(&mut self) {
        self.built = false;
    }

    /// Returns them, built from `huffman`, unless built already, if
    /// `literals`, coded in `bytes`, are worth it: enough of them, of
    /// codes short enough that runs hold several.
    pub(super) fn of(
        &mut self,
        huffman: &Huffman,
        literals: usize,
        bytes: usize,
    ) -> Option<&Several> {
        if bytes.saturating_mul(8) > literals.saturating_mul(RUNS_BITS) {
            return None;
        }
        if !self.built && literals >= RUNS_WORTH {
            let table = match &mut self.runs {
                Some(table) => table,
                // Zeros, which the allocator may hand out as they are.
                none => {
                    let zeros = vec![0; MAX_CODES].into_boxed_slice();
                    none.insert(zeros.try_into().ok()?)
                },
            };
            runs(&huffman.codes, table);
            self.built = true;
        }
        self.runs.as_deref().filter(|_| self.built)
    }

    /// Returns the bytes it holds, allocated.
    pub(super) fn footprint(&self) -> usize {
        self.runs.as_ref().map_or(0, |_| size_of::<Several>())
    }
}

/// Decodes the next of `runs` from `bits`, which must hold its codes,
/// onto the start of `room`, and returns how many symbols it is, up to
/// three: four bytes written, the last of which the next run overwrites.
#[inline(always)]
fn run(runs: &Several, bits: &mut Marked<'_>, room: &mut [u8; 4]) -> usize {
    let index = usize::try_from(bits.peek(u32::from(MAX_BITS))).unwrap_or_default();
    let run = runs.get(index).copied().unwrap_or_default();
    bits.consume(run);
    *room = (run >> RUN_BITS).to_le_bytes();
    usize::try_from(run >> RUN_COUNT).unwrap_or_default()
}

/// Decodes a refill's `runs` from each of `bits`, which must hold them,
/// onto each of `rooms`, in turn, and returns how many symbols each
/// decoded to, up to fifteen.
#[inline(always)]
fn round(
    runs: &Several,
    bits: [&mut Marked<'_>; 4],
    rooms: [&mut [u8; RUNS_ROOM]; 4],
) -> [usize; 4] {
    let [e, f, g, h] = bits;
    let [w, x, y, z] = rooms;
    let (mut i, mut j, mut k, mut l) = (0_usize, 0_usize, 0_usize, 0_usize);
    for _ in 0..PER_REFILL {
        // Each within its room: four runs before it took twelve bytes
        // at most.
        if let (Some(w), Some(x), Some(y), Some(z)) = (
            w.get_mut(i..).and_then(<[u8]>::first_chunk_mut),
            x.get_mut(j..).and_then(<[u8]>::first_chunk_mut),
            y.get_mut(k..).and_then(<[u8]>::first_chunk_mut),
            z.get_mut(l..).and_then(<[u8]>::first_chunk_mut),
        ) {
            i = i.wrapping_add(run(runs, e, w));
            j = j.wrapping_add(run(runs, f, x));
            k = k.wrapping_add(run(runs, g, y));
            l = l.wrapping_add(run(runs, h, z));
        }
    }
    [i, j, k, l]
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
        let mut all = [0_u8; ROOM];
        let (listed, after) = all
            .split_at_mut_checked(weights.len())
            .ok_or_else(corrupt)?;
        listed.copy_from_slice(weights);
        *after.first_mut().ok_or_else(corrupt)? = last;
        let weights = all
            .get(..weights.len().saturating_add(1))
            .ok_or_else(corrupt)?;

        // The codes are dealt out lightest weight first, each weight's in
        // the order of their symbols: the symbols sorted so, by counting
        // how many each weight has, and so where its first lies. A weight
        // is of four bits, which index sixteen counts unchecked.
        let mut starts = [0_usize; 16];
        for &weight in weights {
            let next = usize::from(weight).wrapping_add(1) & 0x0F;
            if let Some(count) = starts.get_mut(next) {
                *count = count.wrapping_add(1);
            }
        }
        let mut at = 0_usize;
        for start in &mut starts {
            at = at.wrapping_add(*start);
            *start = at;
        }
        let mut sorted = [0_u8; ROOM];
        for (symbol, &weight) in (0_u8..=u8::MAX).zip(weights) {
            if let Some(start) = starts.get_mut(usize::from(weight & 0x0F)) {
                // Fewer than 257, as the weights are.
                if let Some(slot) = sorted.get_mut(*start & 0xFF) {
                    *slot = symbol;
                }
                *start = start.wrapping_add(1);
            }
        }
        // Each weight's symbols now lie before where the next one's did:
        // those of no weight first, which have no code.
        let mut codes = [0; MAX_CODES];
        let shift = MAX_BITS.saturating_sub(bits);
        let mut rest = codes.as_mut_slice();
        let (mut from, mut weight) = (starts.first().copied().unwrap_or_default(), 1_u8);
        for &to in starts.get(1..=usize::from(MAX_BITS)).unwrap_or_default() {
            let symbols = sorted.get(from..to).ok_or_else(corrupt)?;
            // As many entries each as codes of the longest the table
            // takes begin with the weight's, a power of two.
            let each = 1_usize.wrapping_shl(u32::from(weight.saturating_sub(1)));
            let each = each.wrapping_shl(u32::from(shift));
            let len = bits.saturating_add(1).saturating_sub(weight);
            let all = each.saturating_mul(symbols.len());
            let (entries, after) = rest.split_at_mut_checked(all).ok_or_else(corrupt)?;
            deal(entries, symbols, each, len);
            (rest, from, weight) = (after, to, weight.saturating_add(1));
        }
        Ok(Self { codes })
    }

    /// Decodes `stream` into `output`, which it must fill exactly.
    pub(super) fn decode(&self, stream: &[u8], output: &mut [u8]) -> Result<(), Error> {
        let mut bits = Backward::new(stream)?;
        self.rest(&mut bits, output);
        if bits.done() { Ok(()) } else { Err(corrupt()) }
    }

    /// Decodes four streams, which lie one after another in `streams`,
    /// the first three of `lens`, into the quarters of `output`, rounded
    /// up but for the last, which each must fill exactly, by `runs`, if
    /// built: a code of each in turn, their readers held apart, so that
    /// they stay in registers, and the processor looks up each one's codes
    /// while it looks up the others'; and while far from their starts, by
    /// [`Marked`] readers, which hold fewer, as many rounds at once as
    /// their streams and their quarters have room for at the least, so
    /// that a round checks neither.
    pub(super) fn decode4(
        &self,
        runs: Option<&Several>,
        streams: &[u8],
        lens: [usize; 3],
        output: &mut [u8],
    ) -> Result<(), Error> {
        let [one, two, three] = lens;
        let (first, rest) = streams.split_at_checked(one).ok_or_else(corrupt)?;
        let (second, rest) = rest.split_at_checked(two).ok_or_else(corrupt)?;
        let (third, fourth) = rest.split_at_checked(three).ok_or_else(corrupt)?;
        let mut readers = [
            Backward::new(first)?,
            Backward::new(second)?,
            Backward::new(third)?,
            Backward::new(fourth)?,
        ];
        let quarter = output.len().div_ceil(4);
        let (first, rest) = output.split_at_mut_checked(quarter).ok_or_else(corrupt)?;
        let (second, rest) = rest.split_at_mut_checked(quarter).ok_or_else(corrupt)?;
        let (third, fourth) = rest.split_at_mut_checked(quarter).ok_or_else(corrupt)?;
        let mut quarters = [first, second, third, fourth];
        // How far each quarter is decoded.
        let mut ends = [0_usize; 4];
        let [a, b, c, d] = &readers;
        if let (Some(e), Some(f), Some(g), Some(h)) = (a.mark(), b.mark(), c.mark(), d.mark()) {
            let mut marked = [e, f, g, h];
            match runs {
                Some(runs) => several(runs, &mut marked, &mut quarters, &mut ends),
                None => singly(self, &mut marked, &mut quarters, &mut ends),
            }
            for (reader, marked) in readers.iter_mut().zip(&marked) {
                reader.unmark(marked);
            }
        }
        for ((mut bits, quarter), end) in readers.into_iter().zip(quarters).zip(ends) {
            self.rest(&mut bits, quarter.get_mut(end..).unwrap_or_default());
            if !bits.done() {
                return Err(corrupt());
            }
        }
        Ok(())
    }

    /// Decodes and returns the next symbol from `bits`, which must hold
    /// its code.
    #[inline(always)]
    fn marked(&self, bits: &mut Marked<'_>) -> u8 {
        let index = usize::try_from(bits.peek(u32::from(MAX_BITS))).unwrap_or_default();
        let code = self.codes.get(index).copied().unwrap_or_default();
        bits.consume(u32::from(code));
        symbol(code).0
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
        let index = usize::try_from(bits.peek(u32::from(MAX_BITS))).unwrap_or_default();
        // The index is of at most 11 bits, which spares checking it.
        let index = index & MAX_CODES.wrapping_sub(1);
        let (symbol, len) = symbol(self.codes.get(index).copied().unwrap_or_default());
        bits.consume(len);
        symbol
    }
}

/// Decodes from the four streams of `bits` onto `quarters` from
/// `ends`, which it moves on, while each stream is far from its start
/// and each quarter has room: five symbols of each a round, by `huffman`,
/// stored as they are decoded. With `intrinsics`, it is built for BMI2
/// too, whose shifts take any register, and taken where the CPU has it.
#[cfg_attr(
    all(feature = "intrinsics", target_arch = "x86_64"),
    multiversion::multiversion(targets("x86_64+avx2+bmi1+bmi2+lzcnt+popcnt"))
)]
fn singly(
    huffman: &Huffman,
    bits: &mut [Marked<'_>; 4],
    quarters: &mut [&mut [u8]; 4],
    ends: &mut [usize; 4],
) {
    loop {
        let rounds = rounds(bits, quarters, ends, PER_REFILL);
        if rounds == 0 {
            return;
        }
        let [first, second, third, fourth] = quarters;
        let [i, j, k, l] = *ends;
        let rooms = whole(first, i)
            .zip(whole(second, j))
            .zip(whole(third, k))
            .zip(whole(fourth, l));
        // The readers held apart, so that they stay in registers:
        // behind a reference, each store of a symbol might be to one.
        let [mut e, mut f, mut g, mut h] = *bits;
        let mut taken = 0_usize;
        for (((w, x), y), z) in rooms.take(rounds) {
            // Each has bytes for as many refills as there are rounds.
            if !(e.refill() && f.refill() && g.refill() && h.refill()) {
                break;
            }
            let each = w.iter_mut().zip(x).zip(y).zip(z);
            for (((w, x), y), z) in each {
                *w = huffman.marked(&mut e);
                *x = huffman.marked(&mut f);
                *y = huffman.marked(&mut g);
                *z = huffman.marked(&mut h);
            }
            taken = taken.wrapping_add(PER_REFILL);
        }
        *bits = [e, f, g, h];
        for end in ends.iter_mut() {
            *end = end.wrapping_add(taken);
        }
        if taken == 0 {
            return;
        }
    }
}

/// Decodes from the four streams of `bits` onto `quarters` from
/// `ends`, which it moves on, while each stream is far from its start
/// and each quarter has room: five of `runs` of each a round. With
/// `intrinsics`, it is built for BMI2 too, and taken where the CPU has it.
#[cfg_attr(
    all(feature = "intrinsics", target_arch = "x86_64"),
    multiversion::multiversion(targets("x86_64+avx2+bmi1+bmi2+lzcnt+popcnt"))
)]
fn several(
    runs: &Several,
    bits: &mut [Marked<'_>; 4],
    quarters: &mut [&mut [u8]; 4],
    ends: &mut [usize; 4],
) {
    loop {
        let rounds = rounds(bits, quarters, ends, RUNS_ROOM);
        if rounds == 0 {
            return;
        }
        let before = *ends;
        let [first, second, third, fourth] = quarters;
        let [mut i, mut j, mut k, mut l] = *ends;
        // The readers held apart, so that they stay in registers:
        // behind a reference, each store of a run might be to one.
        let [mut e, mut f, mut g, mut h] = *bits;
        for _ in 0..rounds {
            let (Some(w), Some(x), Some(y), Some(z)) = (
                room(first, i),
                room(second, j),
                room(third, k),
                room(fourth, l),
            ) else {
                break;
            };
            // Each has bytes for as many refills as there are rounds.
            if !(e.refill() && f.refill() && g.refill() && h.refill()) {
                break;
            }
            let readers = [&mut e, &mut f, &mut g, &mut h];
            let [m, n, o, p] = round(runs, readers, [w, x, y, z]);
            i = i.wrapping_add(m);
            j = j.wrapping_add(n);
            k = k.wrapping_add(o);
            l = l.wrapping_add(p);
        }
        *bits = [e, f, g, h];
        *ends = [i, j, k, l];
        if *ends == before {
            return;
        }
    }
}

/// Deals `entries` out to `symbols`, `each` of them to one in turn, as
/// its code, of `len` bits: by copies of a fixed size where a symbol has
/// few, as most have, so that none is a call.
fn deal(entries: &mut [Code], symbols: &[u8], each: usize, len: u8) {
    /// The same for a size known when built.
    fn fixed<const N: usize>(entries: &mut [Code], symbols: &[u8], len: u8) {
        let (entries, _) = entries.as_chunks_mut::<N>();
        for (entries, &symbol) in entries.iter_mut().zip(symbols) {
            *entries = [code(symbol, len); N];
        }
    }
    match each {
        1 => fixed::<1>(entries, symbols, len),
        2 => fixed::<2>(entries, symbols, len),
        4 => fixed::<4>(entries, symbols, len),
        8 => fixed::<8>(entries, symbols, len),
        16 => fixed::<16>(entries, symbols, len),
        _ => {
            for (entries, &symbol) in entries.chunks_mut(each.max(1)).zip(symbols) {
                entries.fill(code(symbol, len));
            }
        },
    }
}

/// Returns the room that a refill's runs take of `quarter` from `end`, if
/// it has as much.
fn room(quarter: &mut [u8], end: usize) -> Option<&mut [u8; RUNS_ROOM]> {
    quarter.get_mut(end..)?.first_chunk_mut()
}

/// Returns the rooms of a refill's symbols each that `quarter` holds whole
/// from `end` on.
fn whole(quarter: &mut [u8], end: usize) -> core::slice::IterMut<'_, [u8; PER_REFILL]> {
    let rest = quarter.get_mut(end..).unwrap_or_default();
    let (whole, _) = rest.as_chunks_mut();
    whole.iter_mut()
}

/// Returns how many rounds the four streams of `bits`, and `quarters`,
/// decoded as far as `ends`, have room for, at the least: a round refills
/// each stream once, and takes up to `room` of each quarter.
fn rounds(
    bits: &[Marked<'_>; 4],
    quarters: &[&mut [u8]; 4],
    ends: &[usize; 4],
    room: usize,
) -> usize {
    let streams = bits.iter().map(Marked::refills);
    let rooms = quarters.iter().zip(ends);
    let rooms = rooms.map(|(quarter, &end)| {
        let left = quarter.len().saturating_sub(end);
        left.checked_div(room).unwrap_or_default()
    });
    streams.chain(rooms).min().unwrap_or_default()
}

/// Sets `runs` to those of `codes`: for each 11 bits, the codes they hold
/// whole, one at least, up to three, each looked up past the one before,
/// and counted in if it ends within the 11 bits; without a branch, as
/// which do varies from one index to the next.
fn runs(codes: &[Code; MAX_CODES], runs: &mut Several) {
    let all = u32::from(MAX_BITS);
    let mask = u32::try_from(MAX_CODES.wrapping_sub(1)).unwrap_or_default();
    let look = |window: u32| {
        let at = usize::try_from(window & mask).unwrap_or_default();
        symbol(codes.get(at).copied().unwrap_or_default())
    };
    for (index, run) in (0_u32..).zip(runs) {
        // The bits left, from the top, with zeros after them.
        let (first, one) = look(index);
        let window = index.wrapping_shl(one);
        let (second, two) = look(window);
        let two = one.wrapping_add(two);
        let (third, three) = look(window.wrapping_shl(two.wrapping_sub(one)));
        let three = two.wrapping_add(three);
        let (pair, triple) = (two <= all, three <= all);
        let used = if triple {
            three
        } else if pair {
            two
        } else {
            one
        };
        let count = 1_u32
            .wrapping_add(u32::from(pair))
            .wrapping_add(u32::from(triple));
        let symbols = u32::from_le_bytes([first, second, third, 0]);
        *run = used | symbols << RUN_BITS | count << RUN_COUNT;
    }
}

/// Decodes weights coded with FSE (RFC 8878 §4.2.1.2) from `bytes` onto
/// `weights`: a table's description, and a stream that two states read in
/// turn, until one's next state would read past its start, after which
/// the other's symbol is the last.
fn fse(bytes: &[u8], weights: &mut [u8; ROOM]) -> Result<usize, Error> {
    let symbols = usize::from(MAX_BITS).saturating_add(1);
    let (distribution, used) = Distribution::read(bytes, Table::MAX_LOG, symbols)?;
    let table = Table::new(&distribution)?;
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
        let code = |of: u8| {
            let at = huffman.codes.iter().position(|&code| symbol(code).0 == of);
            (at.unwrap() >> 7, symbol(huffman.codes[at.unwrap()]).1)
        };
        // Table 25: 4 is 0000, 5 0001, 2 001, 1 01, and 0 1.
        assert_eq!(
            [4, 5, 2, 1, 0].map(code),
            [(0, 4), (1, 4), (2, 3), (4, 2), (8, 1)]
        );
        assert!(!huffman.codes.iter().any(|&code| symbol(code).0 == 3));
        // "0154", as Table 25's codes have it, from the stream that Table
        // 26 codes "0145" in, as it swaps the codes of 4 and 5.
        let mut output = [0; 4];
        huffman
            .decode(&[0b0001_0000, 0b0000_1101], &mut output)
            .unwrap();
        assert_eq!(output, [0, 1, 5, 4]);
    }

    /// Codes `symbols` with `huffman`'s codes, as a stream read backward:
    /// the first code first, after the marker, from the top.
    fn encode(huffman: &Huffman, symbols: &[u8]) -> Vec<u8> {
        let mut bits = vec![true];
        for &of in symbols {
            let at = huffman.codes.iter().position(|&code| symbol(code).0 == of);
            let len = symbol(huffman.codes[at.unwrap()]).1;
            let code = at.unwrap() >> (u32::from(MAX_BITS) - len);
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
        assert!(long.codes.iter().any(|&code| symbol(code).1 == 11));
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
            let stream = encode(&huffman, &original);
            let mut output = vec![0; original.len()];
            huffman.decode(&stream, &mut output).unwrap();
            assert_eq!(output, original);
            // Too short a stream for what it decodes to.
            let mut output = vec![0; original.len() + 1];
            assert!(huffman.decode(&stream, &mut output).is_err());
            for runs in [None, built.as_ref()] {
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

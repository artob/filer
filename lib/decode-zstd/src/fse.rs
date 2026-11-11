// This is free and unencumbered software released into the public domain.

//! Finite State Entropy (RFC 8878 §4.1): the distributions that describe
//! the decoding tables of zstd's sequences and Huffman weights, and how a
//! table's states follow from one.

use super::{
    bits::{Backward, Forward},
    corrupt,
};
use crate::Error;

/// The finest accuracy of any table, as a log, and so its most states.
const MAX_LOG: u8 = 9;
pub(super) const MAX_STATES: usize = 1 << MAX_LOG;

/// The most symbols of any distribution: those of match lengths.
pub(super) const MAX_SYMBOLS: usize = 53;

/// As many of a symbol as are laid at once, before they are spread: more
/// than most symbols' counts, so that few take a second turn.
const LAID: usize = 32;

/// A distribution: each symbol's count of a table's `1 << log` states,
/// or -1 for "less than 1", which takes one.
#[derive(Clone, Debug)]
pub(super) struct Distribution {
    counts: [i16; MAX_SYMBOLS],
    len: usize,
    log: u8,
}

impl Distribution {
    /// Reads a distribution's description (RFC 8878 §4.1.1) from the start
    /// of `bytes`, of an accuracy of at most `max_log` and at most
    /// `symbols` symbols, and returns it and the bytes it took.
    pub(super) fn read(bytes: &[u8], max_log: u8, symbols: usize) -> Result<(Self, usize), Error> {
        let mut bits = Forward::new(bytes);
        let log = u8::try_from(bits.read(4)?).map_err(|_| corrupt())?;
        let log = log.saturating_add(5);
        if log > max_log.min(MAX_LOG) {
            return Err(corrupt());
        }
        // The probability points left to distribute; each symbol's is
        // read in as few bits as the values it may still take need.
        let mut left = 1_u32.checked_shl(u32::from(log)).ok_or_else(corrupt)?;
        let symbols = symbols.min(MAX_SYMBOLS);
        let (mut counts, mut len) = ([0_i16; MAX_SYMBOLS], 0_usize);
        let mut push = |count: i16| -> Result<(), Error> {
            if len >= symbols {
                return Err(corrupt());
            }
            *counts.get_mut(len).ok_or_else(corrupt)? = count;
            len = len.saturating_add(1);
            Ok(())
        };
        // Each count is read from bits looked at ahead, and one more is
        // read past or not by what they are, not by a branch on them, as
        // which it is varies; so whether the stream holds them is told
        // once they are all read, of bits that are zeros past its end.
        while left > 0 {
            let max = left.saturating_add(1);
            let width = u32::BITS.saturating_sub(max.leading_zeros());
            let short = mask(width).saturating_sub(max);
            let ahead = u32::try_from(bits.peek() & u64::from(u32::MAX)).unwrap_or_default();
            let high = 1_u32.wrapping_shl(width.saturating_sub(1));
            let low = ahead & high.wrapping_sub(1);
            // A value below the short ones' count takes a bit less; any
            // other is past them if its last bit is set.
            let small = low < short;
            let more = !small & (ahead & high != 0);
            let value = low.wrapping_add(if more { high.wrapping_sub(short) } else { 0 });
            bits.skip(width.wrapping_sub(u32::from(small)));
            // A probability of -1, "less than 1", takes a point too.
            let count = i16::try_from(value)
                .map_err(|_| corrupt())?
                .saturating_sub(1);
            left = left
                .checked_sub(u32::from(count.unsigned_abs()))
                .ok_or_else(corrupt)?;
            push(count)?;
            // Zeros are followed by how many more follow, in twos.
            if count == 0 {
                loop {
                    let repeat = bits.peek() & 3;
                    bits.skip(2);
                    for _ in 0..repeat {
                        push(0)?;
                    }
                    if repeat < 3 {
                        break;
                    }
                }
            }
        }
        if !bits.within() {
            return Err(corrupt());
        }
        // The points are all distributed, so the counts take every state.
        Ok((Self { counts, len, log }, bits.bytes()))
    }

    /// Returns the distribution of `counts`, which must take each of the
    /// `1 << log` states.
    #[cfg(test)]
    pub(super) fn new(counts: &[i16], log: u8) -> Result<Self, Error> {
        let whole = 1_u32.checked_shl(u32::from(log)).ok_or_else(corrupt)?;
        let total = counts.iter().try_fold(0_u32, |total, &count| match count {
            -1.. => total.checked_add(u32::from(count.unsigned_abs())),
            _ => None,
        });
        if total != Some(whole) || log > MAX_LOG {
            return Err(corrupt());
        }
        let mut all = [0_i16; MAX_SYMBOLS];
        all.get_mut(..counts.len())
            .ok_or_else(corrupt)?
            .copy_from_slice(counts);
        Ok(Self {
            counts: all,
            len: counts.len(),
            log,
        })
    }

    /// Returns the accuracy, as a log, and so the states' count.
    pub(super) const fn log(&self) -> u8 {
        self.log
    }

    /// Returns each symbol's count.
    fn counts(&self) -> &[i16] {
        self.counts.get(..self.len).unwrap_or_default()
    }

    /// Returns how many symbols it counts.
    pub(super) const fn symbols(&self) -> usize {
        self.len
    }

    /// Deals the states out to the symbols, into `symbols`, and returns
    /// each state's, in order: those of less than 1 take the last states,
    /// from the end, and the others are spread over the rest, by a step
    /// that visits each state once.
    ///
    /// The others are first laid out in a row, each as many times as its
    /// count, and then each state that the step visits below the last
    /// ones takes the next of the row: so that neither a count nor a
    /// state above is branched on, which vary as distributions do.
    pub(super) fn spread<'a>(&self, symbols: &'a mut [u8; MAX_STATES]) -> Result<&'a [u8], Error> {
        let size = 1_usize.wrapping_shl(u32::from(self.log)).min(MAX_STATES);
        let last = size.wrapping_sub(1);
        // Each by copies of a fixed size, whose excess the next overwrites;
        // and those of less than 1 apart, each over the last that was not.
        let mut row = [0_u8; MAX_STATES + LAID];
        let mut lows = [0_u8; MAX_SYMBOLS];
        let (mut laid, mut low) = (0_usize, 0_usize);
        for (symbol, &count) in (0_u8..).zip(self.counts()) {
            if let Some(slot) = lows.get_mut(low) {
                *slot = symbol;
            }
            low = low.wrapping_add(usize::from(count == -1));
            let mut at = laid;
            laid = laid.saturating_add(usize::try_from(count).unwrap_or_default());
            loop {
                let copy = row.get_mut(at..).and_then(<[u8]>::first_chunk_mut);
                *copy.ok_or_else(corrupt)? = [symbol; LAID];
                at = at.saturating_add(LAID);
                if at >= laid {
                    break;
                }
            }
        }
        // The row takes every state below the last ones.
        let high = size.checked_sub(low).ok_or_else(corrupt)?;
        if laid != high {
            return Err(corrupt());
        }
        // An odd step, which visits each state once before the first again.
        let step = (size >> 1).wrapping_add(size >> 3).wrapping_add(3);
        let mut next = 0_usize;
        for visit in 0..size {
            let at = visit.wrapping_mul(step) & last;
            // Within both, as `last` is, and as no more are taken than
            // visited: a state at the end takes one that the next takes
            // too, and then its own.
            if let (Some(slot), Some(&symbol)) = (symbols.get_mut(at), row.get(next)) {
                *slot = symbol;
            }
            next = next.wrapping_add(usize::from(at < high));
        }
        let top = symbols.get_mut(high..size).ok_or_else(corrupt)?;
        for (slot, &symbol) in top.iter_mut().rev().zip(&lows) {
            *slot = symbol;
        }
        symbols.get(..size).ok_or_else(corrupt)
    }

    /// Returns the number of each symbol's first state.
    pub(super) fn numbers(&self) -> Numbers {
        let mut numbers = [0_u16; Numbers::ROOM];
        for (number, &count) in numbers.iter_mut().zip(self.counts()) {
            // A symbol of less than 1 has one state.
            *number = count.unsigned_abs();
        }
        Numbers { numbers }
    }
}

/// The number of each symbol's next state, in the order of its states,
/// from its count on: which tells how the state finds the one after it,
/// from the bits that it reads more, added to a base.
///
/// A symbol of `count` states has them take shares of the next state's
/// range, as its numbers from `count` to twice that, less one, would:
/// each reads as many bits as bring its number to the table's size, and
/// adds them to what its number, shifted so, is past that size. So the
/// first of them, up to the next power of two, read a bit more than the
/// rest, and start where the range does once the rest have their share.
#[derive(Clone, Debug)]
pub(super) struct Numbers {
    /// Of a power of two of symbols, so that none is checked.
    numbers: [u16; Self::ROOM],
}

impl Numbers {
    /// More than the most symbols.
    pub(super) const ROOM: usize = MAX_SYMBOLS.next_power_of_two();

    /// Returns where `symbol` lies among as many as there is room for.
    #[inline(always)]
    pub(super) fn index(symbol: u8) -> usize {
        usize::from(symbol) & Self::ROOM.wrapping_sub(1)
    }

    /// Returns the number of the next of `symbol`'s states: the states'
    /// in order, one each time.
    #[inline(always)]
    pub(super) fn take(&mut self, symbol: u8) -> u32 {
        let Some(next) = self.numbers.get_mut(Self::index(symbol)) else {
            return 0;
        };
        let number = *next;
        *next = number.wrapping_add(1);
        u32::from(number)
    }

    /// Returns the bits that a state of `number` reads, in a table of an
    /// accuracy of `log`: as many as bring its number to the table's
    /// size. A number is at least 1, of a symbol that has states, and less
    /// than twice that size, so this never wraps.
    #[inline(always)]
    pub(super) fn bits(number: u32, log: u8) -> u8 {
        let most = u8::try_from((number | 1).ilog2()).unwrap_or_default();
        log.wrapping_sub(most)
    }
}

/// A state of a decoding table: the symbol it decodes, and how to find
/// the next, from `bits` more read and added to `base`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Entry {
    pub(super) symbol: u8,
    pub(super) bits: u8,
    pub(super) base: u16,
}

/// A decoding table of an accuracy of up to 6, as that of Huffman weights
/// is: one entry per state, held in an array of the most it takes, so
/// that it is never allocated. Sequences build tables of their own, each
/// state's symbol resolved to what it is worth.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Table {
    log: u8,
    entries: [Entry; Self::STATES],
}

impl Table {
    /// Its finest accuracy, as a log, and so its most states.
    pub(super) const MAX_LOG: u8 = 6;
    const STATES: usize = 1 << Self::MAX_LOG;

    /// Returns the table of `distribution`, of an accuracy it holds.
    pub(super) fn new(distribution: &Distribution) -> Result<Self, Error> {
        if distribution.log > Self::MAX_LOG {
            return Err(corrupt());
        }
        let mut symbols = [0; MAX_STATES];
        let states = distribution.spread(&mut symbols)?;
        let mut numbers = distribution.numbers();
        let mut entries = [Entry::default(); Self::STATES];
        for (entry, &symbol) in entries.iter_mut().zip(states) {
            let number = numbers.take(symbol);
            let bits = Numbers::bits(number, distribution.log);
            let whole = 1_u32.wrapping_shl(u32::from(distribution.log));
            let base = number.wrapping_shl(u32::from(bits)).wrapping_sub(whole);
            let base = u16::try_from(base).unwrap_or_default();
            *entry = Entry { symbol, bits, base };
        }
        Ok(Self {
            log: distribution.log,
            entries,
        })
    }

    /// Returns the state that `bits` begin with.
    pub(super) fn start(&self, bits: &mut Backward<'_>) -> usize {
        usize::try_from(bits.read(u32::from(self.log))).unwrap_or_default()
    }

    /// Returns the entry of `state`.
    pub(super) fn entry(&self, state: usize) -> Entry {
        self.entries.get(state).copied().unwrap_or_default()
    }

    /// Returns the state after `entry`'s, from `bits`, which must hold
    /// them, up to 6, without a refill.
    pub(super) fn next(entry: Entry, bits: &mut Backward<'_>) -> usize {
        let more = usize::try_from(bits.take(u32::from(entry.bits))).unwrap_or_default();
        usize::from(entry.base).saturating_add(more)
    }
}

/// Returns the low `n` bits set.
fn mask(n: u32) -> u32 {
    1_u32
        .checked_shl(n)
        .map_or(u32::MAX, |bit| bit.wrapping_sub(1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    /// Each state's symbol, as RFC 8878 §4.1.1 defines it: those of less
    /// than 1 from the end, and the others each placed a step past the
    /// last, skipping the states at the end.
    fn placed(counts: &[i16], log: u8) -> Vec<u8> {
        let size = 1_usize << log;
        let mut symbols = alloc::vec![0_u8; size];
        let mut high = size;
        for (symbol, _) in counts.iter().enumerate().filter(|(_, count)| **count == -1) {
            high -= 1;
            symbols[high] = symbol as u8;
        }
        let step = (size >> 1) + (size >> 3) + 3;
        let mut at = 0;
        for (symbol, &count) in counts.iter().enumerate() {
            for _ in 0..count.max(0) {
                symbols[at] = symbol as u8;
                at = (at + step) & (size - 1);
                while at >= high {
                    at = (at + step) & (size - 1);
                }
            }
        }
        assert_eq!(at, 0);
        symbols
    }

    /// Each state's entry, as RFC 8878 §4.1.1 defines it: a symbol's
    /// states numbered from its count, in their order, each reading as
    /// many bits as bring its number to the table's size.
    fn defined(counts: &[i16], log: u8) -> Vec<Entry> {
        let mut numbers: Vec<u32> = counts
            .iter()
            .map(|count| count.unsigned_abs().into())
            .collect();
        let entries = placed(counts, log).into_iter().map(|symbol| {
            let number = &mut numbers[usize::from(symbol)];
            let bits = u32::from(log) - number.ilog2();
            let base = (*number << bits) - (1 << log);
            *number += 1;
            Entry {
                symbol,
                bits: bits as u8,
                base: base as u16,
            }
        });
        entries.collect()
    }

    /// The entries that a distribution deals out, each state's in order.
    fn dealt(counts: &[i16], log: u8) -> Vec<Entry> {
        let distribution = Distribution::new(counts, log).unwrap();
        let mut symbols = [0; MAX_STATES];
        let states = distribution.spread(&mut symbols).unwrap();
        let mut numbers = distribution.numbers();
        let entries = states.iter().map(|&symbol| {
            let number = numbers.take(symbol);
            let bits = Numbers::bits(number, log);
            let base = ((number << bits) - (1 << log)) as u16;
            Entry { symbol, bits, base }
        });
        entries.collect()
    }

    /// RFC 8878 §4.1.1's example, a symbol of 5 states of 128: their
    /// bits and bases, from Table 21, in the order of the states.
    #[test]
    fn numbers_states_as_the_rfc_does() {
        let fives: Vec<_> = dealt(&[5, 123], 7)
            .iter()
            .filter(|entry| entry.symbol == 0)
            .map(|entry| (entry.bits, entry.base))
            .collect();
        assert_eq!(fives, [(5, 32), (5, 64), (5, 96), (4, 0), (4, 16)]);
    }

    /// Counts of every kind: powers of two and not, one, less than one,
    /// none, and a symbol that takes every state; at every accuracy a
    /// description may have, from 5 up.
    #[test]
    fn deals_states_as_the_rfc_defines() {
        let mut seed = 0x9E37_79B9_u32;
        let mut below = move |n: u32| {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            (seed >> 16) % n
        };
        for log in 5..=MAX_LOG {
            let whole = 1_i16 << log;
            assert_eq!(dealt(&[whole], log), defined(&[whole], log));
            for _ in 0..40 {
                // Counts made up until they take every state.
                let (mut counts, mut left) = (Vec::new(), whole);
                while left > 0 && counts.len() < MAX_SYMBOLS - 1 {
                    let count = match below(6) {
                        0 => 0,
                        1 => -1,
                        2 => 1,
                        _ => 1 + below(left as u32) as i16,
                    };
                    left -= count.abs();
                    counts.push(count);
                }
                if left > 0 {
                    counts.push(left);
                }
                let entries = dealt(&counts, log);
                assert_eq!(entries.len(), 1 << log);
                assert_eq!(entries, defined(&counts, log), "{counts:?} at {log}");
                // Every state of a symbol leads to states of the table.
                assert!(
                    entries
                        .iter()
                        .all(|entry| u32::from(entry.base) + (1 << entry.bits) <= 1 << log)
                );
            }
        }
    }

    #[test]
    fn reads_what_it_describes() {
        // An accuracy of 5, so 32 points: 18 for symbol 0, as 19 in 5 of
        // its 6 bits, as values up to 30 are short; and 14 for symbol 1,
        // as 15 in 4 bits, low bits first.
        let bytes = u32::to_le_bytes(19 << 4 | 7 << 9 | 1 << 12);
        let (distribution, used) = Distribution::read(&bytes, 6, 2).unwrap();
        assert_eq!(used, 2);
        assert_eq!(
            (distribution.log(), distribution.counts()),
            (5, &[18, 14][..])
        );
        let table = Table::new(&distribution).unwrap();
        let states = |symbol| {
            let entries = table.entries.iter().take(1 << table.log);
            entries.filter(|e| e.symbol == symbol).count()
        };
        assert_eq!((states(0), states(1)), (18, 14));
        // Too fine, too many symbols, and too short.
        assert!(Distribution::read(&bytes, 4, 2).is_err());
        assert!(Distribution::read(&bytes, 6, 1).is_err());
        assert!(Distribution::read(&bytes[..1], 6, 2).is_err());
        // Counts that do not add up.
        assert!(Distribution::new(&[5, 2], 3).is_err());
        assert!(Distribution::new(&[-2, 10], 3).is_err());
        // Too fine for a table of weights.
        let fine = Distribution::new(&[100, 28], 7).unwrap();
        assert!(Table::new(&fine).is_err());
    }
}

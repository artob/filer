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
        while left > 0 {
            let max = left.saturating_add(1);
            let width = u32::BITS.saturating_sub(max.leading_zeros());
            let short = mask(width).saturating_sub(max);
            let low = bits.read(width.saturating_sub(1))?;
            let low = u32::try_from(low).map_err(|_| corrupt())?;
            let value = if low < short || bits.read(1)? == 0 {
                low
            } else {
                let high = 1_u32.wrapping_shl(width.saturating_sub(1));
                low.saturating_add(high).saturating_sub(short)
            };
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
                    let repeat = bits.read(2)?;
                    for _ in 0..repeat {
                        push(0)?;
                    }
                    if repeat < 3 {
                        break;
                    }
                }
            }
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
    pub(super) fn spread<'a>(&self, symbols: &'a mut [u8; MAX_STATES]) -> Result<&'a [u8], Error> {
        let size = 1_usize.wrapping_shl(u32::from(self.log)).min(MAX_STATES);
        let last = size.wrapping_sub(1);
        let mut high = size;
        for (symbol, _) in (0_u8..)
            .zip(self.counts())
            .filter(|&(_, &count)| count == -1)
        {
            high = high.checked_sub(1).ok_or_else(corrupt)?;
            *symbols.get_mut(high).ok_or_else(corrupt)? = symbol;
        }
        let step = (size >> 1).wrapping_add(size >> 3).wrapping_add(3);
        let mut at = 0_usize;
        for (symbol, &count) in (0_u8..).zip(self.counts()) {
            for _ in 0..count.max(0) {
                // Within the array, as `last` is.
                if let Some(slot) = symbols.get_mut(at & last) {
                    *slot = symbol;
                }
                at = at.wrapping_add(step) & last;
                // Some state lies below `high`: this symbol's.
                while at >= high {
                    at = at.wrapping_add(step) & last;
                }
            }
        }
        if at != 0 {
            return Err(corrupt());
        }
        symbols.get(..size).ok_or_else(corrupt)
    }

    /// Returns how each symbol's states share the next state's range.
    pub(super) fn shares(&self) -> Shares {
        let mut shares = [Share::default(); MAX_SYMBOLS];
        for (share, &count) in shares.iter_mut().zip(self.counts()) {
            // A symbol of less than 1 has one state, which reads a whole
            // state's bits.
            let count = u32::from(count.unsigned_abs());
            let Some(most) = count.checked_ilog2() else {
                continue;
            };
            let bits = u32::from(self.log).saturating_sub(most);
            let whole = 1_u32.wrapping_shl(u32::from(self.log));
            let base = count.wrapping_shl(bits).wrapping_sub(whole);
            *share = Share {
                base: u16::try_from(base).unwrap_or_default(),
                by: u16::try_from(1_u32.wrapping_shl(bits)).unwrap_or_default(),
                bits: u8::try_from(bits).unwrap_or_default(),
            };
        }
        Shares {
            shares,
            whole: 1_u16.wrapping_shl(u32::from(self.log)),
        }
    }
}

/// How a symbol's next state, in the order of its states, finds the state
/// after it: from `bits` more read and added to `base`.
///
/// A symbol of `count` states has them take shares of the next state's
/// range, as its numbers from `count` to twice that, less one, would:
/// each reads as many bits as bring its number to the table's size, and
/// adds them to what its number, shifted so, is past that size. So the
/// first of them, up to the next power of two, read a bit more than the
/// rest, their bases a step of that many bits apart, up to the range's
/// end; and the rest start over from the range's start, by half the step.
#[derive(Clone, Copy, Debug, Default)]
struct Share {
    base: u16,
    by: u16,
    bits: u8,
}

/// Each symbol's [`Share`] of a table, as states are dealt them.
#[derive(Clone, Debug)]
pub(super) struct Shares {
    shares: [Share; MAX_SYMBOLS],
    /// The table's states, which a range's end is.
    whole: u16,
}

impl Shares {
    /// Returns the bits that the next of `symbol`'s states reads, and the
    /// base it adds them to: the states' in order, one each time.
    #[inline(always)]
    pub(super) fn take(&mut self, symbol: u8) -> (u8, u16) {
        let Some(share) = self.shares.get_mut(usize::from(symbol)) else {
            return (0, 0);
        };
        let taken = (share.bits, share.base);
        share.base = share.base.wrapping_add(share.by);
        if share.base == self.whole {
            *share = Share {
                base: 0,
                by: share.by >> 1,
                bits: share.bits.wrapping_sub(1),
            };
        }
        taken
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
        let mut shares = distribution.shares();
        let mut entries = [Entry::default(); Self::STATES];
        for (entry, &symbol) in entries.iter_mut().zip(states) {
            let (bits, base) = shares.take(symbol);
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

    /// Each state's entry, as RFC 8878 §4.1.1 defines it: a symbol's
    /// states numbered from its count, in their order, each reading as
    /// many bits as bring its number to the table's size.
    fn defined(counts: &[i16], log: u8) -> Vec<Entry> {
        let distribution = Distribution::new(counts, log).unwrap();
        let mut symbols = [0; MAX_STATES];
        let states = distribution.spread(&mut symbols).unwrap();
        let mut numbers: Vec<u32> = counts
            .iter()
            .map(|count| count.unsigned_abs().into())
            .collect();
        let entries = states.iter().map(|&symbol| {
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

    /// The entries that shares deal out, each state's in order.
    fn dealt(counts: &[i16], log: u8) -> Vec<Entry> {
        let distribution = Distribution::new(counts, log).unwrap();
        let mut symbols = [0; MAX_STATES];
        let states = distribution.spread(&mut symbols).unwrap();
        let mut shares = distribution.shares();
        let entries = states.iter().map(|&symbol| {
            let (bits, base) = shares.take(symbol);
            Entry { symbol, bits, base }
        });
        entries.collect()
    }

    /// RFC 8878 §4.1.1's example, a symbol of 5 states of 128: their
    /// bits and bases, from Table 21, in the order of the states.
    #[test]
    fn shares_states_as_the_rfc_does() {
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
    fn deals_shares_as_states_are_numbered() {
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

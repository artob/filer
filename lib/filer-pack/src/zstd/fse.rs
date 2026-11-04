// This is free and unencumbered software released into the public domain.

//! Finite State Entropy (RFC 8878 §4.1): the decoding tables of zstd's
//! sequences and Huffman weights, from the distributions that describe
//! them.

use super::{
    bits::{Backward, Forward},
    corrupt,
};
use alloc::{vec, vec::Vec};
use filer_api::Error;

/// A state of a decoding table: the symbol it decodes, and how to find
/// the next, from `bits` more read and added to `base`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Entry {
    pub(super) symbol: u8,
    pub(super) bits: u8,
    pub(super) base: u16,
}

/// A decoding table: one entry per state, `1 << log` of them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Table {
    log: u8,
    entries: Vec<Entry>,
}

impl Table {
    /// Reads a distribution's description (RFC 8878 §4.1.1) from the start
    /// of `bytes`, of an accuracy of at most `max_log` and at most
    /// `symbols` symbols, and returns its table and the bytes it took.
    pub(super) fn read(bytes: &[u8], max_log: u8, symbols: usize) -> Result<(Self, usize), Error> {
        let mut bits = Forward::new(bytes);
        let log = u8::try_from(bits.read(4)?).map_err(|_| corrupt())?;
        let log = log.saturating_add(5);
        if log > max_log {
            return Err(corrupt());
        }
        // The probability points left to distribute; each symbol's is
        // read in as few bits as the values it may still take need.
        let mut left = 1_u32.checked_shl(u32::from(log)).ok_or_else(corrupt)?;
        let mut counts = Vec::with_capacity(symbols);
        while left > 0 {
            if counts.len() >= symbols {
                return Err(corrupt());
            }
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
            counts.push(count);
            // Zeros are followed by how many more follow, in twos.
            if count == 0 {
                loop {
                    let repeat = bits.read(2)?;
                    for _ in 0..repeat {
                        if counts.len() >= symbols {
                            return Err(corrupt());
                        }
                        counts.push(0);
                    }
                    if repeat < 3 {
                        break;
                    }
                }
            }
        }
        Ok((Self::new(&counts, log)?, bits.bytes()))
    }

    /// Returns the table of a distribution: each symbol's count of the
    /// `1 << log` states, or -1 for "less than 1", which take one.
    pub(super) fn new(counts: &[i16], log: u8) -> Result<Self, Error> {
        let whole = 1_u32.checked_shl(u32::from(log)).ok_or_else(corrupt)?;
        let total = counts.iter().try_fold(0_u32, |total, &count| match count {
            -1.. => total.checked_add(u32::from(count.unsigned_abs())),
            _ => None,
        });
        if total != Some(whole) {
            return Err(corrupt());
        }
        let size = usize::try_from(whole).map_err(|_| corrupt())?;
        let mut entries = vec![Entry::default(); size];
        // Symbols of less than 1 take the last states, from the end.
        let mut high = size;
        for (symbol, _) in counts.iter().enumerate().filter(|(_, count)| **count == -1) {
            high = high.checked_sub(1).ok_or_else(corrupt)?;
            let entry = entries.get_mut(high).ok_or_else(corrupt)?;
            entry.symbol = u8::try_from(symbol).map_err(|_| corrupt())?;
        }
        // The others are spread over the rest, by a step that visits each
        // state once.
        let step = (size >> 1).saturating_add(size >> 3).saturating_add(3);
        let mut at = 0_usize;
        for (symbol, &count) in counts.iter().enumerate() {
            let symbol = u8::try_from(symbol).map_err(|_| corrupt())?;
            for _ in 0..count.max(0) {
                entries.get_mut(at).ok_or_else(corrupt)?.symbol = symbol;
                loop {
                    at = at.saturating_add(step) & size.saturating_sub(1);
                    if at < high {
                        break;
                    }
                }
            }
        }
        if at != 0 {
            return Err(corrupt());
        }
        // Each symbol's states, in order, take shares of the next state's
        // range: the first, twice those of the others, a bit more.
        let mut next: Vec<u32> = counts
            .iter()
            .map(|&count| u32::from(count.unsigned_abs()))
            .collect();
        for entry in &mut entries {
            let x = next
                .get_mut(usize::from(entry.symbol))
                .ok_or_else(corrupt)?;
            let bits = u32::from(log).saturating_sub(x.checked_ilog2().ok_or_else(corrupt)?);
            let base = x.checked_shl(bits).ok_or_else(corrupt)?;
            entry.bits = u8::try_from(bits).map_err(|_| corrupt())?;
            entry.base = u16::try_from(base.saturating_sub(whole)).map_err(|_| corrupt())?;
            *x = x.saturating_add(1);
        }
        Ok(Self { log, entries })
    }

    /// Returns the table of a single symbol, which every state decodes.
    pub(super) fn rle(symbol: u8) -> Self {
        let entry = Entry {
            symbol,
            bits: 0,
            base: 0,
        };
        Self {
            log: 0,
            entries: vec![entry],
        }
    }

    /// Returns the state that `bits` begin with.
    pub(super) fn start(&self, bits: &mut Backward<'_>) -> usize {
        usize::try_from(bits.read(u32::from(self.log))).unwrap_or_default()
    }

    /// Returns the entry of `state`.
    pub(super) fn entry(&self, state: usize) -> Entry {
        self.entries.get(state).copied().unwrap_or_default()
    }

    /// Returns the state after `entry`'s, from `bits`.
    pub(super) fn next(entry: Entry, bits: &mut Backward<'_>) -> usize {
        let more = usize::try_from(bits.read(u32::from(entry.bits))).unwrap_or_default();
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

    /// RFC 8878 §4.1.1's example, a symbol of 5 states of 128: their
    /// bits and bases, from Table 21, in the order of the states.
    #[test]
    fn shares_states_as_the_rfc_does() {
        let table = Table::new(&[5, 123], 7).unwrap();
        let fives: Vec<_> = table
            .entries
            .iter()
            .filter(|entry| entry.symbol == 0)
            .map(|entry| (entry.bits, entry.base))
            .collect();
        assert_eq!(fives, [(5, 32), (5, 64), (5, 96), (4, 0), (4, 16)]);
    }

    #[test]
    fn reads_what_it_describes() {
        // An accuracy of 5, so 32 points: 18 for symbol 0, as 19 in 5 of
        // its 6 bits, as values up to 30 are short; and 14 for symbol 1,
        // as 15 in 4 bits, low bits first.
        let bytes = u32::to_le_bytes(19 << 4 | 7 << 9 | 1 << 12);
        let (table, used) = Table::read(&bytes, 6, 2).unwrap();
        assert_eq!(used, 2);
        let states = |symbol| table.entries.iter().filter(|e| e.symbol == symbol).count();
        assert_eq!((states(0), states(1)), (18, 14));
        // Too fine, too many symbols, and too short.
        assert!(Table::read(&bytes, 4, 2).is_err());
        assert!(Table::read(&bytes, 6, 1).is_err());
        assert!(Table::read(&bytes[..1], 6, 2).is_err());
        // Counts that do not add up.
        assert!(Table::new(&[5, 2], 3).is_err());
        assert!(Table::new(&[-2, 10], 3).is_err());
    }
}

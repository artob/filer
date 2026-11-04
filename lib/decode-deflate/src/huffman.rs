// This is free and unencumbered software released into the public domain.

//! Deflate's Huffman codes (RFC 1951 §3.2.2): canonical codes from their
//! lengths, decoded by a table of the next few bits and, for longer codes,
//! a table under each of their prefixes.

use crate::{Error, corrupt};
use alloc::{vec, vec::Vec};

/// The longest a code may be.
pub(crate) const MAX_BITS: u32 = 15;

/// The lengths a code may have, from 0, none, to the longest.
const LENGTHS: usize = 16;

/// What a table holds for a run of bits, packed for speed: the symbol, or
/// where a table of longer codes begins, above a length byte, which is the
/// code's length; or `LINK` and the bits that index that table; or
/// `INVALID`, for bits that no code begins with. Neither of those is a
/// length that the bits a decoder holds reach, so that one comparison
/// tells a code decoded from the rest.
type Entry = u32;

/// The length byte of an entry of no code.
const INVALID: u32 = 0x7F;

/// The flag in the length byte of an entry that links to a table of longer
/// codes.
const LINK: u32 = 0x80;

/// Returns the entry of `value` and the length byte `bits`.
fn entry(value: u16, bits: u32) -> Entry {
    u32::from(value).wrapping_shl(16) | bits
}

/// A decoded symbol, and the length of its code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Code {
    Symbol {
        symbol: u16,
        bits: u32,
    },
    /// Bits that begin no code, at least as many as the table can tell.
    Invalid,
    /// Too few bits to tell: the code may need more.
    Short,
}

/// Whether a code's lengths must use every code, as zlib requires of all
/// but the literal/length and distance codes: those may leave codes unused
/// when they have a single code of one bit, or, for distances, none.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Lengths,
    Literals,
    Distances,
}

/// A decoding table: `1 << bits` entries indexed by the next bits of the
/// stream, the first read in the lowest, then the tables of longer codes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Table {
    bits: u32,
    /// The low `bits` bits set.
    mask: u64,
    entries: Vec<Entry>,
}

impl Table {
    /// Returns the table of the codes that `lengths`, each symbol's in turn,
    /// define, indexed by `bits` bits before its tables of longer codes.
    pub(crate) fn new(lengths: &[u8], bits: u32, kind: Kind) -> Result<Self, Error> {
        let mut counts = [0_u32; LENGTHS];
        for &len in lengths {
            let count = counts.get_mut(usize::from(len)).ok_or_else(corrupt)?;
            *count = count.saturating_add(1);
        }
        // The codes left at each length, after those shorter take theirs:
        // none may be over-subscribed, and only some may leave any.
        let mut left = 1_i64;
        for &count in counts.iter().skip(1) {
            left = left.saturating_mul(2).saturating_sub(i64::from(count));
            if left < 0 {
                return Err(corrupt());
            }
        }
        let used: u32 = counts.iter().skip(1).sum();
        let single = used == 1 && counts.get(1) == Some(&1);
        let allowed = match kind {
            Kind::Lengths => false,
            Kind::Literals => single,
            Kind::Distances => single || used == 0,
        };
        if left > 0 && !allowed {
            return Err(corrupt());
        }

        // The first code of each length (RFC 1951 §3.2.2, step 2).
        let mut next = [0_u32; LENGTHS];
        let mut code = 0_u32;
        for len in 1..LENGTHS {
            let shorter = counts
                .get(len.saturating_sub(1))
                .copied()
                .unwrap_or_default();
            code = code
                .saturating_add(if len == 1 { 0 } else { shorter })
                .wrapping_shl(1);
            if let Some(next) = next.get_mut(len) {
                *next = code;
            }
        }
        // Each code, read from the stream first bit first, so reversed.
        let mut codes = Vec::with_capacity(lengths.len());
        for (symbol, &len) in lengths.iter().enumerate().filter(|(_, len)| **len > 0) {
            let next = next.get_mut(usize::from(len)).ok_or_else(corrupt)?;
            let code = u16::try_from(*next).map_err(|_| corrupt())?;
            *next = next.saturating_add(1);
            let reversed = code.reverse_bits() >> 16_u32.saturating_sub(u32::from(len));
            let symbol = u16::try_from(symbol).map_err(|_| corrupt())?;
            codes.push((symbol, u32::from(len), usize::from(reversed)));
        }

        // Codes no longer than the index fill every entry they begin.
        let size = 1_usize.wrapping_shl(bits);
        let mask = size.wrapping_sub(1);
        let mut entries = vec![INVALID; size];
        for &(symbol, len, reversed) in codes.iter().filter(|(_, len, _)| *len <= bits) {
            let step = 1_usize.wrapping_shl(len);
            for slot in entries.iter_mut().skip(reversed).step_by(step) {
                *slot = entry(symbol, len);
            }
        }
        // Longer ones, a table under their prefix, as long as the longest
        // code under it, past the prefix, needs.
        let long = || codes.iter().filter(|(_, len, _)| *len > bits);
        let mut links = vec![0_u32; size];
        for &(_, len, reversed) in long() {
            let link = links.get_mut(reversed & mask).ok_or_else(corrupt)?;
            *link = (*link).max(len.saturating_sub(bits));
        }
        for (prefix, &link) in links.iter().enumerate().filter(|(_, link)| **link > 0) {
            let offset = u16::try_from(entries.len()).map_err(|_| corrupt())?;
            *entries.get_mut(prefix).ok_or_else(corrupt)? = entry(offset, LINK | link);
            let sub = 1_usize.wrapping_shl(link);
            entries.resize(entries.len().saturating_add(sub), INVALID);
        }
        for &(symbol, len, reversed) in long() {
            let link = links.get(reversed & mask).copied().ok_or_else(corrupt)?;
            let at = entries.get(reversed & mask).copied().ok_or_else(corrupt)?;
            let offset = usize::try_from(at >> 16).map_err(|_| corrupt())?;
            let start = offset.saturating_add(reversed >> bits);
            let end = offset.saturating_add(1_usize.wrapping_shl(link));
            let step = 1_usize.wrapping_shl(len.saturating_sub(bits));
            let sub = entries.get_mut(start..end).unwrap_or_default();
            for slot in sub.iter_mut().step_by(step) {
                *slot = entry(symbol, len);
            }
        }
        Ok(Self {
            bits,
            mask: 1_u64.wrapping_shl(bits).wrapping_sub(1),
            entries,
        })
    }

    /// Decodes the code that `value`, the next `count` bits of the stream,
    /// begins with, the first in its lowest bit.
    #[inline]
    pub(crate) fn lookup(&self, value: u64, count: u32) -> Code {
        let index = usize::try_from(value & self.mask).unwrap_or_default();
        let entry = self.entries.get(index).copied().unwrap_or(INVALID);
        if entry & 0xFF <= count {
            return symbol(entry);
        }
        self.slow(entry, value, count)
    }

    /// Decodes a code longer than the table's index, or one the bits do
    /// not yet hold, or none.
    #[cold]
    fn slow(&self, mut entry: Entry, value: u64, count: u32) -> Code {
        if entry & LINK != 0 {
            let link = entry & !LINK & 0xFF;
            let sub = 1_u64.wrapping_shl(link).wrapping_sub(1);
            let index = usize::try_from(value.wrapping_shr(self.bits) & sub).unwrap_or_default();
            let offset = usize::try_from(entry >> 16).unwrap_or_default();
            entry = self
                .entries
                .get(offset.saturating_add(index))
                .copied()
                .unwrap_or(INVALID);
            if entry & 0xFF <= count {
                return symbol(entry);
            }
        }
        // Past the bits there are, a longer code may yet begin.
        if entry & 0xFF == INVALID && count >= MAX_BITS {
            Code::Invalid
        } else {
            Code::Short
        }
    }
}

/// Returns the symbol and code length of an entry that has them.
fn symbol(entry: Entry) -> Code {
    Code::Symbol {
        symbol: u16::try_from(entry >> 16).unwrap_or_default(),
        bits: entry & 0xFF,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RFC 1951 §3.2.2's example: lengths (3, 3, 3, 3, 3, 2, 4, 4) give
    /// codes 010, 011, 100, 101, 110, 00, 1110, and 1111, read from the
    /// first bit, which the stream holds in the lowest.
    #[test]
    fn assigns_codes_as_the_rfc_does() {
        for bits in [2, 4, 9] {
            let table = Table::new(&[3, 3, 3, 3, 3, 2, 4, 4], bits, Kind::Lengths).unwrap();
            for (symbol, code, len) in [
                (0, 0b010, 3),
                (1, 0b011, 3),
                (2, 0b100, 3),
                (3, 0b101, 3),
                (4, 0b110, 3),
                (5, 0b00, 2),
                (6, 0b1110, 4),
                (7, 0b1111, 4),
            ] {
                let stream = u64::from(u16::reverse_bits(code) >> (16 - len));
                let decoded = table.lookup(stream, 15);
                assert_eq!(
                    decoded,
                    Code::Symbol { symbol, bits: len },
                    "{bits}: {symbol}"
                );
                assert_eq!(table.lookup(stream, len - 1), Code::Short);
            }
        }
    }

    #[test]
    fn refuses_codes_that_make_no_tree() {
        // Over-subscribed, and incomplete where that is not allowed.
        assert!(Table::new(&[1, 1, 1], 9, Kind::Literals).is_err());
        assert!(Table::new(&[1, 2], 9, Kind::Lengths).is_err());
        assert!(Table::new(&[2, 2, 2], 9, Kind::Literals).is_err());
        // A single code of one bit, and no distance codes at all.
        let single = Table::new(&[0, 1], 9, Kind::Distances).unwrap();
        assert_eq!(single.lookup(0, 15), Code::Symbol { symbol: 1, bits: 1 });
        assert_eq!(single.lookup(1, 15), Code::Invalid);
        let none = Table::new(&[0, 0], 9, Kind::Distances).unwrap();
        assert_eq!(none.lookup(0, 15), Code::Invalid);
        assert!(Table::new(&[0, 0], 9, Kind::Literals).is_err());
    }
}

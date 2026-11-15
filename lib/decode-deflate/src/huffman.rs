// This is free and unencumbered software released into the public domain.

//! Deflate's Huffman codes (RFC 1951 §3.2.2): canonical codes from their
//! lengths, decoded by a table of the next few bits and, for longer codes,
//! a table under each of their prefixes.

use crate::{Error, corrupt};
use alloc::{borrow::Cow, vec, vec::Vec};

/// The longest a code may be.
pub(crate) const MAX_BITS: u32 = 15;

/// The lengths a code may have, from 0, none, to the longest.
const LENGTHS: usize = 16;

/// The most symbols a code may have: the fixed literal/length code's 288.
const SYMBOLS: usize = 288;

/// The most bits a table's index may have.
const MAX_INDEX: u32 = 10;

/// Each byte with its bits in the other order.
#[allow(
    clippy::as_conversions,
    clippy::indexing_slicing,
    reason = "built at compile time, of bytes below 256 alone"
)]
const REVERSED: [u8; 256] = {
    let mut reversed = [0_u8; 256];
    let mut byte = 0;
    while byte < 256 {
        reversed[byte] = (byte as u8).reverse_bits();
        byte += 1;
    }
    reversed
};

/// Returns the low `len` bits of `code`, 15 at most, in the other order,
/// by a table of bytes, as x86-64 has no instruction for it.
fn reverse(code: u32, len: u32) -> usize {
    let [low, high, _, _] = code.to_le_bytes();
    let low = REVERSED.get(usize::from(low)).copied().unwrap_or_default();
    let high = REVERSED.get(usize::from(high)).copied().unwrap_or_default();
    usize::from(u16::from_le_bytes([high, low])).wrapping_shr(16_u32.saturating_sub(len))
}

/// What a table holds for a run of bits, packed for speed: the symbol's
/// meaning, a value, such as a literal, or a length's base, or where a
/// table of longer codes begins; above a kind byte, a literal's flags, or
/// else its code's length; above a length byte, which is how many bits the
/// symbol takes, its code's and the extra bits', 31 at most;
/// or `LINK` and the bits that index that table; or `END` and the length
/// of the code that ends a block; or `INVALID`, for bits that no code
/// begins with, or that begin a code of a symbol that never occurs. None
/// of those is a length that the bits a decoder holds reach, and each sets
/// a bit of `SPECIAL`, which no symbol's length does, so that one test
/// tells a symbol's entry from the rest; and a literal's kind byte flags
/// it, so that one test tells a literal.
pub(crate) type Entry = u32;

/// The kinds of literal/length and distance symbols that the table's maker
/// gives, beside how many extra bits follow a length's or a distance's
/// base, 16 at most: a literal, the block's end, and a symbol that never
/// occurs. Only `LITERAL` stays in an entry's kind byte.
pub(crate) const LITERAL: u8 = 0x40;
pub(crate) const END: u8 = 0x20;
pub(crate) const NEVER: u8 = 0x80;

/// The flag of a literal's entry, and of an entry of two literals, the
/// second in the value's high byte, beside it.
pub(crate) const LITERAL_ENTRY: Entry = 0x4000;
pub(crate) const DOUBLE_ENTRY: Entry = 0x2000;
const DOUBLE: u8 = 0x20;

/// The bits of the length byte that only entries of no symbol set.
pub(crate) const SPECIAL: Entry = 0xE0;

/// The length byte of an entry of no code, or of a symbol that never
/// occurs.
pub(crate) const INVALID: u32 = 0x7F;

/// The flag in the length byte of an entry that links to a table of longer
/// codes.
pub(crate) const LINK: u32 = 0x80;

/// The flag in the length byte of the entry of the code that ends a block,
/// beside its length.
pub(crate) const END_ENTRY: u32 = 0x40;

/// Returns the entry of `value`, `kind`, and the length byte `bits`.
fn entry(value: u16, kind: u8, bits: u32) -> Entry {
    u32::from(value).wrapping_shl(16) | u32::from(kind).wrapping_shl(8) | bits
}

/// Returns an entry's value: a literal, or a length's or a distance's base.
#[inline(always)]
pub(crate) fn value(entry: Entry) -> u32 {
    entry >> 16
}

/// Returns the length of the code of a length or a distance whose entry
/// this is, or of a symbol other than a literal.
#[inline(always)]
pub(crate) fn code(entry: Entry) -> u32 {
    entry.wrapping_shr(8) & 0x1F
}

/// Returns an entry's length byte: how many bits its symbol takes, its
/// code's and the extra bits', if it is a symbol's.
#[inline(always)]
pub(crate) fn length(entry: Entry) -> u32 {
    entry & 0xFF
}

/// Returns what the symbol of `entry` means, a length's or a distance's
/// base and the extra bits after its code in `bits`, the next bits of the
/// stream, as a number: its length, or its distance. The bits it takes,
/// below its length byte, past those of its code, below its kind byte,
/// are the extra bits; each byte as a shift's count, which takes its low
/// bits alone, the rest of the entry above them, as BMI2's `bzhi` and
/// `shrx` do.
#[inline(always)]
pub(crate) fn meaning(entry: Entry, bits: u64) -> u64 {
    let taken = bits & !u64::MAX.wrapping_shl(entry);
    u64::from(value(entry)).wrapping_add(taken.wrapping_shr(entry.wrapping_shr(8)))
}

/// Returns the entry of `entries`, a table's first, that `value`, the next
/// bits of the stream, indexes: masked to their count, a power of two, so
/// that it takes no bounds check.
#[inline(always)]
pub(crate) fn at<const SIZE: usize>(entries: &[Entry; SIZE], value: u64) -> Entry {
    let mask = u64::try_from(SIZE).unwrap_or_default().wrapping_sub(1);
    let index = usize::try_from(value & mask).unwrap_or_default();
    entries.get(index).copied().unwrap_or(INVALID)
}

/// Whether an entry is that of the code that ends a block.
pub(crate) fn ends(entry: Entry) -> bool {
    entry & SPECIAL == END_ENTRY && entry & 0x1F <= MAX_BITS
}

/// Returns the length of the code of an entry for which [`ends`] holds.
pub(crate) fn end_length(entry: Entry) -> u32 {
    entry & 0x1F
}

/// A decoded symbol's meaning, and the length of its code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Code {
    Symbol {
        value: u32,
        kind: u32,
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
    /// Its own, or the fixed codes', made at compile time.
    entries: Cow<'static, [Entry]>,
}

impl Table {
    /// Returns the table of the codes that `lengths`, each symbol's in turn,
    /// define, indexed by `bits` bits before its tables of longer codes,
    /// each holding what `meaning` says its symbol means, as a value and a
    /// kind.
    pub(crate) fn new(
        lengths: &[u8],
        bits: u32,
        kind: Kind,
        meaning: impl Fn(u16) -> (u16, u8),
    ) -> Result<Self, Error> {
        // Counted four ways at once, each a count of every fourth length,
        // as a count waits on the one before of the same length. A length
        // past the longest, which none counts, has no place among the
        // symbols sorted below.
        let mut four = [[0_u32; LENGTHS]; 4];
        let (quads, rest) = lengths.as_chunks::<4>();
        for quad in quads {
            for (counts, &len) in four.iter_mut().zip(quad) {
                if let Some(count) = counts.get_mut(usize::from(len & 0xF)) {
                    *count = count.wrapping_add(1);
                }
            }
        }
        for (counts, &len) in four.iter_mut().zip(rest) {
            if let Some(count) = counts.get_mut(usize::from(len & 0xF)) {
                *count = count.wrapping_add(1);
            }
        }
        let mut counts = [0_u32; LENGTHS];
        for (len, count) in counts.iter_mut().enumerate() {
            *count = four
                .iter()
                .map(|counts| counts.get(len).copied().unwrap_or_default())
                .sum();
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

        // The first code of each length (RFC 1951 §3.2.2, step 2), and
        // where each length's symbols begin in `sorted`.
        let mut next = [0_u32; LENGTHS];
        let mut starts = [0_usize; LENGTHS];
        let (mut code, mut start) = (0_u32, 0_usize);
        for len in 1..LENGTHS {
            let shorter = counts
                .get(len.saturating_sub(1))
                .copied()
                .unwrap_or_default();
            code = code
                .saturating_add(if len == 1 { 0 } else { shorter })
                .wrapping_shl(1);
            if let (Some(next), Some(at)) = (next.get_mut(len), starts.get_mut(len)) {
                (*next, *at) = (code, start);
            }
            let count = counts.get(len).copied().unwrap_or_default();
            start = start.saturating_add(usize::try_from(count).unwrap_or_default());
        }
        // The symbols by their codes' lengths, then by their own order, so
        // that each length's codes come in turn (step 3).
        let mut sorted = [0_u16; SYMBOLS];
        let mut place = starts;
        for (symbol, &len) in lengths.iter().enumerate().filter(|(_, len)| **len > 0) {
            let at = place.get_mut(usize::from(len)).ok_or_else(corrupt)?;
            *sorted.get_mut(*at).ok_or_else(corrupt)? =
                u16::try_from(symbol).map_err(|_| corrupt())?;
            *at = at.saturating_add(1);
        }
        let used = usize::try_from(used).map_err(|_| corrupt())?;
        let sorted = sorted.get(..used).ok_or_else(corrupt)?;
        // Codes no longer than the index fill every entry they begin: each
        // length's codes are set in a table as wide as it, which is doubled
        // for the next, its entries repeated, as the bits past a code's end
        // tell nothing of it. Longer ones wait for their tables.
        let size = 1_usize.wrapping_shl(bits);
        let mask = size.wrapping_sub(1);
        let mut entries = vec![INVALID; size];
        let mut width = 2_usize;
        let mut long = Vec::new();
        // Literals short enough to pair, with their codes' reversals.
        let mut short = [(0_usize, 0_u32, 0_u8); 256];
        let mut shorts = 0_usize;
        for &symbol in sorted {
            let len = lengths
                .get(usize::from(symbol))
                .copied()
                .ok_or_else(corrupt)?;
            let next = next.get_mut(usize::from(len)).ok_or_else(corrupt)?;
            // Read from the stream first bit first, so reversed.
            let reversed = reverse(*next, u32::from(len));
            *next = next.saturating_add(1);
            let len = u32::from(len);
            // What fills each entry of the code: of `END` and `NEVER`, the
            // length byte alone; of the rest, the code's length and the
            // extra bits' as the length byte.
            let meant = match meaning(symbol) {
                (_, NEVER) => INVALID,
                (_, END) => END_ENTRY | len,
                (value, LITERAL) => entry(value, LITERAL, len),
                (value, extra) => {
                    let code = u8::try_from(len).map_err(|_| corrupt())?;
                    entry(value, code, len.saturating_add(u32::from(extra & 0x1F)))
                },
            };
            if len > bits {
                long.push((meant, len, reversed));
                continue;
            }
            if let (Kind::Literals, Ok(byte), Some(slot)) =
                (kind, u8::try_from(symbol), short.get_mut(shorts))
            {
                *slot = (reversed, len, byte);
                shorts = shorts.saturating_add(1);
            }
            let wide = 1_usize.wrapping_shl(len);
            while width < wide {
                entries.copy_within(..width, width);
                width = width.saturating_mul(2);
            }
            *entries.get_mut(reversed).ok_or_else(corrupt)? = meant;
        }
        while width < size {
            entries.copy_within(..width, width);
            width = width.saturating_mul(2);
        }
        // Longer ones, a table under their prefix, as long as the longest
        // code under it, past the prefix, needs.
        let mut links = [0_u8; 1 << MAX_INDEX];
        for &(_, len, reversed) in &long {
            let link = links.get_mut(reversed & mask).ok_or_else(corrupt)?;
            *link = (*link).max(u8::try_from(len.saturating_sub(bits)).map_err(|_| corrupt())?);
        }
        for &(_, _, reversed) in &long {
            let prefix = reversed & mask;
            let link = u32::from(links.get(prefix).copied().unwrap_or_default());
            let slot = entries.get(prefix).copied().ok_or_else(corrupt)?;
            if slot & LINK == 0 {
                let offset = u16::try_from(entries.len()).map_err(|_| corrupt())?;
                *entries.get_mut(prefix).ok_or_else(corrupt)? = entry(offset, 0, LINK | link);
                let sub = 1_usize.wrapping_shl(link);
                entries.resize(entries.len().saturating_add(sub), INVALID);
            }
        }
        for &(meant, len, reversed) in &long {
            let link = u32::from(links.get(reversed & mask).copied().unwrap_or_default());
            let at = entries.get(reversed & mask).copied().ok_or_else(corrupt)?;
            let offset = usize::try_from(at >> 16).map_err(|_| corrupt())?;
            let start = offset.saturating_add(reversed >> bits);
            let end = offset.saturating_add(1_usize.wrapping_shl(link));
            let step = 1_usize.wrapping_shl(len.saturating_sub(bits));
            let sub = entries.get_mut(start..end).unwrap_or_default();
            for slot in sub.iter_mut().step_by(step) {
                *slot = meant;
            }
        }
        if let Some(primary) = entries.first_chunk_mut::<{ 1 << MAX_INDEX }>()
            && kind == Kind::Literals
            && bits == MAX_INDEX
        {
            pair(primary, short.get(..shorts).unwrap_or_default());
        }
        Ok(Self {
            bits,
            mask: 1_u64.wrapping_shl(bits).wrapping_sub(1),
            entries: Cow::Owned(entries),
        })
    }

    /// Returns the table of `entries`, made at compile time, indexed by
    /// `bits` bits, with no tables of longer codes.
    pub(crate) fn fixed(entries: &'static [Entry], bits: u32) -> Self {
        Self {
            bits,
            mask: 1_u64.wrapping_shl(bits).wrapping_sub(1),
            entries: Cow::Borrowed(entries),
        }
    }

    /// Returns the entries that the table's index reaches, `SIZE` of them,
    /// as many as its index bits reach, so that a caller who masks what it
    /// indexes them with takes no bounds check; or `None` for a table of
    /// another size, or none.
    #[inline]
    pub(crate) fn primary<const SIZE: usize>(&self) -> Option<&[Entry; SIZE]> {
        let size = 1_usize.wrapping_shl(self.bits);
        self.entries.first_chunk().filter(|_| size == SIZE)
    }

    /// Returns the entry, in the table of longer codes that `entry` links
    /// to, of the code that `value`, the next bits of the stream, begins
    /// with, or one of no code.
    #[inline]
    pub(crate) fn sub(&self, entry: Entry, value: u64) -> Entry {
        let link = entry & !LINK & 0xFF;
        let sub = 1_u64.wrapping_shl(link).wrapping_sub(1);
        let index = usize::try_from(value.wrapping_shr(self.bits) & sub).unwrap_or_default();
        let offset = usize::try_from(entry >> 16).unwrap_or_default();
        self.entries
            .get(offset.saturating_add(index))
            .copied()
            .unwrap_or(INVALID)
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
    /// not yet hold, or the block's end, or none.
    #[cold]
    fn slow(&self, mut entry: Entry, value: u64, count: u32) -> Code {
        if entry & LINK != 0 {
            entry = self.sub(entry, value);
            if entry & 0xFF <= count {
                return symbol(entry);
            }
        }
        if ends(entry) {
            let bits = end_length(entry);
            return match bits <= count {
                true => Code::Symbol {
                    value: 0,
                    kind: u32::from(END),
                    bits,
                },
                false => Code::Short,
            };
        }
        // Past the bits there are, a longer code may yet begin.
        if entry & 0xFF == INVALID && count >= MAX_BITS {
            Code::Invalid
        } else {
            Code::Short
        }
    }
}

/// Makes each entry of a literal whose code, and the next literal's after
/// it, fit the index of `entries` the entry of both, so that one lookup
/// decodes two literals: for each of the literals `short`, as `(reversed
/// code, length, byte)`, in order of length, those its code begins, whose
/// next bits index the entry of the second as they index the table, read
/// from it as it was.
fn pair(entries: &mut [Entry; 1 << MAX_INDEX], short: &[(usize, u32, u8)]) {
    let Some(&(_, shortest, _)) = short.first() else {
        return;
    };
    let seconds = *entries;
    for &(reversed, len, byte) in short {
        let room = MAX_INDEX.saturating_sub(len);
        if shortest > room {
            break;
        }
        let first = entry(u16::from(byte), LITERAL, len);
        let next = 1_usize.wrapping_shl(room);
        for (bits, &second) in seconds.iter().take(next).enumerate() {
            let both = len.wrapping_add(length(second));
            let single = second & (LITERAL_ENTRY | DOUBLE_ENTRY) == LITERAL_ENTRY;
            let [_, _, second_byte, _] = second.to_le_bytes();
            let paired = entry(
                u16::from_le_bytes([byte, second_byte]),
                LITERAL | DOUBLE,
                both,
            );
            let index = reversed | bits.wrapping_shl(len);
            if let Some(slot) = entries.get_mut(index & ((1 << MAX_INDEX) - 1)) {
                *slot = if single && both <= MAX_INDEX {
                    paired
                } else {
                    first
                };
            }
        }
    }
}

/// Returns the meaning and code length of an entry that has them.
fn symbol(entry: Entry) -> Code {
    let (kind, bits) = match entry & LITERAL_ENTRY {
        0 => (length(entry).saturating_sub(code(entry)), code(entry)),
        _ => (entry.wrapping_shr(8) & 0xFF, length(entry)),
    };
    Code::Symbol {
        value: entry >> 16,
        kind,
        bits,
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
            let table = Table::new(&[3, 3, 3, 3, 3, 2, 4, 4], bits, Kind::Lengths, same).unwrap();
            for (symbol, code, len) in [
                (0_u32, 0b010, 3),
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
                let (value, kind) = (symbol, 0);
                assert_eq!(
                    decoded,
                    Code::Symbol {
                        value,
                        kind,
                        bits: len
                    },
                    "{bits}: {symbol}"
                );
                assert_eq!(table.lookup(stream, len - 1), Code::Short);
            }
        }
    }

    #[test]
    fn refuses_codes_that_make_no_tree() {
        // Over-subscribed, and incomplete where that is not allowed.
        assert!(Table::new(&[1, 1, 1], 9, Kind::Literals, same).is_err());
        assert!(Table::new(&[1, 2], 9, Kind::Lengths, same).is_err());
        assert!(Table::new(&[2, 2, 2], 9, Kind::Literals, same).is_err());
        // A single code of one bit, and no distance codes at all.
        let single = Table::new(&[0, 1], 9, Kind::Distances, same).unwrap();
        let one = Code::Symbol {
            value: 1,
            kind: 0,
            bits: 1,
        };
        assert_eq!(single.lookup(0, 15), one);
        assert_eq!(single.lookup(1, 15), Code::Invalid);
        let none = Table::new(&[0, 0], 9, Kind::Distances, same).unwrap();
        assert_eq!(none.lookup(0, 15), Code::Invalid);
        assert!(Table::new(&[0, 0], 9, Kind::Literals, same).is_err());
    }

    /// Each symbol as itself.
    fn same(symbol: u16) -> (u16, u8) {
        (symbol, 0)
    }
}

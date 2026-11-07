// This is free and unencumbered software released into the public domain.

//! Huffman coding (RFC 8878 §4.2), of literals: the prefix codes that a
//! tree's description gives each byte, and the streams coded with them.

use super::{bits::Backward, corrupt, fse::Table};
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

/// A decoding table, indexed by a stream's next `bits` bits, the length of
/// the longest code: held in an array of the most any table takes, so
/// that it is never allocated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Huffman {
    bits: u8,
    codes: [Code; MAX_CODES],
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

    /// Decodes `stream` into `output`, which it must fill exactly.
    pub(super) fn decode(&self, stream: &[u8], output: &mut [u8]) -> Result<(), Error> {
        let mut bits = Backward::new(stream)?;
        self.rest(&mut bits, output);
        if bits.done() { Ok(()) } else { Err(corrupt()) }
    }

    /// Decodes four streams into four outputs, which each must fill
    /// exactly, in turn, so that the processor looks up each one's codes
    /// while it looks up the others'.
    pub(super) fn decode4(
        &self,
        streams: [&[u8]; 4],
        outputs: [&mut [u8]; 4],
    ) -> Result<(), Error> {
        let [first, second, third, fourth] = streams;
        let mut bits = [
            Backward::new(first)?,
            Backward::new(second)?,
            Backward::new(third)?,
            Backward::new(fourth)?,
        ];
        let mut outputs = outputs;
        let shortest = outputs.iter().map(|output| output.len()).min();
        let mut at = 0_usize;
        while at.saturating_add(PER_REFILL) <= shortest.unwrap_or_default() {
            for (bits, output) in bits.iter_mut().zip(outputs.iter_mut()) {
                bits.refill();
                let slots = output.get_mut(at..).and_then(<[u8]>::first_chunk_mut);
                for slot in slots
                    .into_iter()
                    .flat_map(|slots: &mut [u8; PER_REFILL]| slots)
                {
                    *slot = self.symbol(bits);
                }
            }
            at = at.saturating_add(PER_REFILL);
        }
        for (bits, output) in bits.iter_mut().zip(outputs) {
            self.rest(bits, output.get_mut(at..).unwrap_or_default());
            if !bits.done() {
                return Err(corrupt());
            }
        }
        Ok(())
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
            .decode(&[0b0001_0000, 0b0000_1101], &mut output)
            .unwrap();
        assert_eq!(output, [0, 1, 5, 4]);
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

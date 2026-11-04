// This is free and unencumbered software released into the public domain.

//! Reading bits as zstd packs them: forward, as its FSE table
//! descriptions are, and backward, from a stream's end to its start, as
//! its Huffman- and FSE-coded streams are (RFC 8878 §4.1).

use super::corrupt;
use crate::Error;

/// The most bits read at once.
const MAX: u32 = 56;

/// Returns the low `n` bits set, for `n` up to 64.
fn mask(n: u32) -> u64 {
    1_u64
        .checked_shl(n)
        .map_or(u64::MAX, |bit| bit.wrapping_sub(1))
}

/// Returns the eight bytes of `bytes` from `at` on, little-endian, those
/// past its end read as zeros.
fn word(bytes: &[u8], at: usize) -> u64 {
    if let Some(word) = bytes.get(at..).and_then(|rest| rest.first_chunk()) {
        return u64::from_le_bytes(*word);
    }
    let mut word = [0; 8];
    let rest = bytes.get(at..).unwrap_or_default();
    if let Some(to) = word.get_mut(..rest.len()) {
        to.copy_from_slice(rest);
    }
    u64::from_le_bytes(word)
}

/// Bits read forward, each field from the lowest bits up.
pub(super) struct Forward<'a> {
    bytes: &'a [u8],
    /// The bits read so far.
    at: usize,
}

impl<'a> Forward<'a> {
    pub(super) const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }

    /// Reads `n` bits, up to 56.
    pub(super) fn read(&mut self, n: u32) -> Result<u64, Error> {
        let n = n.min(MAX);
        let shift = u32::try_from(self.at & 7).unwrap_or_default();
        let value = word(self.bytes, self.at >> 3).wrapping_shr(shift) & mask(n);
        self.at = self
            .at
            .saturating_add(usize::try_from(n).unwrap_or(usize::MAX));
        if self.at > self.bytes.len().saturating_mul(8) {
            return Err(corrupt());
        }
        Ok(value)
    }

    /// Returns the bytes read, the last perhaps in part.
    pub(super) const fn bytes(&self) -> usize {
        self.at.div_ceil(8)
    }
}

/// Bits read backward, from a stream's end to its start: past the zeros
/// that pad its last byte, and the one bit set that marks where it begins.
#[derive(Clone)]
pub(super) struct Backward<'a> {
    bytes: &'a [u8],
    /// The bits left to read, below where reading is.
    left: usize,
    /// Whether a read went past the start, the bits missing read as zeros.
    overflow: bool,
}

impl<'a> Backward<'a> {
    /// Returns the stream of `bytes`, whose last byte must not be zero.
    pub(super) fn new(bytes: &'a [u8]) -> Result<Self, Error> {
        let last = bytes.last().copied().filter(|&last| last != 0);
        let last = last.ok_or_else(corrupt)?;
        let marker = 7_usize.saturating_sub(usize::try_from(last.leading_zeros()).unwrap_or(7));
        let left = bytes.len().saturating_sub(1).saturating_mul(8);
        Ok(Self {
            bytes,
            left: left.saturating_add(marker),
            overflow: false,
        })
    }

    /// Returns the next `n` bits, up to 56, without reading them; those
    /// past the start are zeros.
    pub(super) fn peek(&self, n: u32) -> u64 {
        let n = n.min(MAX);
        let wide = usize::try_from(n).unwrap_or(usize::MAX);
        match self.left.checked_sub(wide) {
            Some(at) => {
                let shift = u32::try_from(at & 7).unwrap_or_default();
                word(self.bytes, at >> 3).wrapping_shr(shift) & mask(n)
            },
            None => {
                let left = u32::try_from(self.left).unwrap_or(MAX);
                (word(self.bytes, 0) & mask(left)).wrapping_shl(n.saturating_sub(left))
            },
        }
    }

    /// Reads past the next `n` bits.
    pub(super) fn consume(&mut self, n: u32) {
        let n = usize::try_from(n).unwrap_or(usize::MAX);
        match self.left.checked_sub(n) {
            Some(left) => self.left = left,
            None => (self.left, self.overflow) = (0, true),
        }
    }

    /// Reads the next `n` bits, up to 56.
    pub(super) fn read(&mut self, n: u32) -> u64 {
        let value = self.peek(n);
        self.consume(n);
        value
    }

    /// Returns whether a read went past the start.
    pub(super) const fn overflowed(&self) -> bool {
        self.overflow
    }

    /// Returns whether every bit has been read, and none past the start.
    pub(super) const fn done(&self) -> bool {
        self.left == 0 && !self.overflow
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_forward_from_the_lowest_bits() {
        let mut bits = Forward::new(&[0b1010_0110, 0b0000_0011]);
        assert_eq!(bits.read(3).unwrap(), 0b110);
        assert_eq!(bits.read(7).unwrap(), 0b111_0100);
        assert_eq!(bits.bytes(), 2);
        assert_eq!(bits.read(6).unwrap(), 0);
        assert!(bits.read(1).is_err());
    }

    /// RFC 8878 §4.2.2's example: "0145" coded as `1`, `01`, `0001`, and
    /// `0000`, as its Table 26 has them, read from the top.
    #[test]
    fn reads_backward_from_the_marker() {
        let mut bits = Backward::new(&[0b0001_0000, 0b0000_1101]).unwrap();
        assert_eq!(bits.read(1), 0b1);
        assert_eq!(bits.read(2), 0b01);
        assert_eq!(bits.peek(8), 0b0001_0000);
        assert_eq!(bits.read(4), 0b0001);
        assert!(!bits.done());
        assert_eq!(bits.read(4), 0b0000);
        assert!(bits.done());
        assert_eq!(bits.read(3), 0);
        assert!(bits.overflowed() && !bits.done());
        assert!(Backward::new(&[1, 0]).is_err());
        assert!(Backward::new(&[]).is_err());
        assert!(Backward::new(&[1]).unwrap().done());
    }
}

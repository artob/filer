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
///
/// They are read from the top of a container, the eight bytes from `at`
/// on, which [`refill`](Self::refill) moves back over those read, so that
/// reading takes no load: it holds at least 57 bits unread after a refill,
/// until it reaches the start.
#[derive(Clone)]
pub(super) struct Backward<'a> {
    bytes: &'a [u8],
    container: u64,
    at: usize,
    /// The container's bits read, from its top; those past its bottom, at
    /// the start, read as zeros, or past 64, as anything.
    consumed: u32,
}

impl<'a> Backward<'a> {
    /// Returns the stream of `bytes`, whose last byte must not be zero.
    pub(super) fn new(bytes: &'a [u8]) -> Result<Self, Error> {
        let last = bytes.last().copied().filter(|&last| last != 0);
        let last = last.ok_or_else(corrupt)?;
        let at = bytes.len().saturating_sub(8);
        // A stream of less than eight bytes lies low in the container, its
        // top bytes zeros, read already.
        let missing = 8_usize.saturating_sub(bytes.len().saturating_sub(at));
        let missing = u32::try_from(missing).unwrap_or_default();
        Ok(Self {
            bytes,
            container: word(bytes, at),
            at,
            consumed: last
                .leading_zeros()
                .saturating_add(1)
                .saturating_add(missing.saturating_mul(8)),
        })
    }

    /// Moves the container back over the whole bytes read, as far as the
    /// start.
    pub(super) fn refill(&mut self) {
        let back = usize::try_from(self.consumed >> 3).unwrap_or_default();
        let back = back.min(self.at);
        // Neither goes below zero: `back` is at most either.
        self.at = self.at.wrapping_sub(back);
        let back = u32::try_from(back).unwrap_or_default();
        self.consumed = self.consumed.wrapping_sub(back.wrapping_mul(8));
        // A stream of less than eight bytes never moves, its container
        // as `new` filled it.
        let word = self.bytes.get(self.at..).and_then(<[u8]>::first_chunk);
        if let Some(&word) = word {
            self.container = u64::from_le_bytes(word);
        }
    }

    /// Returns the next `n` bits, up to 56, without reading them: the
    /// container must hold them, as it does those after a refill, up to
    /// 57; those past the start are zeros.
    pub(super) fn peek(&self, n: u32) -> u64 {
        let n = n.min(MAX);
        // Two shifts, so that none of 64 is needed when `n` is 0.
        (self.container.wrapping_shl(self.consumed) >> 1).wrapping_shr(63_u32.saturating_sub(n))
    }

    /// Reads past the next `n` bits.
    pub(super) fn consume(&mut self, n: u32) {
        self.consumed = self.consumed.saturating_add(n.min(MAX));
    }

    /// Reads the next `n` bits, up to 56, which the container must hold.
    pub(super) fn take(&mut self, n: u32) -> u64 {
        let value = self.peek(n);
        self.consume(n);
        value
    }

    /// Refills, and reads the next `n` bits, up to 56.
    pub(super) fn read(&mut self, n: u32) -> u64 {
        self.refill();
        self.take(n)
    }

    /// Returns the bits left to read, or `None` if a read went past the
    /// start.
    fn left(&self) -> Option<usize> {
        let held = self.at.saturating_mul(8).saturating_add(64);
        let consumed = usize::try_from(self.consumed).unwrap_or(usize::MAX);
        held.checked_sub(consumed)
    }

    /// Returns whether a read went past the start.
    pub(super) fn overflowed(&self) -> bool {
        self.left().is_none()
    }

    /// Returns whether every bit has been read, and none past the start.
    pub(super) fn done(&self) -> bool {
        self.left() == Some(0)
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

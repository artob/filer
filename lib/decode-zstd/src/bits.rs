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
/// until it reaches the start. It is held shifted past those read, so
/// that the next are at its top, one shift away.
#[derive(Clone)]
pub(super) struct Backward<'a> {
    bytes: &'a [u8],
    /// The container's bits unread, from its top, and zeros after them.
    unread: u64,
    at: usize,
    /// The container's bits read, from its top; those past its bottom, at
    /// the start, read as zeros.
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
        let consumed = last
            .leading_zeros()
            .saturating_add(1)
            .saturating_add(missing.saturating_mul(8));
        Ok(Self {
            bytes,
            unread: word(bytes, at).checked_shl(consumed).unwrap_or_default(),
            at,
            consumed,
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
        // as `new` filled it, shifted as read since.
        let word = self.bytes.get(self.at..).and_then(<[u8]>::first_chunk);
        if let Some(&word) = word {
            let word = u64::from_le_bytes(word);
            self.unread = word.checked_shl(self.consumed).unwrap_or_default();
        }
    }

    /// Returns the next `n` bits, up to 56, without reading them: the
    /// container must hold them, as it does those after a refill, up to
    /// 57; those past the start are zeros.
    pub(super) fn peek(&self, n: u32) -> u64 {
        // Two shifts, so that none of 64 is needed when `n` is 0; the
        // second by `!n`, which a shift takes as 63 less `n`, as it takes
        // only the low six bits.
        (self.unread >> 1).wrapping_shr(!n)
    }

    /// Reads past the next `n` bits, up to 56. However corrupt a stream,
    /// what it is read for never adds up to `u32::MAX` bits: at most
    /// 98,047 sequences of up to 89 bits, or 128 KiB of literals of up to
    /// 11.
    pub(super) fn consume(&mut self, n: u32) {
        self.consumed = self.consumed.wrapping_add(n);
        self.unread = self.unread.wrapping_shl(n);
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

    /// Returns a reader that goes on from here, by a count of the bits
    /// left alone; or nothing, if a read went past the start.
    pub(super) fn counted(&self) -> Option<Counted<'a>> {
        self.left().map(|left| Counted {
            bytes: self.bytes,
            left,
        })
    }

    /// Goes on from where `counted`, which
    /// [`counted`](Self::counted) returned of it, has read to.
    pub(super) fn recount(&mut self, counted: &Counted<'_>) {
        let left = counted.left;
        // The container ends with the byte of the next bit, or lies at
        // the start, of a stream that has less before it.
        self.at = left.div_ceil(8).saturating_sub(8);
        let held = self.at.saturating_mul(8).saturating_add(64);
        self.consumed = u32::try_from(held.saturating_sub(left)).unwrap_or(u32::MAX);
        let word = word(self.bytes, self.at);
        self.unread = word.checked_shl(self.consumed).unwrap_or_default();
    }

    /// Returns the bits left to read, or `None` if a read went past the
    /// start.
    fn left(&self) -> Option<usize> {
        let held = self.at.saturating_mul(8).saturating_add(64);
        let consumed = usize::try_from(self.consumed).unwrap_or(usize::MAX);
        held.checked_sub(consumed)
    }

    /// Returns it as a [`Marked`] reader, refilled, its stream lying from
    /// `start` of the bytes the marked reader is given, unless its stream
    /// is shorter than its container.
    pub(super) fn mark(&self, start: usize) -> Option<Marked> {
        let back = usize::try_from(self.consumed >> 3).unwrap_or_default();
        let at = self.at.checked_sub(back)?;
        let word = self.bytes.get(at..).and_then(<[u8]>::first_chunk)?;
        Some(Marked {
            unread: (u64::from_le_bytes(*word) | 1).wrapping_shl(self.consumed & 7),
            at: start.saturating_add(at),
        })
    }

    /// Goes on from where `marked`, which [`mark`](Self::mark) returned of
    /// it, at `start`, has read to.
    pub(super) fn unmark(&mut self, marked: Marked, start: usize) {
        self.at = marked.at.saturating_sub(start);
        self.consumed = marked.unread.trailing_zeros();
        let word = word(self.bytes, self.at);
        self.unread = word.checked_shl(self.consumed).unwrap_or_default();
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

/// A [`Backward`] reader for a loop that reads a whole sequence from one
/// container: no more than how many bits are left to read, from which it
/// loads the container anew each time, so that reading waits on nothing
/// but that count.
#[derive(Clone, Copy)]
pub(super) struct Counted<'a> {
    bytes: &'a [u8],
    left: usize,
}

impl Counted<'_> {
    /// Returns a container whose top holds the next bits, 57 at least, if
    /// the stream holds it whole: until it has fewer than 57 left.
    #[inline(always)]
    pub(super) fn container(&self) -> Option<u64> {
        // The eight bytes that end with the next bit's; short of them, the
        // subtraction wraps past the stream's end.
        let at = (self.left.wrapping_sub(1) >> 3).wrapping_sub(7);
        let word = self.bytes.get(at..)?.first_chunk()?;
        let above = u32::try_from(self.left.wrapping_neg() & 7).unwrap_or_default();
        Some(u64::from_le_bytes(*word).wrapping_shl(above))
    }

    /// Reads past the next `n` bits, no more than a container held.
    #[inline(always)]
    pub(super) fn consume(&mut self, n: u32) {
        // A container holds no more than are left, so this never wraps.
        let n = usize::try_from(n).unwrap_or_default();
        self.left = self.left.wrapping_sub(n);
    }
}

/// A [`Backward`] reader for loops that read several streams at once,
/// with few registers to spare: its container is marked by a set bit
/// below its unread bits, whose place tells how many it has read, and it
/// lies at `at` of the bytes that it is given, which all the streams lie
/// in. Read no more than 56 bits from a refill, it reads only its own
/// stream's while it lies at least eight bytes from that stream's start.
#[derive(Clone, Copy)]
pub(super) struct Marked {
    unread: u64,
    pub(super) at: usize,
}

impl Marked {
    /// Moves the container back over the whole bytes read, in `bytes`.
    #[inline(always)]
    pub(super) fn refill(&mut self, bytes: &[u8]) {
        let consumed = self.unread.trailing_zeros();
        let back = usize::try_from(consumed >> 3).unwrap_or_default();
        self.at = self.at.wrapping_sub(back);
        let word = bytes.get(self.at..).and_then(<[u8]>::first_chunk);
        let word = word.copied().map_or(0, u64::from_le_bytes);
        self.unread = (word | 1).wrapping_shl(consumed & 7);
    }

    /// Returns the next `n` bits, from 1 to 56, without reading them.
    #[inline(always)]
    pub(super) const fn peek(&self, n: u32) -> u64 {
        self.unread.wrapping_shr(64_u32.wrapping_sub(n))
    }

    /// Reads past the next `n` bits, up to 56.
    #[inline(always)]
    pub(super) const fn consume(&mut self, n: u32) {
        self.unread = self.unread.wrapping_shl(n);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

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
    /// A counted reader's containers hold what refills do, and a reader
    /// goes on from where one has read to.
    #[test]
    fn counts_bits_as_it_refills() {
        let bytes: Vec<u8> = (1..=40_u32).map(|n| (n * 37 % 251) as u8 | 1).collect();
        let (mut refilled, mut resumed) = (
            Backward::new(&bytes).unwrap(),
            Backward::new(&bytes).unwrap(),
        );
        let mut counted = resumed.counted().unwrap();
        let (mut n, mut whole) = (1, 0);
        while let Some(container) = counted.container() {
            assert_eq!(container >> (64 - n), refilled.read(n));
            counted.consume(n);
            (n, whole) = (n % 57 + 3, whole + 1);
        }
        // Until fewer than 57 bits are left; the rest by refills.
        assert!(whole > 4 && counted.left < 57);
        resumed.recount(&counted);
        while !refilled.done() {
            assert_eq!(resumed.read(1), refilled.read(1));
            assert!(!refilled.overflowed());
        }
        assert!(resumed.done());
        // A stream shorter than a container has none whole.
        let short = Backward::new(&bytes[..5]).unwrap();
        assert!(short.counted().unwrap().container().is_none());
    }

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

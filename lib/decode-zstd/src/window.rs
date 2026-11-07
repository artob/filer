// This is free and unencumbered software released into the public domain.

//! A frame's window (RFC 8878 §3.1.1.1.2): the latest bytes it decoded,
//! which its blocks copy from, and those not yet produced.

use crate::{Error, corrupt};
use alloc::vec::Vec;

/// The bytes a short copy takes at once, past what it copies, and the
/// room kept past a block for it, and before the window: so that what it
/// writes past the end is overwritten later, never needed.
const SHORT: usize = 16;
const SLACK: usize = 32;

/// What a frame has decoded, in a buffer of its window and two blocks,
/// allocated as it fills, and then never again: decoding goes round it,
/// from its start again once a block may not fit before its end, so that
/// what is decoded past there, the previous lap, is still copied from,
/// until the current lap reaches it.
#[derive(Clone)]
pub(super) struct Window {
    bytes: Vec<u8>,
    /// Where the current lap ends, the next byte decoded goes.
    end: usize,
    /// Where the previous lap ended, if any; its bytes from `end` on are
    /// still the window's.
    lapped: usize,
    /// How many of the current lap's bytes have been produced.
    produced: usize,
    /// How far back blocks copy from, and the most a block decodes to.
    size: usize,
    block: usize,
    /// What the frame says it decodes to, if it does, which the buffer
    /// takes at once, if less than the most it holds.
    expected: Option<usize>,
}

impl Window {
    /// Returns an empty window of `size`, of blocks of at most `block`,
    /// for a frame that decodes to `expected` bytes, if it says.
    pub(super) const fn new(size: usize, block: usize, expected: Option<usize>) -> Self {
        Self {
            bytes: Vec::new(),
            end: 0,
            lapped: 0,
            produced: 0,
            size,
            block,
            expected,
        }
    }

    /// The most the buffer holds: the window, and two blocks, each with
    /// its slack, so that a lap that starts over leaves the window behind
    /// it whole, and a short copy past a block's end too.
    fn capacity(&self) -> usize {
        let block = self.block.saturating_add(SLACK);
        self.size.saturating_add(block.saturating_mul(2))
    }

    /// Makes room for a block at the end, once everything decoded has
    /// been produced: by growing the buffer, to what the frame decodes to
    /// if it says, or else doubling it, up to its capacity; or else by
    /// starting a lap.
    pub(super) fn prepare(&mut self) -> Result<(), Error> {
        let need = self.end.saturating_add(self.block).saturating_add(SLACK);
        if need <= self.bytes.len() {
            return Ok(());
        }
        let len = self.bytes.len();
        if len < self.capacity() {
            let expected = self.expected.unwrap_or_default();
            let grown = len.saturating_mul(2).max(need).max(4096).max(expected);
            let grown = grown.min(self.capacity());
            self.bytes.reserve_exact(grown.saturating_sub(len));
            self.bytes.resize(grown, 0);
            if need <= grown {
                return Ok(());
            }
        }
        // A lap starts over only once all is produced; the last ended past
        // the window, a block, and its slack, so that the window is left
        // whole, even by a short copy past a block's end.
        if self.pending() {
            return Err(corrupt());
        }
        (self.lapped, self.end, self.produced) = (self.end, 0, 0);
        Ok(())
    }

    /// Where the current lap ends: where a block about to decode starts.
    pub(super) const fn end(&self) -> usize {
        self.end
    }

    /// Returns how many bytes back may be copied from: those of the
    /// current lap, and of the previous one, past where this one ends.
    fn history(&self) -> usize {
        self.end
            .saturating_add(self.lapped.saturating_sub(self.end))
    }

    /// Returns the room for `n` more bytes at the end.
    fn room(&mut self, n: usize) -> Result<&mut [u8], Error> {
        let to = self.end.checked_add(n).ok_or_else(corrupt)?;
        self.bytes.get_mut(self.end..to).ok_or_else(corrupt)
    }

    /// Appends `bytes`.
    pub(super) fn extend(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.room(bytes.len())?.copy_from_slice(bytes);
        self.end = self.end.saturating_add(bytes.len());
        Ok(())
    }

    /// Appends `n` of `byte`.
    pub(super) fn fill(&mut self, byte: u8, n: usize) -> Result<(), Error> {
        self.room(n)?.fill(byte);
        self.end = self.end.saturating_add(n);
        Ok(())
    }

    /// Returns a cursor at the end, to decode a block of at most `max`
    /// bytes onto, which [`advance`](Self::advance) then takes the end of.
    pub(super) fn cursor(&mut self, max: usize) -> Cursor<'_> {
        Cursor {
            bytes: &mut self.bytes,
            end: self.end,
            limit: self.end.saturating_add(max),
            lapped: self.lapped,
            size: self.size,
        }
    }

    /// Moves the end to where a block's cursor ended.
    pub(super) fn advance(&mut self, end: usize) {
        self.end = end;
    }

    /// Returns what the current lap holds from `start`: what a block that
    /// started there decoded to.
    pub(super) fn since(&self, start: usize) -> &[u8] {
        self.bytes.get(start..self.end).unwrap_or_default()
    }

    /// Produces what it can into `output`, and returns how much.
    pub(super) fn produce(&mut self, output: &mut [u8]) -> usize {
        let pending = self.bytes.get(self.produced..self.end).unwrap_or_default();
        let n = pending.len().min(output.len());
        if let (Some(to), Some(from)) = (output.get_mut(..n), pending.get(..n)) {
            to.copy_from_slice(from);
        }
        self.produced = self.produced.saturating_add(n);
        n
    }

    pub(super) const fn pending(&self) -> bool {
        self.produced < self.end
    }

    /// Returns the bytes it holds, allocated.
    pub(super) fn footprint(&self) -> usize {
        self.bytes.capacity()
    }

    /// Returns a copy of what later blocks may copy from or are yet to
    /// produce: the window's size of the latest bytes, or more if more are
    /// pending, in a buffer of just those, to grow again as it decodes.
    pub(super) fn copy_state(&self) -> Self {
        let pending = self.end.saturating_sub(self.produced);
        let keep = self.size.max(pending).min(self.history());
        // Those of the previous lap first, then the current one's.
        let current = keep.min(self.end);
        let previous = keep.saturating_sub(current);
        let from = self.lapped.saturating_sub(previous);
        let mut bytes = Vec::with_capacity(keep);
        bytes.extend_from_slice(self.bytes.get(from..self.lapped).unwrap_or_default());
        let start = self.end.saturating_sub(current);
        bytes.extend_from_slice(self.bytes.get(start..self.end).unwrap_or_default());
        Self {
            end: bytes.len(),
            produced: bytes.len().saturating_sub(pending),
            bytes,
            lapped: 0,
            size: self.size,
            block: self.block,
            expected: None,
        }
    }
}

/// Where a block decodes to: the end of a window, held apart from it, so
/// that it stays in a register as the block's sequences run.
pub(super) struct Cursor<'a> {
    bytes: &'a mut [u8],
    /// Where the next byte goes, and the most it may reach.
    pub(super) end: usize,
    limit: usize,
    lapped: usize,
    size: usize,
}

impl Cursor<'_> {
    /// Returns the room for `n` more bytes at the end, within the limit.
    fn room(&mut self, n: usize) -> Result<&mut [u8], Error> {
        let to = self.end.checked_add(n).filter(|&to| to <= self.limit);
        let to = to.ok_or_else(corrupt)?;
        self.bytes.get_mut(self.end..to).ok_or_else(corrupt)
    }

    /// Appends `bytes`.
    pub(super) fn extend(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.room(bytes.len())?.copy_from_slice(bytes);
        self.end = self.end.saturating_add(bytes.len());
        Ok(())
    }

    /// Appends the first `len` of `bytes`: if few, by a short copy, which
    /// takes more of them if there are, and writes them past the end.
    pub(super) fn literals(&mut self, bytes: &[u8], len: usize) -> Result<(), Error> {
        let to = self.end.checked_add(len).filter(|&to| to <= self.limit);
        let to = to.ok_or_else(corrupt)?;
        if len <= SHORT
            && let Some(source) = bytes.first_chunk::<SHORT>()
            && let Some(target) = self.short()
        {
            *target = *source;
            self.end = to;
            return Ok(());
        }
        self.extend(bytes.get(..len).ok_or_else(corrupt)?)
    }

    /// Returns the room for a short copy at the end, if the buffer has it.
    fn short(&mut self) -> Option<&mut [u8; SHORT]> {
        self.bytes.get_mut(self.end..)?.first_chunk_mut::<SHORT>()
    }

    /// Appends `len` bytes copied from `offset` back, at most the window's
    /// size and what it has decoded, which they may overlap, repeating what
    /// they copy: from the previous lap first, if it reaches back there.
    pub(super) fn copy(&mut self, offset: usize, len: usize) -> Result<(), Error> {
        if offset == 0 || offset > self.size {
            return Err(corrupt());
        }
        let to = self.end.checked_add(len).filter(|&to| to <= self.limit);
        let to = to.ok_or_else(corrupt)?;
        // Most often, from the current lap, not overlapping: a short copy,
        // if it is far enough back that one does not overlap either.
        if let Some(from) = self.end.checked_sub(offset).filter(|_| len <= offset) {
            if len <= SHORT && offset >= SHORT {
                let source = self.bytes.get(from..).and_then(|bytes| bytes.first_chunk());
                if let Some(&source) = source
                    && let Some(target) = self.short()
                {
                    *target = source;
                    self.end = to;
                    return Ok(());
                }
            }
            let (before, after) = self.bytes.split_at_mut(self.end.min(self.bytes.len()));
            let source = before.get(from..from.saturating_add(len));
            let target = after.get_mut(..len);
            let (Some(source), Some(target)) = (source, target) else {
                return Err(corrupt());
            };
            target.copy_from_slice(source);
            self.end = to;
            return Ok(());
        }
        let history = self
            .end
            .saturating_add(self.lapped.saturating_sub(self.end));
        if offset > history {
            return Err(corrupt());
        }
        let mut left = len;
        if let Some(back) = offset.checked_sub(self.end).filter(|&back| back > 0) {
            // `back` bytes from the previous lap's end, which the current
            // lap has not reached, as the window lies wholly behind it.
            let from = self.lapped.checked_sub(back).ok_or_else(corrupt)?;
            let n = left.min(back);
            self.within(from, n)?;
            left = left.saturating_sub(n);
        }
        while left > 0 {
            let from = self.end.checked_sub(offset).ok_or_else(corrupt)?;
            let n = left.min(offset);
            self.within(from, n)?;
            left = left.saturating_sub(n);
        }
        Ok(())
    }

    /// Appends `n` bytes from `from`, which must not overlap the end.
    fn within(&mut self, from: usize, n: usize) -> Result<(), Error> {
        let to = from.checked_add(n).ok_or_else(corrupt)?;
        let end = self.end.checked_add(n).ok_or_else(corrupt)?;
        if to > self.bytes.len() || end > self.bytes.len() {
            return Err(corrupt());
        }
        self.bytes.copy_within(from..to, self.end);
        self.end = end;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    /// Copies `len` bytes from `offset` back onto `window`, as a block.
    fn copy(window: &mut Window, offset: usize, len: usize) -> Result<(), Error> {
        let mut cursor = window.cursor(len);
        cursor.copy(offset, len)?;
        let end = cursor.end;
        window.advance(end);
        Ok(())
    }

    /// Decodes `bytes` onto `window`, a block of them at a time, producing
    /// each as it goes, and returns all it produced.
    fn decode(window: &mut Window, bytes: &[u8]) -> Vec<u8> {
        let mut produced = Vec::new();
        for block in bytes.chunks(window.block) {
            window.prepare().unwrap();
            window.extend(block).unwrap();
            let mut output = vec![0; block.len()];
            assert_eq!(window.produce(&mut output), block.len());
            produced.extend_from_slice(&output);
        }
        produced
    }

    #[test]
    fn copies_across_laps() {
        // A window of 10 and blocks of 4: a buffer of 82, with the slack
        // of each, round which decoding goes once a block and its slack
        // may not fit.
        let mut window = Window::new(10, 4, None);
        let bytes: Vec<u8> = (0..52).collect();
        assert_eq!(decode(&mut window, &bytes), bytes);
        assert_eq!(window.bytes.len(), 10 + 2 * (4 + SLACK));
        // The current lap holds 48 to 51, the previous still 4 to 47.
        assert_eq!((window.end, window.lapped), (4, 48));
        window.prepare().unwrap();
        // From the previous lap.
        copy(&mut window, 8, 2).unwrap();
        assert_eq!(window.since(4), [44, 45]);
        window.prepare().unwrap();
        // From the previous lap into the current one.
        copy(&mut window, 8, 4).unwrap();
        assert_eq!(window.since(6), [46, 47, 48, 49]);
        window.prepare().unwrap();
        // Overlapping what it copies.
        copy(&mut window, 2, 3).unwrap();
        assert_eq!(window.since(10), [48, 49, 48]);
        // Past the window, or past what was ever decoded.
        assert!(copy(&mut window, 11, 1).is_err());
        assert!(copy(&mut Window::new(10, 4, None), 1, 1).is_err());
    }

    #[test]
    fn copies_few_bytes_at_once() {
        let mut window = Window::new(64, 16, None);
        let bytes: Vec<u8> = (0..40).collect();
        decode(&mut window, &bytes);
        window.prepare().unwrap();
        let mut cursor = window.cursor(16);
        // Five literals of more, and five bytes from twenty back, each
        // writing past what it adds, which what follows overwrites.
        cursor.literals(&[100; 20], 5).unwrap();
        cursor.copy(20, 5).unwrap();
        cursor.literals(&[200; 3], 3).unwrap();
        assert!(cursor.copy(20, 4).is_err());
        let end = cursor.end;
        window.advance(end);
        assert_eq!(
            window.since(40),
            [100, 100, 100, 100, 100, 25, 26, 27, 28, 29, 200, 200, 200]
        );
    }

    #[test]
    fn copies_only_the_window_into_state() {
        let mut window = Window::new(10, 4, None);
        let bytes: Vec<u8> = (0..20).collect();
        decode(&mut window, &bytes);
        let copy = window.copy_state();
        assert_eq!(copy.bytes, (10..20).collect::<Vec<u8>>());
        let mut copy = copy;
        copy.prepare().unwrap();
        self::copy(&mut copy, 10, 2).unwrap();
        assert_eq!(copy.since(10), [10, 11]);
    }
}

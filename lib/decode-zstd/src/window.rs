// This is free and unencumbered software released into the public domain.

//! A frame's window (RFC 8878 §3.1.1.1.2): the latest bytes it decoded,
//! which its blocks copy from, and those not yet produced.

use super::sequences::Sequence;
use crate::{Error, corrupt};
use alloc::vec::Vec;

/// The bytes a short copy takes at once, past what it copies.
const SHORT: usize = 16;

/// The bytes a wide copy takes at once, as a sequence's literals and its
/// match each are, if no longer, and the room the two take past where
/// they start, each written whole.
const WIDE: usize = 32;
const ROOM: usize = 2 * WIDE;

/// The room kept past a block for such copies, and before the window: so
/// that what they write past the end is overwritten later, never needed.
const SLACK: usize = ROOM;

/// The longest match copied by short copies, though it does not overlap:
/// past it, a copy at once is faster.
const LONG: usize = 64;

/// The most literals copied by short copies, which then write less than
/// the slack past them.
const LITERALS: usize = 48;

/// The most literals, and the longest match, that copies of a fixed
/// width take in turn: past it, a copy at once is faster.
const TURNS: usize = 256;

/// What a pattern of up to 15 bytes, held low in sixteen, is multiplied
/// by to fill them: a one in each byte where it starts over.
#[allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "evaluated when the crate is built, where a panic fails the build"
)]
const REPEATS: [u128; SHORT] = {
    let mut repeats = [0; SHORT];
    let mut period = 1;
    while period < SHORT {
        let (mut ones, mut at) = (0_u128, 0);
        while at < SHORT {
            ones |= 1 << (8 * at);
            at += period;
        }
        repeats[period] = ones;
        period += 1;
    }
    repeats
};

/// How far back a pattern of up to 15 bytes last began at least sixteen
/// bytes back: as many whole patterns as take that.
#[allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "evaluated when the crate is built, where a panic fails the build"
)]
const STRIDES: [usize; SHORT] = {
    let mut strides = [0; SHORT];
    let mut period = 1;
    while period < SHORT {
        strides[period] = SHORT.div_ceil(period) * period;
        period += 1;
    }
    strides
};

/// What a frame has decoded, in a buffer of its window and two blocks,
/// allocated as it fills, and then never again: decoding goes round it,
/// from its start again once a block may not fit before its end, so that
/// what is decoded past there, the previous lap, is still copied from,
/// until the current lap reaches it. A frame that says what it decodes
/// to, no more than the buffer may hold, takes a buffer of just that,
/// and never goes round.
///
/// The buffer is set to zeros, as memory that is written to must be,
/// only as far as the next block may reach: so that the zeros are still
/// in the cache when the block is decoded over them, and a frame is not
/// held up for those of blocks that it never comes to.
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

    /// Returns where the frame ends in the buffer, if it says what it
    /// decodes to and the buffer may hold that, with its slack: no block
    /// then decodes past there, and no lap starts over.
    fn bound(&self) -> Option<usize> {
        let fits = |expected: &usize| expected.saturating_add(SLACK) <= self.capacity();
        self.expected.filter(fits)
    }

    /// Returns the most that a block which starts at the end may reach.
    fn reach(&self) -> usize {
        let reach = self.end.saturating_add(self.block);
        self.bound().map_or(reach, |bound| reach.min(bound))
    }

    /// Makes room for a block at the end, once everything decoded has
    /// been produced: by growing the buffer, to what the frame decodes to
    /// if it says, or else doubling it, up to its capacity, zeros as far
    /// as the block may reach; or else by starting a lap.
    pub(super) fn prepare(&mut self) -> Result<(), Error> {
        let need = self.reserve()?;
        if need > self.bytes.len() {
            self.bytes.resize(need, 0);
        }
        Ok(())
    }

    /// Allocates room for a block at the end, or else starts a lap, as
    /// [`prepare`](Self::prepare) does, and returns how far the buffer
    /// must reach for the block: what it allocates is yet to be set.
    fn reserve(&mut self) -> Result<usize, Error> {
        let need = self.reach().saturating_add(SLACK);
        let len = self.bytes.len();
        if need <= self.capacity() {
            if need > self.bytes.capacity() {
                let expected = self.expected.unwrap_or_default().saturating_add(SLACK);
                let grown = len.saturating_mul(2).max(need).max(4096).max(expected);
                let grown = grown.min(self.capacity());
                self.bytes.reserve_exact(grown.saturating_sub(len));
            }
            return Ok(need);
        }
        // A lap starts over only once all is produced; the last ended past
        // the window, a block, and its slack, so that the window is left
        // whole, even by a short copy past a block's end.
        if self.pending() {
            return Err(corrupt());
        }
        (self.lapped, self.end, self.produced) = (self.end, 0, 0);
        Ok(self.reach().saturating_add(SLACK))
    }

    /// Returns where a block of `n` bytes that starts at the end ends, if
    /// it may: no more than a block, nor past what the frame decodes to.
    fn ends(&self, n: usize) -> Result<usize, Error> {
        let to = self.end.checked_add(n).filter(|&to| to <= self.reach());
        to.ok_or_else(corrupt)
    }

    /// Appends `bytes`, a block stored as it is, once everything decoded
    /// has been produced, and returns where it starts: over what the
    /// buffer holds, and onto its end past that, so that nothing is set
    /// to zeros to be written over.
    pub(super) fn append(&mut self, bytes: &[u8]) -> Result<usize, Error> {
        self.reserve()?;
        let (start, to) = (self.end, self.ends(bytes.len())?);
        let held = self.bytes.len().min(to).saturating_sub(start);
        let (over, onto) = bytes.split_at_checked(held).ok_or_else(corrupt)?;
        let room = self.bytes.get_mut(start..).unwrap_or_default();
        room.get_mut(..held)
            .ok_or_else(corrupt)?
            .copy_from_slice(over);
        self.bytes.extend_from_slice(onto);
        self.end = to;
        Ok(start)
    }

    /// Appends `n` of `byte`, a block of one byte repeated, as
    /// [`append`](Self::append) does a block stored as it is.
    pub(super) fn repeat(&mut self, byte: u8, n: usize) -> Result<usize, Error> {
        self.reserve()?;
        let (start, to) = (self.end, self.ends(n)?);
        let held = self.bytes.len().min(to);
        self.bytes
            .get_mut(start..held)
            .unwrap_or_default()
            .fill(byte);
        if to > held {
            self.bytes.resize(to, byte);
        }
        self.end = to;
        Ok(start)
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
    #[cfg(test)]
    fn room(&mut self, n: usize) -> Result<&mut [u8], Error> {
        let to = self.end.checked_add(n).ok_or_else(corrupt)?;
        self.bytes.get_mut(self.end..to).ok_or_else(corrupt)
    }

    /// Appends `bytes`.
    #[cfg(test)]
    pub(super) fn extend(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.room(bytes.len())?.copy_from_slice(bytes);
        self.end = self.end.saturating_add(bytes.len());
        Ok(())
    }

    /// Returns a cursor at the end, to decode a block of at most `max`
    /// bytes onto, and no further than the frame says it decodes to,
    /// which [`advance`](Self::advance) then takes the end of.
    pub(super) fn cursor(&mut self, max: usize) -> Cursor<'_> {
        let limit = self.end.saturating_add(max);
        let limit = self.bound().map_or(limit, |bound| limit.min(bound));
        Cursor {
            bytes: &mut self.bytes,
            end: self.end,
            limit,
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
/// that it stays in a register as the block's sequences run; or the end of
/// what a frame decoded to so far, straight into a caller's buffer.
pub(super) struct Cursor<'a> {
    bytes: &'a mut [u8],
    /// Where the next byte goes, and the most it may reach.
    pub(super) end: usize,
    limit: usize,
    lapped: usize,
    size: usize,
}

impl<'a> Cursor<'a> {
    /// Returns a cursor at `end` of `bytes`, which hold a frame from its
    /// start, of a window of `size`, to decode a block of at most `max`
    /// bytes onto: what it copies from is what the frame decoded to.
    pub(super) fn over(bytes: &'a mut [u8], end: usize, max: usize, size: usize) -> Self {
        Self {
            bytes,
            end,
            limit: end.saturating_add(max),
            lapped: 0,
            size,
        }
    }

    /// Returns a copy of it, of the same bytes, whose end the caller takes
    /// back: a value of its own, which stays in registers as sequences run
    /// onto it, where one behind a reference is read and written back to
    /// memory round each.
    pub(super) fn apart(&mut self) -> Cursor<'_> {
        Cursor {
            bytes: self.bytes,
            end: self.end,
            limit: self.limit,
            lapped: self.lapped,
            size: self.size,
        }
    }

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

    /// Appends `n` of `byte`.
    pub(super) fn fill(&mut self, byte: u8, n: usize) -> Result<(), Error> {
        self.room(n)?.fill(byte);
        self.end = self.end.saturating_add(n);
        Ok(())
    }

    /// Appends the first `len` of `bytes`: if few, by a short copy, which
    /// takes more of them if there are, and writes them past the end; if
    /// up to `LITERALS`, by as many as it takes, if `bytes` and the buffer
    /// hold them, a fixed most, so that they are not made a call to
    /// `memcpy`.
    #[inline(always)]
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
        if len <= LITERALS
            && let Some(source) = bytes.first_chunk::<LITERALS>()
            && let Some(target) = self
                .bytes
                .get_mut(self.end..)
                .and_then(<[u8]>::first_chunk_mut::<LITERALS>)
        {
            let (sources, _) = source.as_chunks::<SHORT>();
            let (targets, _) = target.as_chunks_mut::<SHORT>();
            for (n, (target, source)) in targets.iter_mut().zip(sources).enumerate() {
                if n.wrapping_mul(SHORT) >= len {
                    break;
                }
                *target = *source;
            }
            self.end = to;
            return Ok(());
        }
        self.extend(bytes.get(..len).ok_or_else(corrupt)?)
    }

    /// Returns whether a copy may reach `offset` back: no farther than
    /// the window's size.
    pub(super) const fn reaches(&self, offset: usize) -> bool {
        offset <= self.size
    }

    /// Runs `sequences`, each its literals, from the start of `literals`,
    /// which hold as many, and then its match, none from farther back
    /// than the window [reaches](Self::reaches); and returns the literals
    /// past them. Those that take no more than wide copies run by them,
    /// checked only for what memory they touch: the limit is checked once
    /// they have all run, which fails no later than the block does.
    pub(super) fn run<'l>(
        &mut self,
        sequences: &[Sequence],
        literals: &'l [u8],
    ) -> Result<&'l [u8], Error> {
        let within = [self.limit, self.lapped, self.size];
        let (literals, end) = run(self.bytes, self.end, within, sequences, literals)?;
        self.end = end;
        if end > self.limit {
            return Err(corrupt());
        }
        Ok(literals)
    }

    /// Returns the room for a short copy at the end, if the buffer has it.
    fn short(&mut self) -> Option<&mut [u8; SHORT]> {
        self.bytes.get_mut(self.end..)?.first_chunk_mut::<SHORT>()
    }

    /// Appends `len` bytes copied from `offset` back, at most the window's
    /// size and what it has decoded, which they may overlap, repeating what
    /// they copy: from the previous lap first, if it reaches back there.
    #[inline(always)]
    pub(super) fn copy(&mut self, offset: usize, len: usize) -> Result<(), Error> {
        if offset == 0 || offset > self.size {
            return Err(corrupt());
        }
        let to = self.end.checked_add(len).filter(|&to| to <= self.limit);
        let to = to.ok_or_else(corrupt)?;
        // Most often, from the current lap: by short copies, if it is far
        // enough back that each reads only what is written already, and
        // the match is short or overlaps.
        if let Some(from) = self.end.checked_sub(offset)
            && offset >= SHORT
            && (len <= LONG || len > offset)
            && self.shorts(from, to)
        {
            self.end = to;
            return Ok(());
        }
        self.end = far(self.bytes, self.end, self.lapped, offset, len)?;
        Ok(())
    }

    /// Copies what lies from `from` on to the end, up to `to`, by short
    /// copies, which write past `to`, and returns whether the buffer has
    /// room for them: `from` must lie at least a short copy back.
    #[inline(always)]
    fn shorts(&mut self, mut from: usize, to: usize) -> bool {
        if to.saturating_add(SHORT) > self.bytes.len() {
            return false;
        }
        let mut at = self.end;
        while at < to {
            let source = self
                .bytes
                .get(from..)
                .and_then(<[u8]>::first_chunk::<SHORT>);
            let Some(&source) = source else {
                return false;
            };
            let target = self
                .bytes
                .get_mut(at..)
                .and_then(|bytes| bytes.first_chunk_mut());
            let Some(target) = target else {
                return false;
            };
            *target = source;
            // Neither wraps: both lie below `to`, which the buffer holds
            // a short copy past.
            (from, at) = (from.wrapping_add(SHORT), at.wrapping_add(SHORT));
        }
        true
    }
}

/// Runs `sequence` onto `bytes` at `end` by wide copies, its literals from
/// the start of `literals`, and returns the literals past them and where
/// it ends; or nothing, having moved nothing on, unless both hold a wide
/// copy from there, its literals are fewer than one, its match is no
/// longer than a few, and lies at least one back from its literals, in
/// what `bytes` hold before `end`.
///
/// The literals are written whole, then the match over what they wrote
/// past their length, so neither copy waits on the other's bytes; and a
/// match longer than one copy goes on by more, each from at least one
/// back.
#[inline(always)]
fn wide<'l>(
    bytes: &mut [u8],
    end: usize,
    literals: &'l [u8],
    sequence: &Sequence,
) -> Option<(&'l [u8], usize)> {
    let (literal_len, match_len, offset) = (sequence.literals, sequence.matched, sequence.offset);
    let source = literals.first_chunk::<WIDE>()?;
    if literal_len >= WIDE || match_len > LONG {
        return None;
    }
    let (before, after) = bytes.split_at_mut_checked(end)?;
    let room = after.first_chunk_mut::<ROOM>()?;
    // How far back the match lies from where the literals start: an
    // offset within the literals wraps, and lies before any buffer.
    let back = offset.wrapping_sub(literal_len);
    let from = before.len().checked_sub(back)?;
    let matched = before.get(from..)?.first_chunk::<WIDE>()?;
    *room.first_chunk_mut::<WIDE>()? = *source;
    *room.get_mut(literal_len..)?.first_chunk_mut::<WIDE>()? = *matched;
    // Neither wraps: the lengths are below those checked, and `end` is
    // within the buffer.
    let start = end.wrapping_add(literal_len);
    let to = start.wrapping_add(match_len);
    let mut at = start.wrapping_add(WIDE);
    while at < to {
        let (before, after) = bytes.split_at_mut_checked(at)?;
        let from = at.checked_sub(offset)?;
        let matched = before.get(from..)?.first_chunk::<WIDE>()?;
        *after.first_chunk_mut::<WIDE>()? = *matched;
        at = at.wrapping_add(WIDE);
    }
    Some((literals.get(literal_len..)?, to))
}

/// Runs `sequences` onto `bytes` from `end`, as [`Cursor::run`] does,
/// whose limit, lap, and size are `within`, and returns the literals past
/// them and where they end, the limit unchecked.
///
/// A function of its own, the end and the literals held apart in it, so
/// that they stay in registers: no copy waits on the one before, but
/// where it copies what that one wrote. With `intrinsics`, it is built
/// for AVX2 too, which copies a wide copy's bytes at once, and taken
/// where the CPU has it.
#[cfg_attr(
    all(feature = "intrinsics", target_arch = "x86_64"),
    multiversion::multiversion(targets("x86_64+avx2+bmi1+bmi2+lzcnt+popcnt"))
)]
#[inline(never)]
fn run<'l>(
    bytes: &mut [u8],
    end: usize,
    within: [usize; 3],
    sequences: &[Sequence],
    literals: &'l [u8],
) -> Result<(&'l [u8], usize), Error> {
    /// Runs `sequence` onto `bytes` at `end`, its literals from the start of
    /// `literals`, as [`wide`] does, where it does not: more literals than a
    /// wide copy, a longer match than a few, or one from fewer than a wide
    /// copy back, which copies what it writes. It returns the literals past
    /// them and where it ends; or nothing, having written nothing that
    /// counts, unless both hold each copy whole, and neither the literals nor
    /// the match are longer than copies in turn serve.
    ///
    /// The literals go by wide copies. A match from at least a wide copy
    /// back does too, each from behind what the last wrote; from at least a
    /// short copy back, by short copies likewise; and from nearer, what it
    /// repeats, a pattern as long as it is far, is first multiplied out to a
    /// short copy's bytes, and then copied on from where the pattern last
    /// began a short copy back. A match from before the buffer's start
    /// lies in its previous lap, which ended at `lapped`, and is copied
    /// from there by `behind`. A function of its own, not inlined, so that
    /// the loop that calls it stays in registers; and within [`run`], so
    /// that it is built for the instructions that `run` is.
    #[cfg_attr(
        all(feature = "intrinsics", target_arch = "x86_64"),
        multiversion::inherit_target
    )]
    #[inline(never)]
    fn turns<'l>(
        bytes: &mut [u8],
        end: usize,
        lapped: usize,
        literals: &'l [u8],
        sequence: &Sequence,
    ) -> Option<(&'l [u8], usize)> {
        let (literal_len, match_len, offset) =
            (sequence.literals, sequence.matched, sequence.offset);
        if literal_len > TURNS || match_len > TURNS {
            return None;
        }
        // None of these wraps: the lengths are below those checked, and
        // `end` is within the buffer.
        let mut copied = 0_usize;
        loop {
            let source = literals.get(copied..)?.first_chunk::<WIDE>()?;
            let at = end.wrapping_add(copied);
            *bytes.get_mut(at..)?.first_chunk_mut::<WIDE>()? = *source;
            copied = copied.wrapping_add(WIDE);
            if copied >= literal_len {
                break;
            }
        }
        let start = end.wrapping_add(literal_len);
        let to = start.wrapping_add(match_len);
        let mut at = start;
        if offset >= WIDE {
            while at < to {
                let (before, after) = bytes.split_at_mut_checked(at)?;
                let Some(from) = at.checked_sub(offset) else {
                    return behind(bytes, start, lapped, literals, sequence);
                };
                let matched = before.get(from..)?.first_chunk::<WIDE>()?;
                *after.first_chunk_mut::<WIDE>()? = *matched;
                at = at.wrapping_add(WIDE);
            }
            return Some((literals.get(literal_len..)?, to));
        }
        let mut stride = offset;
        if offset < SHORT {
            // The pattern is the last `offset` of the sixteen bytes before
            // the match, its high ones; a pattern of none repeats nothing.
            let (before, after) = bytes.split_at_mut_checked(at)?;
            let last = u128::from_le_bytes(*before.last_chunk::<SHORT>()?);
            let high = u32::try_from(SHORT.checked_sub(offset)?.wrapping_mul(8)).ok()?;
            let pattern = last.checked_shr(high).filter(|_| offset > 0)?;
            let repeated = pattern.wrapping_mul(*REPEATS.get(offset)?);
            *after.first_chunk_mut::<SHORT>()? = repeated.to_le_bytes();
            stride = *STRIDES.get(offset)?;
            at = at.wrapping_add(SHORT);
        }
        while at < to {
            let (before, after) = bytes.split_at_mut_checked(at)?;
            let from = at.checked_sub(stride)?;
            let matched = before.get(from..)?.first_chunk::<SHORT>()?;
            *after.first_chunk_mut::<SHORT>()? = *matched;
            at = at.wrapping_add(SHORT);
        }
        Some((literals.get(literal_len..)?, to))
    }

    /// Copies `sequence`'s match onto `bytes` at `start`, past its
    /// literals, as `turns` does, where it lies wholly in the buffer's
    /// previous lap, which ended at `lapped`: by wide copies too, as what
    /// that lap left of the window lies past all that this one writes, a
    /// block and its slack away. It returns the `literals` past the
    /// sequence's and where it ends; or nothing, having written nothing
    /// that counts, for a match that reaches into this lap, or that the
    /// buffer does not hold a wide copy past. A function of its own, as
    /// only a buffer that decoding goes round has such matches.
    #[cfg_attr(
        all(feature = "intrinsics", target_arch = "x86_64"),
        multiversion::inherit_target
    )]
    #[inline(never)]
    fn behind<'l>(
        bytes: &mut [u8],
        start: usize,
        lapped: usize,
        literals: &'l [u8],
        sequence: &Sequence,
    ) -> Option<(&'l [u8], usize)> {
        let (literal_len, match_len, offset) =
            (sequence.literals, sequence.matched, sequence.offset);
        // As far before the previous lap's end as the match starts before
        // this one's start, if there is one, and if it ends there too.
        let back = offset
            .checked_sub(start)
            .filter(|&back| back >= match_len)?;
        let mut from = lapped.checked_sub(back).filter(|_| back > 0)?;
        // Neither wraps: the match is no longer than copies in turn
        // serve, and `start` is within the buffer.
        let to = start.wrapping_add(match_len);
        let mut at = start;
        while at < to {
            let matched = *bytes.get(from..)?.first_chunk::<WIDE>()?;
            *bytes.get_mut(at..)?.first_chunk_mut::<WIDE>()? = matched;
            (from, at) = (from.wrapping_add(WIDE), at.wrapping_add(WIDE));
        }
        Some((literals.get(literal_len..)?, to))
    }

    let [limit, lapped, size] = within;
    let (mut end, mut literals) = (end, literals);
    for sequence in sequences {
        if let Some((rest, to)) = wide(bytes, end, literals, sequence) {
            (literals, end) = (rest, to);
            continue;
        }
        if let Some((rest, to)) = turns(bytes, end, lapped, literals, sequence) {
            (literals, end) = (rest, to);
            continue;
        }
        let cursor = Cursor {
            bytes: &mut *bytes,
            end,
            limit,
            lapped,
            size,
        };
        end = careful(cursor, literals, sequence)?;
        literals = literals.get(sequence.literals..).ok_or_else(corrupt)?;
    }
    Ok((literals, end))
}

/// Runs `sequence` onto `cursor`, its literals from the start of
/// `literals`, as [`wide`] does, where neither it nor [`run`]'s copies in
/// turn serve, and returns where it ends: a free function, not inlined,
/// so that the loop that calls it stays in registers.
#[inline(never)]
fn careful(mut cursor: Cursor<'_>, literals: &[u8], sequence: &Sequence) -> Result<usize, Error> {
    cursor.literals(literals, sequence.literals)?;
    cursor.copy(sequence.offset, sequence.matched)?;
    Ok(cursor.end)
}

/// Appends to `bytes` from `end` the `len` bytes from `offset` back, as
/// [`Cursor::copy`] does where short copies do not serve, with its buffer
/// `lapped` to there, and returns where they end: a free function, not a
/// method, so that a cursor that calls it stays in registers.
#[inline(never)]
fn far(
    bytes: &mut [u8],
    end: usize,
    lapped: usize,
    offset: usize,
    len: usize,
) -> Result<usize, Error> {
    // From the current lap, not overlapping, at once.
    if let Some(from) = end.checked_sub(offset).filter(|_| len <= offset) {
        let (before, after) = bytes.split_at_mut(end.min(bytes.len()));
        let source = before.get(from..from.saturating_add(len));
        let target = after.get_mut(..len);
        let (Some(source), Some(target)) = (source, target) else {
            return Err(corrupt());
        };
        target.copy_from_slice(source);
        return Ok(end.saturating_add(len));
    }
    let history = end.saturating_add(lapped.saturating_sub(end));
    if offset > history {
        return Err(corrupt());
    }
    let (mut end, mut left) = (end, len);
    if let Some(back) = offset.checked_sub(end).filter(|&back| back > 0) {
        // `back` bytes from the previous lap's end, which the current
        // lap has not reached, as the window lies wholly behind it.
        let from = lapped.checked_sub(back).ok_or_else(corrupt)?;
        let n = left.min(back);
        end = within(bytes, from, end, n)?;
        left = left.saturating_sub(n);
    }
    if left == 0 {
        return Ok(end);
    }
    // What lies from `from` to the end repeats every `offset` bytes, and
    // so does when copied on, each time twice as much.
    let from = end.checked_sub(offset).ok_or_else(corrupt)?;
    while left > 0 {
        let n = left.min(end.saturating_sub(from));
        end = within(bytes, from, end, n)?;
        left = left.saturating_sub(n);
    }
    Ok(end)
}

/// Appends `n` of `bytes` from `from` to its `end`, which they must not
/// overlap, and returns where they end: a free function, not a method, so
/// that a cursor that calls it stays in registers.
fn within(bytes: &mut [u8], from: usize, end: usize, n: usize) -> Result<usize, Error> {
    let to = from.checked_add(n).ok_or_else(corrupt)?;
    let after = end.checked_add(n).ok_or_else(corrupt)?;
    if to > bytes.len() || after > bytes.len() {
        return Err(corrupt());
    }
    bytes.copy_within(from..to, end);
    Ok(after)
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
        // A window of 10 and blocks of 4: a buffer of 146, with the slack
        // of each, round which decoding goes once a block and its slack
        // may not fit.
        let mut window = Window::new(10, 4, None);
        let bytes: Vec<u8> = (0..84).collect();
        assert_eq!(decode(&mut window, &bytes), bytes);
        // Allocated whole, and zeros as far as the last block before
        // the lap might have reached.
        assert!(window.bytes.capacity() >= 10 + 2 * (4 + SLACK));
        assert_eq!(window.bytes.len(), 76 + 4 + SLACK);
        // The current lap holds 80 to 83, the previous still 4 to 79.
        assert_eq!((window.end, window.lapped), (4, 80));
        window.prepare().unwrap();
        // From the previous lap.
        copy(&mut window, 8, 2).unwrap();
        assert_eq!(window.since(4), [76, 77]);
        window.prepare().unwrap();
        // From the previous lap into the current one.
        copy(&mut window, 8, 4).unwrap();
        assert_eq!(window.since(6), [78, 79, 80, 81]);
        window.prepare().unwrap();
        // Overlapping what it copies.
        copy(&mut window, 2, 3).unwrap();
        assert_eq!(window.since(10), [80, 81, 80]);
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

    /// Runs sequences made up at random, short and long, near and far,
    /// as copying each byte in turn does: onto a window, and straight
    /// into a buffer with no room past them, where the last run with
    /// care.
    #[test]
    fn runs_sequences_as_copying_each_byte_does() {
        let mut seed = 0x9E37_79B9_7F4A_7C15_u64;
        let mut below = move |n: usize| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % n as u64) as usize
        };
        let history: Vec<u8> = (0..300_u32).map(|n| (n * 7 % 251) as u8).collect();
        let literals: Vec<u8> = (0..16384_u32).map(|n| (n * 13 % 253) as u8).collect();
        for _ in 0..300 {
            let mut expected = history.clone();
            let (mut sequences, mut taken) = (Vec::new(), 0);
            while expected.len() < 6000 {
                let literal_len = match below(16) {
                    0 => below(600),
                    1 | 2 => below(70),
                    _ => below(12),
                };
                let match_len = 3 + match below(16) {
                    0 => below(600),
                    1 | 2 => below(200),
                    _ => below(30),
                };
                let reach = expected.len() + literal_len;
                let offset = 1 + match below(4) {
                    0 => below(20),
                    1 => below(48),
                    _ => below(reach),
                };
                expected.extend_from_slice(&literals[taken..taken + literal_len]);
                for _ in 0..match_len {
                    expected.push(expected[expected.len() - offset.min(reach)]);
                }
                taken += literal_len;
                sequences.push(Sequence {
                    literals: literal_len,
                    matched: match_len,
                    offset: offset.min(reach),
                });
            }
            let mut window = Window::new(1 << 16, 1 << 13, None);
            window.prepare().unwrap();
            window.extend(&history).unwrap();
            let mut cursor = window.cursor(1 << 13);
            let rest = cursor.run(&sequences, &literals).unwrap();
            assert_eq!(rest.len(), literals.len() - taken);
            let end = cursor.end;
            window.advance(end);
            assert_eq!(window.since(0), expected);

            let mut whole = vec![0; expected.len()];
            whole[..history.len()].copy_from_slice(&history);
            let max = expected.len() - history.len();
            let mut cursor = Cursor::over(&mut whole, history.len(), max, 1 << 16);
            cursor.run(&sequences, &literals[..taken]).unwrap();
            assert_eq!(cursor.end, expected.len());
            assert_eq!(whole, expected);
            // One byte more than the limit allows.
            let more = Sequence {
                literals: 0,
                matched: 1,
                offset: 1,
            };
            let mut cursor = window.cursor(0);
            assert!(cursor.run(&[more], &literals).is_err());
        }
    }

    /// Blocks of sequences made up at random, and blocks stored as they
    /// are and of one byte repeated between them, round a buffer of few
    /// blocks, lap after lap, as copying each byte in turn does: whether
    /// the frame says what it decodes to or not.
    #[test]
    fn runs_blocks_across_laps_as_copying_each_byte_does() {
        let mut seed = 0x2545_F491_4F6C_DD1D_u64;
        let mut below = move |n: usize| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % n as u64) as usize
        };
        let literals: Vec<u8> = (0..4096_u32).map(|n| (n * 13 % 253) as u8).collect();
        let (size, block) = (1024, 256);
        for total in [200, 1000, 1500, 1700, 40_000] {
            for says in [false, true] {
                let mut window = Window::new(size, block, says.then_some(total));
                let (mut expected, mut produced) = (Vec::new(), Vec::new());
                while expected.len() < total {
                    let left = (total - expected.len()).min(block);
                    let start = expected.len();
                    match below(8) {
                        0 => {
                            let stored: Vec<u8> = (0..1 + below(left)).map(|n| n as u8).collect();
                            expected.extend_from_slice(&stored);
                            assert_eq!(window.append(&stored).unwrap(), window.end - stored.len());
                        },
                        1 => {
                            let n = 1 + below(left);
                            expected.resize(start + n, 0xAA);
                            assert_eq!(window.repeat(0xAA, n).unwrap(), window.end - n);
                        },
                        _ => {
                            let (mut sequences, mut taken) = (Vec::new(), 0);
                            while expected.len() - start + 40 < left {
                                let literal_len = below(12);
                                let match_len = 3 + below(24);
                                let reach = (expected.len() + literal_len).min(size);
                                if reach == 0 {
                                    expected.push(literals[taken]);
                                    taken += 1;
                                    sequences.push(Sequence {
                                        literals: 1,
                                        matched: 0,
                                        offset: 1,
                                    });
                                    continue;
                                }
                                let offset = 1 + match below(4) {
                                    0 => below(20).min(reach - 1),
                                    _ => below(reach),
                                };
                                expected.extend_from_slice(&literals[taken..taken + literal_len]);
                                for _ in 0..match_len {
                                    expected.push(expected[expected.len() - offset]);
                                }
                                taken += literal_len;
                                sequences.push(Sequence {
                                    literals: literal_len,
                                    matched: match_len,
                                    offset,
                                });
                            }
                            window.prepare().unwrap();
                            let mut cursor = window.cursor(block);
                            cursor.run(&sequences, &literals).unwrap();
                            let end = cursor.end;
                            window.advance(end);
                        },
                    }
                    let mut output = vec![0; expected.len() - start];
                    assert_eq!(window.produce(&mut output), output.len());
                    assert!(!window.pending());
                    produced.extend_from_slice(&output);
                }
                assert_eq!(produced, expected, "{total} said: {says}");
                // No more than a frame that the buffer holds whole says,
                // which then takes no more than that, nor than a block.
                if says && total + SLACK <= size + 2 * (block + SLACK) {
                    assert!(window.append(&[0]).is_err() && window.repeat(0, 1).is_err());
                    assert_eq!((window.lapped, window.end), (0, total));
                    assert!(window.bytes.len() <= total + SLACK);
                } else if total > size + 2 * (block + SLACK) {
                    assert!(window.lapped > 0);
                }
                assert!(window.append(&vec![0; block + 1]).is_err());
            }
        }
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

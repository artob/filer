// This is free and unencumbered software released into the public domain.

//! The dictionary: what a stream has decoded, which matches copy from, as
//! far back as its size, in a buffer that wraps around once it is that
//! large, and that grows to it only as the stream does.

use crate::model::LONGEST;
use alloc::vec::Vec;

/// The most a decoder decodes before it produces what it decoded.
const CHUNK: usize = 64 * 1024;

/// The least a buffer grows by.
const LEAST: usize = 4096;

#[derive(Clone, Debug)]
pub(crate) struct Window {
    buf: Vec<u8>,
    /// The most the buffer holds: the dictionary's size, or the stream's,
    /// if smaller.
    max: usize,
    /// Whether the stream is no larger than `max`, so that the buffer
    /// never wraps around past bytes not yet produced.
    bounded: bool,
    /// Where the next byte goes.
    pos: usize,
    /// How many of the bytes before `pos` are not yet produced.
    pending: usize,
    /// How many bytes have been decoded since the dictionary was last
    /// reset: how far back a match may reach, up to the dictionary's size,
    /// and the position that packets are coded by.
    pub(crate) processed: u64,
    /// The dictionary's size.
    dictionary: u32,
    /// The latest byte, which literals are coded by.
    pub(crate) last: u8,
    /// How many bytes it may take before `ready` must look again: what
    /// it last found, less what it has taken since.
    room: usize,
}

impl Window {
    /// Returns an empty window of a dictionary of `dictionary` bytes, for a
    /// stream of `size` bytes, if known.
    pub(crate) fn new(dictionary: u32, size: Option<u64>) -> Self {
        let full = usize::try_from(dictionary).unwrap_or(usize::MAX);
        let (max, bounded) = match size.and_then(|size| usize::try_from(size).ok()) {
            Some(size) if size <= full => (size, true),
            _ => (full, false),
        };
        Self {
            buf: Vec::new(),
            max,
            bounded,
            pos: 0,
            pending: 0,
            processed: 0,
            dictionary,
            last: 0,
            room: 0,
        }
    }

    /// Forgets what was decoded, as far as matches are concerned.
    pub(crate) fn reset(&mut self) {
        self.processed = 0;
    }

    /// Returns how many bytes are decoded and not yet produced.
    pub(crate) const fn pending(&self) -> usize {
        self.pending
    }

    /// Makes room for a packet, of a match as long as the longest, and
    /// returns whether there is: none while it holds a chunk or as much as
    /// it can, until they are produced.
    #[inline(always)]
    pub(crate) fn ready(&mut self) -> bool {
        self.room >= LONGEST || self.look()
    }

    /// Makes room, as `ready` does, and finds how much there is.
    #[cold]
    fn look(&mut self) -> bool {
        let need = self.pos.saturating_add(LONGEST);
        let len = self.buf.len();
        if len < self.max && need > len {
            let grown = need.max(len.saturating_mul(2)).max(LEAST).min(self.max);
            self.buf.resize(grown, 0);
        }
        let len = self.buf.len();
        // Up to the buffer's end, until it is as large as it gets; then
        // around it, up to the bytes not yet produced, unless the stream
        // ends first.
        let space = if len < self.max {
            len.saturating_sub(self.pos)
        } else if self.bounded {
            usize::MAX
        } else {
            len.saturating_sub(self.pending)
        };
        self.room = CHUNK.saturating_sub(self.pending).min(space);
        self.room >= LONGEST
    }

    /// Returns whether a match may reach back `distance`, less one: within
    /// both the dictionary and what was decoded since it was reset.
    pub(crate) fn reaches(&self, distance: u32) -> bool {
        distance < self.dictionary && u64::from(distance) < self.processed
    }

    /// Returns the byte `distance` back, from 1, if it holds it.
    pub(crate) fn byte(&self, distance: usize) -> Option<u8> {
        let at = match self.pos.checked_sub(distance) {
            Some(at) => at,
            None => self
                .buf
                .len()
                .checked_sub(distance.wrapping_sub(self.pos))?,
        };
        self.buf.get(at).copied()
    }

    /// Appends `byte`.
    pub(crate) fn put(&mut self, byte: u8) {
        if let Some(slot) = self.buf.get_mut(self.pos) {
            *slot = byte;
        }
        self.last = byte;
        self.advance(1);
    }

    /// Moves past `n` bytes just written, wrapping around once the buffer
    /// is as large as it gets; until then, `ready` grows it first.
    fn advance(&mut self, n: usize) {
        self.pos = self.pos.wrapping_add(n);
        if self.pos >= self.buf.len() && self.buf.len() >= self.max {
            self.pos = self.pos.wrapping_sub(self.buf.len());
        }
        self.pending = self.pending.wrapping_add(n);
        self.room = self.room.saturating_sub(n);
        self.processed = self
            .processed
            .wrapping_add(u64::try_from(n).unwrap_or(u64::MAX));
    }

    /// Appends `length` bytes from `distance` back, from 1, which it holds;
    /// they may overlap those they copy, repeating them.
    pub(crate) fn copy(&mut self, distance: usize, length: usize) {
        let len = self.buf.len();
        if let Some(from) = self.pos.checked_sub(distance)
            && self.pos.saturating_add(length) < len
        {
            let end = self.pos.wrapping_add(length);
            if distance >= length {
                self.buf
                    .copy_within(from..from.wrapping_add(length), self.pos);
            } else if distance == 1 {
                // A run of the latest byte.
                if let Some(run) = self.buf.get_mut(self.pos..end) {
                    run.fill(self.last);
                }
            } else if let Some(span) = self.buf.get_mut(from..end) {
                // Repeating what it copies: a period at a time, each
                // after the last is written, or byte by byte, for short
                // periods.
                if distance >= 8 {
                    let mut at = distance;
                    while at < span.len() {
                        let n = distance.min(span.len().wrapping_sub(at));
                        let start = at.wrapping_sub(distance);
                        span.copy_within(start..start.wrapping_add(n), at);
                        at = at.wrapping_add(n);
                    }
                } else {
                    for at in distance..span.len() {
                        if let Some(&byte) = span.get(at.wrapping_sub(distance))
                            && let Some(slot) = span.get_mut(at)
                        {
                            *slot = byte;
                        }
                    }
                }
            }
            let end = self.pos.wrapping_add(length).wrapping_sub(1);
            self.last = self.buf.get(end).copied().unwrap_or_default();
            self.advance(length);
            return;
        }
        for _ in 0..length {
            let byte = self.byte(distance).unwrap_or_default();
            self.put(byte);
        }
    }

    /// Appends as much of `bytes` as it has room for, up to a chunk not yet
    /// produced, and returns how much.
    pub(crate) fn write(&mut self, bytes: &[u8]) -> usize {
        let mut written = 0;
        while written < bytes.len() && self.ready() {
            let room = self.buf.len().saturating_sub(self.pos);
            let n = room.min(LONGEST).min(bytes.len().saturating_sub(written));
            let (Some(to), Some(from)) = (
                self.buf.get_mut(self.pos..self.pos.saturating_add(n)),
                bytes.get(written..written.saturating_add(n)),
            ) else {
                break;
            };
            to.copy_from_slice(from);
            self.last = from.last().copied().unwrap_or(self.last);
            self.advance(n);
            written = written.saturating_add(n);
        }
        written
    }

    /// Produces what it can of the bytes not yet produced into `output`,
    /// and returns how much.
    pub(crate) fn produce(&mut self, output: &mut [u8]) -> usize {
        let mut produced = 0;
        while self.pending > 0 && produced < output.len() {
            let start = match self.pos.checked_sub(self.pending) {
                Some(start) => start,
                None => self
                    .buf
                    .len()
                    .saturating_sub(self.pending.wrapping_sub(self.pos)),
            };
            let run = self
                .pending
                .min(self.buf.len().saturating_sub(start))
                .min(output.len().saturating_sub(produced));
            let (Some(to), Some(from)) = (
                output.get_mut(produced..produced.saturating_add(run)),
                self.buf.get(start..start.saturating_add(run)),
            ) else {
                break;
            };
            to.copy_from_slice(from);
            self.pending = self.pending.saturating_sub(run);
            produced = produced.saturating_add(run);
        }
        produced
    }
}

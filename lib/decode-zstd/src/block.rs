// This is free and unencumbered software released into the public domain.

//! Compressed blocks (RFC 8878 §3.1.1.3): literals, then the sequences
//! that interleave them with copies of what came before.

use super::{
    huffman::{Huffman, Runs},
    literals,
    sequences::{self, Codes},
    window::Cursor,
};
use crate::Error;
use alloc::vec::Vec;

/// What a frame's compressed blocks pass on, each to the next: the last
/// Huffman table, and their sequences' tables and offsets.
#[derive(Clone, Debug, Default)]
pub(super) struct Tables {
    huffman: Option<Huffman>,
    runs: Runs,
    codes: Codes,
}

impl Tables {
    /// Returns the bytes it holds, allocated.
    pub(super) fn footprint(&self) -> usize {
        self.runs.footprint().saturating_add(self.codes.footprint())
    }
}

/// Decodes a compressed block's `content` onto `output`, of at most `max`
/// literals, with `scratch` to decode them into.
pub(super) fn decode(
    content: &[u8],
    tables: &mut Tables,
    scratch: &mut Vec<u8>,
    output: &mut Cursor<'_>,
    max: usize,
) -> Result<(), Error> {
    let (literals, len, used) =
        literals::read(content, &mut tables.huffman, &mut tables.runs, scratch, max)?;
    let section = content.get(used..).unwrap_or_default();
    if ahead() {
        touch(section);
    }
    sequences::decode(section, &mut tables.codes, literals, len, output)
}

/// Returns whether a block's sequences are to be loaded ahead, a byte of
/// each cache line from their start on, before they are read: on AMD's
/// cores, told by SSE4a, which only they have, with `intrinsics`, as the
/// CPU is told at run time with `std`, else where the build enables it.
///
/// A block's sequences are read from their end back, which Zen 5 asks
/// memory for ahead of less well than from their start on, where no
/// cache holds them, as where a block is decoded straight from where a
/// caller holds it. Loaded so, the corpus decodes there 0.4% faster
/// whole, 4% with a window of 128 MiB, and `webster` at level 22 11%;
/// and in pieces of 64 KiB, 1.3% faster at level 19. Granite Rapids and
/// Neoverse V3 decoded 0.4 to 1.2% slower so, as it put out of their
/// first caches what they would have had there; and Zen 1 decodes
/// neither faster nor slower so beyond a few percent, as much as its
/// timings vary (2026-11-12).
fn ahead() -> bool {
    #[cfg(all(feature = "intrinsics", feature = "std", target_arch = "x86_64"))]
    {
        std::arch::is_x86_feature_detected!("sse4a")
    }
    #[cfg(all(feature = "intrinsics", not(feature = "std"), target_arch = "x86_64"))]
    {
        cfg!(target_feature = "sse4a")
    }
    #[cfg(not(all(feature = "intrinsics", target_arch = "x86_64")))]
    {
        false
    }
}

/// The bytes of a cache line.
const LINE: usize = 64;

/// Loads a byte of each cache line of `bytes`, from their start on, for
/// the loads' sake: a function of its own, not inlined, so that they are
/// all under way before what follows waits on any.
#[inline(never)]
fn touch(bytes: &[u8]) {
    let mut loaded = 0_u8;
    for line in bytes.chunks(LINE) {
        loaded ^= line.first().copied().unwrap_or_default();
    }
    core::hint::black_box(loaded);
}

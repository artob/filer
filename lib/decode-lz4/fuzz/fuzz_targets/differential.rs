// This is free and unencumbered software released into the public domain.

//! LZ4 streams, which must decode as `lz4_flex` decodes them: to the same
//! bytes, or not at all, but for a stream cut short between blocks, which
//! it takes to end there, and legacy frames it reads more leniently than
//! `lz4`. Seed the corpus with real ones, such as those in `tests/data`.

#![no_main]

#[path = "../../tests/common/mod.rs"]
mod common;

use libfuzzer_sys::fuzz_target;

/// A legacy frame's magic number, little-endian.
const LEGACY: [u8; 4] = [0x02, 0x21, 0x4C, 0x18];

/// Returns whether `frame`, a legacy frame, has what `lz4_flex` reads and
/// `lz4` refuses: a block size with its high bit set, which `lz4_flex`
/// takes as a stored block's, though legacy frames have none; or one to
/// three bytes after a block, which it takes as the frame's end.
fn lenient(frame: &[u8]) -> bool {
    let Some(mut rest) = frame.strip_prefix(&LEGACY) else {
        return false;
    };
    loop {
        let Some((&size, blocks)) = rest.split_first_chunk::<4>() else {
            return !rest.is_empty();
        };
        let size = u32::from_le_bytes(size);
        if size >> 31 == 1 {
            return true;
        }
        let Some(next) = usize::try_from(size)
            .ok()
            .and_then(|size| blocks.get(size..))
        else {
            return false;
        };
        if size == 0 || next.is_empty() {
            return false;
        }
        rest = next;
    }
}

fuzz_target!(|stream: &[u8]| {
    let piece = stream.len().max(1);
    let ours = common::decode(stream, piece).ok();
    let theirs = common::lz4_flex(stream);
    let cut = || common::unfinished(stream, piece).ok().map(|(data, _)| data);
    // `lz4_flex` takes four bytes alone, whatever they are, to hold
    // nothing; `lz4` refuses four that begin no frame, as this crate does.
    let four = stream.len() == 4 && theirs.as_ref().is_some_and(Vec::is_empty);
    let legacy = || (0..stream.len()).any(|at| stream.get(at..).is_some_and(lenient));
    assert!(
        ours == theirs || (ours.is_none() && (four || cut() == theirs || legacy())),
        "disagrees with lz4_flex"
    );
});

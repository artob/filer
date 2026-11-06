// This is free and unencumbered software released into the public domain.

//! LZ4 streams, which must decode as `lz4_flex` decodes them: to the same
//! bytes, or not at all, but where it reads them otherwise than `lz4`.
//! It takes a stream cut short between blocks to end there, and a block
//! that decodes to nothing as the end; it reads legacy frames leniently,
//! and no frame after one, nor any skippable frame. Seed the corpus with
//! real ones, such as those in `tests/data`.

#![no_main]

#[path = "../../tests/common/mod.rs"]
mod common;

use libfuzzer_sys::fuzz_target;
use std::io::Read;

/// A legacy frame's magic number, little-endian.
const LEGACY: u32 = 0x184C_2102;

/// Returns whether `word` is a legacy or skippable frame's magic number,
/// which begins a frame that `lz4_flex` refuses, or what follows one.
fn refused(word: u32) -> bool {
    word == LEGACY || word >> 4 == 0x0184_D2A5
}

/// Returns whether `frame`, a legacy frame, has what `lz4_flex` reads and
/// `lz4` refuses: a block size with its high bit set, which `lz4_flex`
/// takes as a stored block's, though legacy frames have none; or one to
/// three bytes after a block, which it takes as the frame's end.
fn lenient(frame: &[u8]) -> bool {
    let Some((&magic, mut rest)) = frame.split_first_chunk::<4>() else {
        return false;
    };
    if u32::from_le_bytes(magic) != LEGACY {
        return false;
    }
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

/// Decodes `stream` through `lz4_flex` as `common::lz4_flex` does, but
/// reads on past a read of nothing: it reads a block that decodes to
/// nothing, such as a legacy frame's of one zero byte, as the end.
fn read_on(stream: &[u8]) -> Option<Vec<u8>> {
    let (mut output, mut rest) = (Vec::new(), stream);
    let mut decoder = lz4_flex::frame::FrameDecoder::new(&mut rest);
    loop {
        let left = decoder.get_ref().len();
        let room = common::CAP.saturating_sub(output.len()) as u64;
        (&mut decoder).take(room).read_to_end(&mut output).ok()?;
        if decoder.get_ref().len() == left || output.len() >= common::CAP {
            break;
        }
    }
    rest.is_empty().then_some(output)
}

fuzz_target!(|stream: &[u8]| {
    let piece = stream.len().max(1);
    let ours = common::decode(stream, piece).ok();
    let theirs = common::lz4_flex(stream);
    let cut = || common::unfinished(stream, piece).ok().map(|(data, _)| data);
    // `lz4_flex` takes four bytes alone, whatever they are, to hold
    // nothing; `lz4` refuses four that begin no frame, as this crate does.
    let four = stream.len() == 4 && theirs.as_ref().is_some_and(Vec::is_empty);
    let has_lenient = || (0..stream.len()).any(|at| lenient(stream.get(at..).unwrap_or_default()));
    let has_refused = || {
        let words = stream.windows(4).filter_map(|word| word.try_into().ok());
        words.map(u32::from_le_bytes).any(refused)
    };
    assert!(
        ours == theirs
            || (ours.is_none() && (four || cut() == theirs || has_lenient()))
            || (theirs.is_none() && (has_refused() || ours.is_some() && ours == read_on(stream))),
        "disagrees with lz4_flex"
    );
});

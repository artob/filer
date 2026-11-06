// This is free and unencumbered software released into the public domain.

//! LZ4 streams, which must decode as `lz4_flex` decodes them: to the same
//! bytes, or not at all, but for a stream cut short between blocks, which
//! it takes to end there, a block that decodes to nothing, which it
//! takes as the end, and legacy frames it reads otherwise than `lz4`.
//! Seed the corpus with real ones, such as those in `tests/data`.

#![no_main]

#[path = "../../tests/common/mod.rs"]
mod common;

use libfuzzer_sys::fuzz_target;
use std::io::Read;

/// A legacy frame's magic number, little-endian.
const LEGACY: [u8; 4] = [0x02, 0x21, 0x4C, 0x18];

/// What `lz4_flex` reads of a legacy frame otherwise than `lz4` does.
#[derive(Clone, Copy, PartialEq)]
enum Legacy {
    /// Nothing.
    Alike,
    /// What `lz4` refuses: a block size with its high bit set, which
    /// `lz4_flex` takes as a stored block's, though legacy frames have
    /// none; or one to three bytes after a block, which it takes as the
    /// frame's end.
    Lenient,
    /// Another frame after it, which `lz4_flex` refuses, taking its magic
    /// number as a block size, and `lz4` reads.
    Followed,
}

/// Returns what `lz4_flex` reads of `frame`, a legacy frame, otherwise.
fn legacy(frame: &[u8]) -> Legacy {
    let Some(mut rest) = frame.strip_prefix(&LEGACY) else {
        return Legacy::Alike;
    };
    loop {
        let Some((&size, blocks)) = rest.split_first_chunk::<4>() else {
            return if rest.is_empty() {
                Legacy::Alike
            } else {
                Legacy::Lenient
            };
        };
        let size = u32::from_le_bytes(size);
        if size == 0x184D_2204 || size == u32::from_le_bytes(LEGACY) || size >> 4 == 0x0184_D2A5 {
            return Legacy::Followed;
        }
        if size >> 31 == 1 {
            return Legacy::Lenient;
        }
        let Some(next) = usize::try_from(size)
            .ok()
            .and_then(|size| blocks.get(size..))
        else {
            return Legacy::Alike;
        };
        if size == 0 || next.is_empty() {
            return Legacy::Alike;
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
    let has = |kind| (0..stream.len()).any(|at| stream.get(at..).map(legacy) == Some(kind));
    assert!(
        ours == theirs
            || (ours.is_none() && (four || cut() == theirs || has(Legacy::Lenient)))
            || (theirs.is_none()
                && (has(Legacy::Followed) || ours.is_some() && ours == read_on(stream))),
        "disagrees with lz4_flex"
    );
});

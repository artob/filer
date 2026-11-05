// This is free and unencumbered software released into the public domain.

//! LZ4 streams, which must decode as `lz4_flex` decodes them: to the same
//! bytes, or not at all, but for a stream cut short between blocks, which
//! it takes to end there. Seed the corpus with real ones, such as those
//! in `tests/data`.

#![no_main]

#[path = "../../tests/common/mod.rs"]
mod common;

use libfuzzer_sys::fuzz_target;

fuzz_target!(|stream: &[u8]| {
    let piece = stream.len().max(1);
    let ours = common::decode(stream, piece).ok();
    let theirs = common::lz4_flex(stream);
    let cut = || common::unfinished(stream, piece).ok().map(|(data, _)| data);
    // `lz4_flex` takes four bytes alone, whatever they are, to hold
    // nothing; `lz4` refuses four that begin no frame, as this crate does.
    let four = stream.len() == 4 && theirs.as_ref().is_some_and(Vec::is_empty);
    assert!(
        ours == theirs || (ours.is_none() && (four || cut() == theirs)),
        "disagrees with lz4_flex"
    );
});

// This is free and unencumbered software released into the public domain.

//! bzip2 streams, which must decode as `bzip2` decodes them: to the same
//! bytes, or not at all, but for randomized blocks, which this crate finds
//! unsupported (`common::agrees_with_bzip2`). Seed the corpus with real
//! ones, such as those in `tests/data`.

#![no_main]

#[path = "../../tests/common/mod.rs"]
mod common;

use libfuzzer_sys::fuzz_target;

fuzz_target!(|stream: &[u8]| {
    let ours = common::decode(stream, stream.len().max(1));
    assert!(
        common::agrees_with_bzip2(stream, &ours),
        "disagrees with bzip2"
    );
});

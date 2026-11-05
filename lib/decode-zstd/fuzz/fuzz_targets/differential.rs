// This is free and unencumbered software released into the public domain.

//! zstd streams, which must decode as `ruzstd` decodes them: to the same
//! bytes, or not at all, but for the frames zstd refuses and `ruzstd`
//! does not (`common::agrees_with_ruzstd`), which this crate refuses too.
//! Seed the corpus with real ones, such as those in `tests/data`.

#![no_main]

#[path = "../../tests/common/mod.rs"]
mod common;

use libfuzzer_sys::fuzz_target;

fuzz_target!(|stream: &[u8]| {
    let ours = common::decode(stream, stream.len().max(1)).ok();
    assert!(
        common::agrees_with_ruzstd(stream, ours.as_deref()),
        "disagrees with ruzstd"
    );
});

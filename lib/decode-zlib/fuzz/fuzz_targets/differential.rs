// This is free and unencumbered software released into the public domain.

//! zlib streams, which must decode as zlib decodes them, through
//! `zlib-rs`: to the same bytes, consuming the same, or not at all. Seed
//! the corpus with real ones, such as those in `tests/data`.

#![no_main]

#[path = "../../tests/common/mod.rs"]
mod common;

use libfuzzer_sys::fuzz_target;

fuzz_target!(|stream: &[u8]| {
    let ours = common::decode(stream, stream.len().max(1)).ok();
    assert!(ours == common::zlib(stream), "disagrees with zlib");
});

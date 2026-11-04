// This is free and unencumbered software released into the public domain.

//! xz files: what this crate refuses, `lzma-rust2` must refuse too, and
//! what it decodes, `lzma-rust2` must decode to the same bytes, or refuse,
//! as it refuses chains of two filters or more, which xz decodes. Seed the
//! corpus with real ones, such as those in `tests/data`.

#![no_main]

#[path = "../../tests/common/mod.rs"]
mod common;

use libfuzzer_sys::fuzz_target;

fuzz_target!(|stream: &[u8]| {
    let ours = common::decode(stream, stream.len().max(1)).ok().map(|(data, _)| data);
    let theirs = common::theirs(stream);
    let chained = ours.is_some() && theirs.is_none();
    assert!(chained || ours == theirs, "disagrees with lzma-rust2");
});

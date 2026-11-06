// This is free and unencumbered software released into the public domain.

//! LZMA and LZMA2 streams, framed as `input.rs` says, which must decode as
//! `lzma-rust2` decodes them: to the same bytes, or not at all; but for
//! LZMA of a known size, which `lzma-rust2` may decode where this crate,
//! as the LZMA SDK, refuses, and LZMA of a dictionary larger than
//! `lzma-rust2` takes.

#![no_main]

#[path = "../../tests/common/mod.rs"]
mod common;
mod input;

use input::Kind;
use libfuzzer_sys::fuzz_target;

/// The largest dictionary `lzma-rust2` takes, though LZMA's properties
/// allow any up to `u32::MAX`, as the LZMA SDK takes.
const DICTIONARY: u32 = 0xFFFF_FFF0;

fuzz_target!(|data: &[u8]| {
    let Some((kind, _, stream)) = input::parse(data) else {
        return;
    };
    let ours = common::decode(|| kind.decoder(), stream, stream.len().max(1)).ok();
    let theirs = match kind {
        Kind::Lzma(properties, size) => common::theirs_lzma(stream, properties, size),
        Kind::Lzma2(property) => common::theirs_lzma2(stream, property),
    };
    let data = |decoded: &Option<(Vec<u8>, usize)>| decoded.as_ref().map(|(data, _)| data.clone());
    let lax = matches!(kind, Kind::Lzma(_, Some(_))) && ours.is_none();
    let large = matches!(kind, Kind::Lzma([_, dictionary @ ..], _)
        if u32::from_le_bytes(dictionary) > DICTIONARY && theirs.is_none());
    assert!(
        lax || large || data(&ours) == data(&theirs),
        "disagrees with lzma-rust2"
    );
});

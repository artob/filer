// This is free and unencumbered software released into the public domain.

//! LZMA and LZMA2 streams, framed as `input.rs` says, which must decode as
//! `lzma-rust2` decodes them: to the same bytes, or not at all; but for
//! LZMA of a known size, which `lzma-rust2` may decode where this crate,
//! as the LZMA SDK, refuses.

#![no_main]

#[path = "../../tests/common/mod.rs"]
mod common;
mod input;

use input::Kind;
use libfuzzer_sys::fuzz_target;

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
    assert!(lax || data(&ours) == data(&theirs), "disagrees with lzma-rust2");
});

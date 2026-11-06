// This is free and unencumbered software released into the public domain.

//! LZMA and LZMA2 streams, framed as `input.rs` says, which must decode
//! in pieces as the whole does, and never panic.

#![no_main]

#[path = "../../tests/common/mod.rs"]
mod common;
#[path = "../input.rs"]
mod input;

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Some((kind, piece, stream)) = input::parse(data) else {
        return;
    };
    // Any error is one of the two kinds; what matters is that pieces and
    // the whole agree, and that nothing panics.
    let _ = common::agree(|| kind.decoder(), stream, piece);
});

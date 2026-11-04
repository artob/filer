// This is free and unencumbered software released into the public domain.

//! gzip streams: the first byte sets the size of the pieces the decoder
//! is fed in, which must decode as the whole does; the rest is the
//! stream. Seed the corpus with real ones, such as those in `tests/data`,
//! each after a byte.

#![no_main]

#[path = "../../tests/common/mod.rs"]
mod common;

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Some((&piece, stream)) = data.split_first() else {
        return;
    };
    // Any error is `Corrupt`; what matters is that pieces and the whole
    // agree, and that nothing panics.
    let _ = common::agree(stream, usize::from(piece % 64) + 1);
});

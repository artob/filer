// This is free and unencumbered software released into the public domain.

//! Whole archives: the input is the archive. Seed the corpus with real
//! ones, such as `tests/data/hello.zip`.

#![no_main]

#[path = "../../tests/common/mod.rs"]
mod common;

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    common::exercise::exercise(data.to_vec());
});

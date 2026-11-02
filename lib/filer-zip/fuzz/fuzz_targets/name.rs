// This is free and unencumbered software released into the public domain.

//! Member names: the input names a file, a directory, and a symlink in an
//! otherwise valid archive, so that name checks and implied directories
//! see arbitrary bytes.

#![no_main]

#[path = "../../tests/common/mod.rs"]
mod common;

use common::Writer;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let name = &data[..data.len().min(0xFFFF)];
    let mut dir = name.to_vec();
    dir.push(b'/');
    let archive = Writer::default()
        .file(name, b"x")
        .dir(&dir)
        .symlink(name, b"../outside")
        .finish();
    common::exercise::exercise(archive);
});

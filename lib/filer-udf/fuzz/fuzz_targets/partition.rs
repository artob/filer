// This is free and unencumbered software released into the public domain.

//! The partition's first blocks, of the file set, the file entries, and
//! the directories: the input replaces them.

#![no_main]

#[path = "../../tests/common/mod.rs"]
mod common;
mod template;

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let template = template::plain();
    common::exercise(template::overlay(template, template.partition, data));
});

// This is free and unencumbered software released into the public domain.

//! A file zisofs compresses: the input replaces its header and block
//! pointers.

#![no_main]

#[path = "../../tests/common/mod.rs"]
mod common;
mod template;

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let template = template::zisofs();
    common::exercise(template::overlay(template, template.file, data));
});

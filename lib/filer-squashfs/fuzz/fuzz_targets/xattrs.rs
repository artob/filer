// This is free and unencumbered software released into the public domain.

//! The extended attributes' names and values: the input replaces them.

#![no_main]

#[path = "../../tests/common/mod.rs"]
mod common;
mod template;

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let template = template::attributed();
    common::exercise(template::overlay(template, template.xattrs, data));
});

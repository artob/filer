// This is free and unencumbered software released into the public domain.

//! Directories: the input, after its first byte, replaces the root
//! directory of a valid volume, and what follows it.

#![no_main]

#[path = "../../tests/common/mod.rs"]
mod common;
mod template;

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Some((&selector, data)) = data.split_first() else {
        return;
    };
    let template = template::template(selector);
    common::exercise::exercise(template::overlay(template, template.root, data));
});

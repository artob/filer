// This is free and unencumbered software released into the public domain.

//! Directory indexes: the input replaces the first index record of a
//! valid volume.

#![no_main]

#[path = "../../tests/common/mod.rs"]
mod common;
mod template;

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let template = template::plain();
    common::exercise::exercise(template::overlay(template, template.index, data));
});

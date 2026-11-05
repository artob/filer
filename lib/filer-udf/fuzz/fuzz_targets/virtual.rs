// This is free and unencumbered software released into the public domain.

//! A write-once volume's last blocks, of its virtual allocation table:
//! the input replaces them.

#![no_main]

#[path = "../../tests/common/mod.rs"]
mod common;
mod template;

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let template = template::write_once();
    common::exercise(template::overlay(template, template.last, data));
});

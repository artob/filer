// This is free and unencumbered software released into the public domain.

//! Compressed files: the input replaces the first compressed chunk of a
//! valid volume.

#![no_main]

#[path = "../../tests/common/mod.rs"]
mod common;
mod template;

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let template = template::compressed();
    common::exercise::exercise(template::overlay(template, template.compressed, data));
});

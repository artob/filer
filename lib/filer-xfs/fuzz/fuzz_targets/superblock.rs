// This is free and unencumbered software released into the public domain.

//! The superblock: the input replaces it.

#![no_main]

#[path = "../../tests/common/mod.rs"]
mod common;
mod template;

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let template = template::attrs();
    common::exercise::exercise(template::overlay(template, template.superblock, data));
});

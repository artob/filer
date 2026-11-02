// This is free and unencumbered software released into the public domain.

//! The log of a volume needing recovery: the input replaces its start.

#![no_main]

#[path = "../../tests/common/mod.rs"]
mod common;
mod template;

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let template = template::dirty();
    common::exercise::exercise(template::overlay(template, template.log, data));
});

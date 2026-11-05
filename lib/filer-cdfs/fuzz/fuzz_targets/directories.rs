// This is free and unencumbered software released into the public domain.

//! The root directory, Rock Ridge's entries and all: the input replaces
//! its first records.

#![no_main]

#[path = "../../tests/common/mod.rs"]
mod common;
mod template;

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let template = template::rock_ridge();
    common::exercise(template::overlay(template, template.root, data));
});

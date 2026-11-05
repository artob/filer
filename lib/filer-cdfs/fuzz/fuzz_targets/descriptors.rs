// This is free and unencumbered software released into the public domain.

//! The volume descriptor set, from sector 16: the input replaces it.

#![no_main]

#[path = "../../tests/common/mod.rs"]
mod common;
mod template;

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let template = template::rock_ridge();
    common::exercise(template::overlay(template, template::DESCRIPTORS, data));
});

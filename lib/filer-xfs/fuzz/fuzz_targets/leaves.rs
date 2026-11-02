// This is free and unencumbered software released into the public domain.

//! A directory leaf block, below nodes: the input replaces the first.

#![no_main]

#[path = "../../tests/common/mod.rs"]
mod common;
mod template;

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let template = template::dirs();
    common::exercise(template::overlay(template, template.leaves, data));
});

// This is free and unencumbered software released into the public domain.

//! The root directory of Joliet's tree: the input replaces its first
//! records.

#![no_main]

#[path = "../../tests/common/mod.rs"]
mod common;
mod template;

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let template = template::joliet();
    common::exercise(template::overlay(template, template.root, data));
});

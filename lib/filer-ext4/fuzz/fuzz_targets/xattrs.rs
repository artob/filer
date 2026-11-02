// This is free and unencumbered software released into the public domain.

//! An extended attribute block: the input replaces it.

#![no_main]

#[path = "../../tests/common/mod.rs"]
mod common;
mod template;

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let template = template::plain();
    common::exercise::exercise(template::overlay(template, template.xattrs, data));
});

// This is free and unencumbered software released into the public domain.

//! Reparse points, as `FSCTL_GET_REPARSE_POINT` returns them: the input is
//! the whole value. Every link must have a target Filer can follow.

#![no_main]

use filer_fs::reparse::Reparse;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Some(Reparse::Link(target)) = Reparse::parse(data) {
        assert!(!target.is_empty() && !target.contains(&0));
    }
});

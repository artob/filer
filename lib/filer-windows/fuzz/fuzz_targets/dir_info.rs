// This is free and unencumbered software released into the public domain.

//! Directory entries, as `NtQueryDirectoryFile` returns them: the input is
//! one call's buffer, read in each class. Every name must survive the
//! round trip through WTF-8 that listing and opening make.

#![no_main]

#[path = "../../src/dir_info.rs"]
mod dir_info;

use dir_info::{Class, parse};
use filer_msft::wtf8;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    for class in [Class::Extd, Class::Both] {
        for entry in parse(data, class).unwrap_or_default() {
            // Every field reads, whatever the bytes.
            let _ = (entry.is_dot(), entry.attributes, entry.tag, entry.id);
            assert_eq!(wtf8::decode(&wtf8::encode(&entry.name)), Some(entry.name));
        }
    }
});

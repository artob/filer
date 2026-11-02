// This is free and unencumbered software released into the public domain.

//! Deflate data: the input is the packed bytes of one member of a valid
//! archive, declared to decode to 64 KiB, so that decoding, its bounds,
//! and the CRC check see arbitrary streams.

#![no_main]

#[path = "../../tests/common/mod.rs"]
mod common;

use common::{Entry, FILE, Writer};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mut entry = Entry::new(b"member", Some(FILE), &[0; 64 * 1024]);
    entry.method = 8;
    entry.packed = Some(data.to_vec());
    common::exercise::exercise(Writer::default().entry(entry).finish());
});

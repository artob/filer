// This is free and unencumbered software released into the public domain.

//! Raw LZ4 blocks, which must decode as `lz4_flex` decodes them, into room
//! for as many bytes as the first two say: to the same bytes, or not at
//! all.

#![no_main]

#[path = "../../tests/common/mod.rs"]
mod common;

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Some((&[a, b], block)) = data.split_first_chunk() else {
        return;
    };
    let max = usize::from(u16::from_le_bytes([a, b]));
    let mut output = vec![0; max];
    let ours = decode_lz4::decode_block(block, &mut output)
        .ok()
        .map(|len| output[..len].to_vec());
    assert!(ours == common::lz4_flex_block(block, max), "disagrees with lz4_flex");
});

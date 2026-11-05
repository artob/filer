// This is free and unencumbered software released into the public domain.

//! Raw LZO1X blocks, which must decode as `lzokay-native` decodes them,
//! into room for as many bytes as the first two say: to the same bytes,
//! or not at all.

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
    let ours = decode_lzo::decode_block(block, &mut output)
        .ok()
        .map(|len| output[..len].to_vec());
    assert!(ours == common::lzokay(block, max), "disagrees with lzokay-native");
});

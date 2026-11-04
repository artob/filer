// This is free and unencumbered software released into the public domain.

//! Raw LZMA and LZMA2 streams: the first byte sets the size of the pieces
//! the decoder is fed in, which must decode as the whole does, and, by its
//! top bit, which of the two; then come LZMA's five bytes of properties,
//! or LZMA2's one, and the stream.

#![no_main]

#[path = "../../tests/common/mod.rs"]
mod common;

use filer_api::ErrorKind;
use filer_pack::{Lzma, Lzma2};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Some((&first, data)) = data.split_first() else {
        return;
    };
    let piece = usize::from(first % 64) + 1;
    let result = if first & 0x80 == 0 {
        let Some((&properties, stream)) = data.split_first_chunk::<5>() else {
            return;
        };
        if Lzma::new(properties, None).is_err() {
            return;
        }
        common::agree(|| Lzma::new(properties, None).unwrap(), stream, piece)
    } else {
        let Some((&property, stream)) = data.split_first() else {
            return;
        };
        if Lzma2::new(property).is_err() {
            return;
        }
        common::agree(|| Lzma2::new(property).unwrap(), stream, piece)
    };
    if let Err(error) = result {
        assert_eq!(error.kind(), ErrorKind::Corrupt);
    }
});

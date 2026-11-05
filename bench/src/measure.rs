// This is free and unencumbered software released into the public domain.

//! A single decoding, timed and checked, for `check`; `procedure` times
//! decoders as `METHOD.md` asks.

use crate::decoders::Decoder;
use std::time::Instant;

/// Decodes `packed` once with `decoder`, given and taken in pieces of
/// `piece` bytes, or whole for `usize::MAX`, and returns the MB/s (10^6
/// bytes a second) of decoded output; or why it failed, or that it
/// decoded otherwise than `original`.
pub fn once(
    decoder: &Decoder,
    packed: &[u8],
    original: &[u8],
    piece: usize,
) -> Result<f64, String> {
    let mut output = vec![0; original.len()];
    let start = Instant::now();
    let len = (decoder.decode)(packed, &mut output, piece)?;
    let elapsed = start.elapsed();
    match output[..len] == *original {
        true => Ok(original.len() as f64 / elapsed.as_secs_f64() / 1e6),
        false => Err("wrong".into()),
    }
}

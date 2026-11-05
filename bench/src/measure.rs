// This is free and unencumbered software released into the public domain.

//! Timing a decoder over an input: so far, a warm-up and the median of
//! timed iterations in one process. The procedure's independent
//! processes, interleaving, and confidence intervals are to follow.

use crate::{decoders::Decoder, inputs::Packed};
use std::time::{Duration, Instant};

/// How long each pair of decoder and input warms up, and the least it is
/// timed for, and the fewest iterations.
const WARM_UP: Duration = Duration::from_secs(1);
const TIMED: Duration = Duration::from_millis(500);
const ITERATIONS: usize = 10;

/// Decodes `packed` with `decoder`, given and taken in pieces of `piece`
/// bytes, or whole for `usize::MAX`, and returns the median MB/s (10^6
/// bytes a second) of decoded output; or why it failed, or that it
/// decoded otherwise than `original`, which every iteration is checked
/// against, untimed.
pub fn throughput(
    decoder: &Decoder,
    packed: &Packed,
    original: &[u8],
    piece: usize,
) -> Result<f64, String> {
    let mut output = vec![0; original.len()];
    let run = |output: &mut [u8]| -> Result<Duration, String> {
        output.fill(0);
        let start = Instant::now();
        let len = (decoder.decode)(&packed.bytes, output, piece)?;
        let elapsed = start.elapsed();
        if output[..len] != *original {
            return Err(format!("wrong at level {}", packed.level));
        }
        Ok(elapsed)
    };
    let start = Instant::now();
    while start.elapsed() < WARM_UP {
        run(&mut output)?;
    }
    let (mut times, start) = (Vec::new(), Instant::now());
    while times.len() < ITERATIONS || start.elapsed() < TIMED {
        times.push(run(&mut output)?);
    }
    times.sort();
    let median = times[times.len() / 2];
    Ok(original.len() as f64 / median.as_secs_f64() / 1e6)
}

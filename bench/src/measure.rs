// This is free and unencumbered software released into the public domain.

//! Timing a decoder over an input: so far, a warm-up and the median of
//! timed iterations in one process. The procedure's independent
//! processes, interleaving, and confidence intervals are to follow.

use crate::decoders::Decoder;
use std::time::{Duration, Instant};

/// How long each pair of decoder and input warms up, the least it is
/// timed for, and the fewest iterations.
pub struct Budget {
    pub warm_up: Duration,
    pub timed: Duration,
    pub iterations: usize,
}

impl Budget {
    /// What `METHOD.md` asks of each process.
    pub const FULL: Self = Self {
        warm_up: Duration::from_secs(1),
        timed: Duration::from_millis(500),
        iterations: 10,
    };

    /// One iteration, untimed but for itself: to check every output.
    pub const QUICK: Self = Self {
        warm_up: Duration::ZERO,
        timed: Duration::ZERO,
        iterations: 1,
    };
}

/// Decodes `packed` with `decoder`, given and taken in pieces of `piece`
/// bytes, or whole for `usize::MAX`, and returns the median MB/s (10^6
/// bytes a second) of decoded output; or why it failed, or that it
/// decoded otherwise than `original`, which every iteration is checked
/// against, untimed.
pub fn throughput(
    decoder: &Decoder,
    packed: &[u8],
    original: &[u8],
    piece: usize,
    budget: &Budget,
) -> Result<f64, String> {
    let mut output = vec![0; original.len()];
    let run = |output: &mut [u8]| -> Result<Duration, String> {
        output.fill(0);
        let start = Instant::now();
        let len = (decoder.decode)(packed, output, piece)?;
        let elapsed = start.elapsed();
        match output[..len] == *original {
            true => Ok(elapsed),
            false => Err("wrong".into()),
        }
    };
    let start = Instant::now();
    while start.elapsed() < budget.warm_up {
        run(&mut output)?;
    }
    let (mut times, start) = (Vec::new(), Instant::now());
    while times.len() < budget.iterations || start.elapsed() < budget.timed {
        times.push(run(&mut output)?);
    }
    times.sort();
    let median = times[times.len() / 2];
    Ok(original.len() as f64 / median.as_secs_f64() / 1e6)
}

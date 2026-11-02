// This is free and unencumbered software released into the public domain.

//! A dependency-free, structure-aware fuzzer for drivers that read volume
//! images: it mutates real volumes where their structures are, with
//! values chosen to hit boundaries, and walks each result, as
//! [`exercise`](crate::exercise) does, to check that every answer is one
//! a corrupt volume may give.
//!
//! A driver's test supplies what is its format's: its fixtures, the
//! regions of each worth mutating, and the [`Mutations`] to make there;
//! [`Fuzz::run`] does the rest. Each run takes a couple of hundred
//! inputs; for longer runs, set `<PREFIX>_ITERATIONS`, and
//! `<PREFIX>_SEED` picks another sequence. A failure prints the seed and
//! iteration that reproduce it.
//!
//! # Examples
//!
//! ```
//! use filer_spec::{
//!     Image, Outcome,
//!     fuzz::{Fuzz, Mutations},
//! };
//! use std::sync::Arc;
//!
//! const MUTATIONS: Mutations = Mutations::new(&[0, 0xFF], &[0, u32::MAX]);
//!
//! // A driver's test would open each input and walk it with `exercise`.
//! let corpus = [Arc::new(vec![1u8; 4096])];
//! let regions = [0..512, 0..4096];
//! Fuzz::new("FILER_EXAMPLE_FUZZ", 0x5EED).iterations(10).run(
//!     corpus.len(),
//!     |source, rng| MUTATIONS.mutate(&corpus[source], &regions, rng),
//!     |input| Outcome {
//!         opened: true,
//!         entries: 1,
//!         bytes: 1 + input.length(),
//!         errors: 0,
//!     },
//! );
//! ```

use crate::{Image, Outcome, Overlay};
use alloc::{sync::Arc, vec::Vec};
use core::ops::Range;
use std::{eprintln, panic, println};

/// SplitMix64: small, fast, and good enough to drive mutations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rng(u64);

impl Rng {
    /// Returns a generator starting from `seed`.
    #[must_use]
    pub const fn new(seed: u64) -> Self {
        Self(seed)
    }

    /// Returns the next number.
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Returns a number below `n`, or 0 if `n` is 0.
    pub fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n.max(1)
    }

    /// Returns one of `items`, which must not be empty.
    pub fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        items[self.below(items.len() as u64) as usize]
    }
}

/// A region of a volume worth mutating: where a structure is.
pub trait Region {
    /// Returns the bytes the region spans, which must not be empty.
    fn range(&self) -> Range<u64>;
}

impl Region for Range<u64> {
    fn range(&self) -> Range<u64> {
        self.clone()
    }
}

/// The byte order of a format's fields, for the words a mutation writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    /// Little-endian.
    Little,
    /// Big-endian.
    Big,
}

/// The mutations to make: each input takes 1 to 6, each in a region, and
/// each of these, by chance:
///
/// - flipping a bit;
/// - writing one of the boundary `bytes`;
/// - writing one of the boundary `words`, as 16 or 32 bits in the format's
///   byte order, aligned;
/// - copying an aligned chunk from elsewhere in the region over another;
/// - writing a chunk of random bytes;
/// - or, now and then, cutting the image short.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mutations {
    bytes: &'static [u8],
    words: &'static [u32],
    order: Order,
    chunk: u64,
}

impl Mutations {
    /// Returns the mutations writing `bytes` and `words`, which must not be
    /// empty, little-endian, in chunks of 8 bytes.
    #[must_use]
    pub const fn new(bytes: &'static [u8], words: &'static [u32]) -> Self {
        Self {
            bytes,
            words,
            order: Order::Little,
            chunk: 8,
        }
    }

    /// Writes words in `order`.
    #[must_use]
    pub const fn order(mut self, order: Order) -> Self {
        self.order = order;
        self
    }

    /// Copies and writes chunks of `chunk` bytes, a power of two: the size
    /// of a format's records, such as FAT's 32-byte directory slots.
    #[must_use]
    pub const fn chunk(mut self, chunk: u64) -> Self {
        self.chunk = chunk;
        self
    }

    /// Returns `image` mutated in some of `regions`, which must not be
    /// empty.
    pub fn mutate<B: Image, R: Region>(
        &self,
        image: &Arc<B>,
        regions: &[R],
        rng: &mut Rng,
    ) -> Overlay<B> {
        self.mutate_then(image, regions, rng, |_, _, _, _| {})
    }

    /// Mutates as [`mutate`](Self::mutate) does, calling `after` with each
    /// mutation's region and offset, as for a format whose structures
    /// carry checksums to reseal.
    pub fn mutate_then<B: Image, R: Region>(
        &self,
        image: &Arc<B>,
        regions: &[R],
        rng: &mut Rng,
        mut after: impl FnMut(&mut Overlay<B>, &R, u64, &mut Rng),
    ) -> Overlay<B> {
        let mut input = Overlay::new(image.clone());
        let word = |value: u32, bytes: usize| -> Vec<u8> {
            match (self.order, bytes) {
                (Order::Little, 2) => (value as u16).to_le_bytes().to_vec(),
                (Order::Big, 2) => (value as u16).to_be_bytes().to_vec(),
                (Order::Little, _) => value.to_le_bytes().to_vec(),
                (Order::Big, _) => value.to_be_bytes().to_vec(),
            }
        };
        let chunk = self.chunk;
        for _ in 0..1 + rng.below(6) {
            let region = &regions[rng.below(regions.len() as u64) as usize];
            let range = region.range();
            let at = range.start + rng.below(range.end - range.start);
            match rng.below(16) {
                0..=3 => {
                    let byte = input.byte(at) ^ (1 << rng.below(8));
                    input.patch(at, &[byte]);
                },
                4..=6 => input.patch(at, &[rng.pick(self.bytes)]),
                7..=8 => input.patch(at & !1, &word(rng.pick(self.words), 2)),
                9..=10 => input.patch(at & !3, &word(rng.pick(self.words), 4)),
                11..=12 => {
                    // Copy a chunk from elsewhere in the region.
                    let from = range.start + (rng.below(range.end - range.start) & !(chunk - 1));
                    let bytes: Vec<u8> = (from..from + chunk).map(|at| input.byte(at)).collect();
                    input.patch(at & !(chunk - 1), &bytes);
                },
                13..=14 => {
                    let bytes: Vec<u8> = (0..chunk).map(|_| rng.next_u64() as u8).collect();
                    input.patch(at & !(chunk - 1), &bytes);
                },
                _ => input.truncate(rng.below(input.length())),
            }
            after(&mut input, region, at, rng);
        }
        input
    }
}

/// A fuzzing run: its environment variables' prefix, and its defaults.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fuzz {
    prefix: &'static str,
    seed: u64,
    iterations: u64,
}

impl Fuzz {
    /// Returns a run of 200 inputs from `seed`, unless `<prefix>_SEED`
    /// and `<prefix>_ITERATIONS` say otherwise.
    #[must_use]
    pub const fn new(prefix: &'static str, seed: u64) -> Self {
        Self {
            prefix,
            seed,
            iterations: 200,
        }
    }

    /// Runs `iterations` inputs by default.
    #[must_use]
    pub const fn iterations(mut self, iterations: u64) -> Self {
        self.iterations = iterations;
        self
    }

    /// Makes each input with `input`, from one of `corpus` volumes (by its
    /// index) and a generator of its own, so that one reproduces alone, and
    /// walks it with `exercise`.
    ///
    /// # Panics
    ///
    /// If `exercise` panics on an input, after printing what reproduces
    /// it; or if no input opened, listed an entry, or read a byte, as the
    /// inputs then test nothing.
    pub fn run<T>(
        &self,
        corpus: usize,
        input: impl Fn(usize, &mut Rng) -> T,
        exercise: impl Fn(T) -> Outcome,
    ) {
        let env = |name: &str, default: u64| {
            let name = alloc::format!("{}_{name}", self.prefix);
            std::env::var(&name).map_or(default, |value| value.parse().expect(&name))
        };
        let iterations = env("ITERATIONS", self.iterations);
        let seed = env("SEED", self.seed);
        let (mut opened, mut total) = (0, Outcome::default());
        for iteration in 0..iterations {
            let mut rng = Rng::new(seed ^ iteration.wrapping_mul(0x2545_F491_4F6C_DD1D));
            let source = rng.below(corpus as u64) as usize;
            let made = input(source, &mut rng);
            // Unwinding only to say what failed, then on.
            match panic::catch_unwind(panic::AssertUnwindSafe(|| exercise(made))) {
                Ok(outcome) => {
                    opened += usize::from(outcome.opened);
                    total.entries += outcome.entries;
                    total.bytes += outcome.bytes;
                    total.errors += outcome.errors;
                },
                Err(panic) => {
                    eprintln!(
                        "failed: {}_SEED={seed} at iteration {iteration} (corpus volume {source})",
                        self.prefix
                    );
                    panic::resume_unwind(panic);
                },
            }
        }
        // Shown with `--nocapture`: how deep the inputs got.
        println!(
            "{iterations} inputs: {opened} opened, {} entries listed, {} bytes read, {} errors",
            total.entries, total.bytes, total.errors
        );
        assert!(
            opened > 0 && total.entries > 0 && total.bytes > 0,
            "inputs too shallow"
        );
    }
}

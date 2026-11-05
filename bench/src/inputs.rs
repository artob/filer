// This is free and unencumbered software released into the public domain.

//! The inputs: the protocol's synthetic edge cases, generated from a fixed
//! seed, and each encoded by its codec's reference encoder at the levels
//! its `BENCH.md` names.

use crate::decoders::Codec;
use std::io::Write;

/// An input as decoded, and its name.
pub struct Input {
    pub name: &'static str,
    pub data: Vec<u8>,
}

/// A level of a codec's reference encoder, and for zstd, whether it
/// matches across a 128 MiB window (`--long=27`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Level {
    pub number: u32,
    pub long: bool,
}

impl Level {
    const fn new(number: u32) -> Self {
        Self {
            number,
            long: false,
        }
    }

    /// Reads what [`label`](Self::label) writes.
    pub fn parse(label: &str) -> Option<Self> {
        let (number, long) = match label.strip_suffix("-long") {
            Some(number) => (number, true),
            None => (label, false),
        };
        Some(Self {
            number: number.parse().ok()?,
            long,
        })
    }

    /// Its name in paths and in `packed.toml`.
    pub fn label(self) -> String {
        match self.long {
            true => format!("{}-long", self.number),
            false => self.number.to_string(),
        }
    }
}

impl std::fmt::Display for Level {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.long {
            true => write!(f, "{} --long=27", self.number),
            false => write!(f, "{}", self.number),
        }
    }
}

/// The least size of the large inputs, which zstd also encodes with
/// `--long=27`.
const LARGE: usize = 32 << 20;

/// The size of each synthetic input.
const SIZE: usize = 8 << 20;

/// The synthetic edge cases, `SIZE` bytes each: bytes that do not
/// compress, zeros, a short pattern repeated, and a block repeated
/// farther apart than small windows reach.
pub fn synthetic() -> Vec<Input> {
    let mut random = SplitMix(0xF11E_5EED);
    let noise: Vec<u8> = (0..SIZE).map(|_| random.next() as u8).collect();
    let pattern = b"filer: a pattern, repeated. ".repeat(SIZE / 28 + 1);
    let far = noise[..SIZE / 2].repeat(2);
    vec![
        Input {
            name: "random",
            data: noise,
        },
        Input {
            name: "zeros",
            data: vec![0; SIZE],
        },
        Input {
            name: "pattern",
            data: pattern[..SIZE].to_vec(),
        },
        Input {
            name: "far",
            data: far,
        },
    ]
}

/// SplitMix64, a fixed sequence from a seed.
pub struct SplitMix(pub u64);

impl SplitMix {
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Shuffles `items`, by Fisher and Yates.
    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            items.swap(i, (self.next() % (i as u64 + 1)) as usize);
        }
    }
}

impl Codec {
    /// The levels its `BENCH.md` names, for an input of `size` bytes.
    pub fn levels(self, size: usize) -> Vec<Level> {
        match self {
            Self::Bzip2 => vec![Level::new(1), Level::new(9)],
            Self::Zstd => {
                let mut levels = [1, 3, 9, 19, 22].map(Level::new).to_vec();
                if size >= LARGE {
                    levels.push(Level {
                        number: 19,
                        long: true,
                    });
                }
                levels
            },
        }
    }

    /// The extension of its files.
    pub fn extension(self) -> &'static str {
        match self {
            Self::Bzip2 => "bz2",
            Self::Zstd => "zst",
        }
    }

    /// Encodes `data` at `level` with the reference encoder: bzip2 1.0.8
    /// or zstd 1.5.7, as their `-sys` crates build them, on one thread.
    pub fn encode(self, data: &[u8], level: Level) -> std::io::Result<Vec<u8>> {
        match self {
            Self::Bzip2 => {
                let level = bzip2_c::Compression::new(level.number);
                let mut encoder = bzip2_c::write::BzEncoder::new(Vec::new(), level);
                encoder.write_all(data)?;
                encoder.finish()
            },
            Self::Zstd => {
                use zstd::zstd_safe::CParameter;
                let mut encoder = zstd::bulk::Compressor::new(level.number as i32)?;
                if level.long {
                    encoder.set_parameter(CParameter::WindowLog(27))?;
                    encoder.set_parameter(CParameter::EnableLongDistanceMatching(true))?;
                }
                encoder.compress(data)
            },
        }
    }
}

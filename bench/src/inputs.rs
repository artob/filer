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

/// An input encoded at a level.
pub struct Packed {
    pub level: u32,
    pub bytes: Vec<u8>,
}

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
struct SplitMix(u64);

impl SplitMix {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

impl Codec {
    /// The levels its `BENCH.md` names.
    pub fn levels(self) -> &'static [u32] {
        match self {
            Self::Bzip2 => &[1, 9],
            Self::Zstd => &[1, 3, 9, 19, 22],
        }
    }

    /// Encodes `data` at `level` with the reference encoder: bzip2 1.0.8
    /// or zstd 1.5.7, as their `-sys` crates build them.
    pub fn encode(self, data: &[u8], level: u32) -> std::io::Result<Vec<u8>> {
        match self {
            Self::Bzip2 => {
                let level = bzip2_c::Compression::new(level);
                let mut encoder = bzip2_c::write::BzEncoder::new(Vec::new(), level);
                encoder.write_all(data)?;
                encoder.finish()
            },
            Self::Zstd => zstd::bulk::compress(data, level as i32),
        }
    }
}

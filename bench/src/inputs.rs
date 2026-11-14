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
            // zlib's, and libdeflate's slowest, which zlib has not.
            Self::Deflate => [1, 6, 9, 12].map(Level::new).to_vec(),
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
            Self::Deflate => "deflate",
            Self::Zstd => "zst",
        }
    }

    /// Encodes `data` at `level` with the reference encoder: bzip2 1.0.8,
    /// zstd 1.5.7, or raw Deflate by zlib at its levels and by libdeflate
    /// at 12, as their crates build them, on one thread.
    pub fn encode(self, data: &[u8], level: Level) -> std::io::Result<Vec<u8>> {
        match self {
            Self::Bzip2 => {
                let level = bzip2_c::Compression::new(level.number);
                let mut encoder = bzip2_c::write::BzEncoder::new(Vec::new(), level);
                encoder.write_all(data)?;
                encoder.finish()
            },
            Self::Deflate if level.number <= 9 => zlib_deflate(data, level.number),
            Self::Deflate => {
                use libdeflater::{CompressionLvl, Compressor};
                let failed = |error| std::io::Error::other(format!("{error:?}"));
                let level = CompressionLvl::new(level.number as i32).map_err(failed)?;
                let mut encoder = Compressor::new(level);
                let mut packed = vec![0; encoder.deflate_compress_bound(data.len())];
                let len = encoder
                    .deflate_compress(data, &mut packed)
                    .map_err(|error| std::io::Error::other(format!("{error:?}")))?;
                packed.truncate(len);
                Ok(packed)
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

/// Encodes `data` as raw Deflate by zlib at `level`, with its defaults
/// otherwise: a 32 KiB window, and 8 for its memory level.
fn zlib_deflate(data: &[u8], level: u32) -> std::io::Result<Vec<u8>> {
    use libz_sys as z;
    let failed = |status| std::io::Error::other(format!("zlib failed with {status}"));
    let mut stream = crate::decoders::zlib::stream();
    let size = size_of::<z::z_stream>() as std::ffi::c_int;
    let (level, method, strategy) = (level as i32, z::Z_DEFLATED, z::Z_DEFAULT_STRATEGY);
    let version = unsafe { z::zlibVersion() };
    let status =
        unsafe { z::deflateInit2_(&mut stream, level, method, -15, 8, strategy, version, size) };
    if status != z::Z_OK {
        return Err(failed(status));
    }
    let bound = unsafe { z::deflateBound(&mut stream, data.len() as _) };
    let mut packed = vec![0_u8; bound as usize];
    stream.next_in = data.as_ptr().cast_mut();
    stream.avail_in = data.len() as _;
    stream.next_out = packed.as_mut_ptr();
    stream.avail_out = packed.len() as _;
    let status = unsafe { z::deflate(&mut stream, z::Z_FINISH) };
    packed.truncate(stream.total_out as usize);
    unsafe { z::deflateEnd(&mut stream) };
    match status {
        z::Z_STREAM_END => Ok(packed),
        status => Err(failed(status)),
    }
}

// This is free and unencumbered software released into the public domain.

//! The compressors: each decompresses a whole block, of metadata or data,
//! through its `decode-*` crate, if its feature is enabled.

/// What compresses an image's blocks, by the superblock's id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Compressor {
    /// zlib streams, which `mksquashfs` calls gzip.
    Gzip,
    /// LZMA, each block after LZMA's classic 13-byte header.
    Lzma,
    /// Raw LZO1X blocks.
    Lzo,
    /// xz streams.
    Xz,
    /// Raw LZ4 blocks.
    Lz4,
    /// zstd frames.
    Zstd,
}

impl Compressor {
    /// Returns the compressor of `id`, if this driver reads it.
    pub(crate) const fn from_id(id: u16) -> Option<Self> {
        let compressor = match id {
            1 => Self::Gzip,
            2 => Self::Lzma,
            3 => Self::Lzo,
            4 => Self::Xz,
            5 => Self::Lz4,
            6 => Self::Zstd,
            _ => return None,
        };
        if compressor.enabled() {
            Some(compressor)
        } else {
            None
        }
    }

    /// Returns whether its feature is enabled.
    const fn enabled(self) -> bool {
        match self {
            Self::Gzip => cfg!(feature = "gzip"),
            Self::Lzma => cfg!(feature = "lzma"),
            Self::Lzo => cfg!(feature = "lzo"),
            Self::Xz => cfg!(feature = "xz"),
            Self::Lz4 => cfg!(feature = "lz4"),
            Self::Zstd => cfg!(feature = "zstd"),
        }
    }

    /// Decompresses `input`, a whole block, into `output`, and returns how
    /// many bytes it decompressed to, or `None` if it is malformed or
    /// decompresses to more than `output` holds.
    pub(crate) fn decompress(self, input: &[u8], output: &mut [u8]) -> Option<usize> {
        match self {
            #[cfg(feature = "gzip")]
            Self::Gzip => {
                let mut decoder = decode_zlib::Decoder::new();
                let len = drain(input, output, |i, o| decoder.decode(i, o).ok())?;
                decoder.finish().ok().map(|()| len)
            },
            #[cfg(feature = "lzma")]
            Self::Lzma => {
                let (properties, rest) = input.split_first_chunk::<5>()?;
                let (size, rest) = rest.split_first_chunk::<8>()?;
                let size = Some(u64::from_le_bytes(*size)).filter(|&size| size != u64::MAX);
                let mut decoder = decode_lzma::Decoder::lzma(*properties, size).ok()?;
                let len = drain(rest, output, |i, o| decoder.decode(i, o).ok())?;
                decoder.finish().ok().map(|()| len)
            },
            #[cfg(feature = "lzo")]
            Self::Lzo => decode_lzo::decode_block(input, output).ok(),
            #[cfg(feature = "xz")]
            Self::Xz => {
                let mut decoder = decode_xz::Decoder::new();
                let len = drain(input, output, |i, o| decoder.decode(i, o).ok())?;
                decoder.finish().ok().map(|()| len)
            },
            #[cfg(feature = "lz4")]
            Self::Lz4 => decode_lz4::decode_block(input, output).ok(),
            #[cfg(feature = "zstd")]
            Self::Zstd => {
                let mut decoder = decode_zstd::Decoder::new();
                let len = drain(input, output, |i, o| decoder.decode(i, o).ok())?;
                decoder.finish().ok().map(|()| len)
            },
            // A compressor whose feature is disabled is never made.
            #[allow(unreachable_patterns, reason = "with every feature, none is left")]
            _ => {
                let _ = (input, output);
                None
            },
        }
    }
}

/// Feeds all of `input` to a sans-I/O decoder's `decode`, `step`, into
/// `output`, and returns how much it produced, or `None` if it fails, or
/// stops before taking all of `input`, as when `output` is full.
#[allow(dead_code, reason = "used by the compressors that stream")]
fn drain(
    input: &[u8],
    output: &mut [u8],
    mut step: impl FnMut(&[u8], &mut [u8]) -> Option<(usize, usize)>,
) -> Option<usize> {
    let (mut consumed, mut produced) = (0_usize, 0_usize);
    loop {
        let (taken, made) = step(input.get(consumed..)?, output.get_mut(produced..)?)?;
        consumed = consumed.checked_add(taken)?;
        produced = produced.checked_add(made)?;
        if (taken, made) == (0, 0) {
            return (consumed == input.len()).then_some(produced);
        }
    }
}

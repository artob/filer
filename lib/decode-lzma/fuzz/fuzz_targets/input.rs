// This is free and unencumbered software released into the public domain.

//! What both targets read from the fuzzer's input: its first byte picks
//! LZMA, of an unknown size or of the size in the next eight bytes, or
//! LZMA2, and the size of pieces; then come LZMA's properties, or LZMA2's
//! one, and the stream. Seed the corpus with streams so framed.

use decode_lzma::{Decoder, Error};

/// What the input asks to decode.
pub enum Kind {
    Lzma([u8; 5], Option<u64>),
    Lzma2(u8),
}

impl Kind {
    pub fn decoder(&self) -> Result<Decoder, Error> {
        match *self {
            Self::Lzma(properties, size) => Decoder::lzma(properties, size),
            Self::Lzma2(property) => Decoder::lzma2(property),
        }
    }
}

/// Returns what `data` asks to decode, the size of pieces, and the stream.
pub fn parse(data: &[u8]) -> Option<(Kind, usize, &[u8])> {
    let (&first, rest) = data.split_first()?;
    let piece = usize::from(first >> 2) + 1;
    match first & 3 {
        0 => {
            let (properties, stream) = rest.split_first_chunk::<5>()?;
            Some((Kind::Lzma(*properties, None), piece, stream))
        },
        1 => {
            let (properties, rest) = rest.split_first_chunk::<5>()?;
            let (size, stream) = rest.split_first_chunk::<8>()?;
            // Small enough to decode within the checks' cap.
            let size = u64::from_le_bytes(*size) % (1 << 20);
            Some((Kind::Lzma(*properties, Some(size)), piece, stream))
        },
        _ => {
            let (&property, stream) = rest.split_first()?;
            Some((Kind::Lzma2(property % 30), piece, stream))
        },
    }
}

// This is free and unencumbered software released into the public domain.

//! Decoding members: the compression methods this build supports, each
//! checked against the member's CRC-32.

use alloc::boxed::Box;
use filer_api::{Error, ErrorKind, OpKind, checksum::Crc32};
use filer_pack::{Codec, Decoder, Verified};

/// Compression methods (APPNOTE 4.4.5).
pub(crate) const STORED: u16 = 0;
#[cfg_attr(not(feature = "deflate"), allow(dead_code))]
pub(crate) const DEFLATE: u16 = 8;
#[cfg_attr(not(feature = "bzip2"), allow(dead_code))]
pub(crate) const BZIP2: u16 = 12;
#[cfg_attr(not(feature = "lzma"), allow(dead_code))]
pub(crate) const LZMA: u16 = 14;
#[cfg_attr(not(feature = "zstd"), allow(dead_code))]
pub(crate) const ZSTD: u16 = 93;
#[cfg_attr(not(feature = "xz"), allow(dead_code))]
pub(crate) const XZ: u16 = 95;

/// How a member is encoded: its method, CRC-32, size, and whether it is
/// encrypted, which this crate does not support.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Method {
    pub(crate) method: u16,
    pub(crate) crc: u32,
    pub(crate) size: u64,
    pub(crate) encrypted: bool,
}

impl Codec for Method {
    fn is_stored(&self) -> bool {
        self.method == STORED && !self.encrypted
    }

    fn decoder(&self) -> Result<Box<dyn Decoder>, Error> {
        let unsupported = Error::new(ErrorKind::Unsupported, OpKind::Read);
        if self.encrypted {
            return Err(unsupported);
        }
        let decoder = decoder(self.method, self.size).ok_or(unsupported)?;
        Ok(Box::new(Verified::new(decoder, Crc32::new(), self.crc)))
    }
}

/// Returns a decoder for `method`, of a member of `size` bytes, if this
/// build supports it.
#[cfg_attr(not(feature = "lzma"), allow(unused_variables))]
fn decoder(method: u16, size: u64) -> Option<Box<dyn Decoder>> {
    match method {
        #[cfg(feature = "deflate")]
        DEFLATE => Some(Box::new(filer_pack::Deflate::new())),
        #[cfg(feature = "bzip2")]
        BZIP2 => Some(Box::new(filer_pack::Bzip2::new())),
        #[cfg(feature = "lzma")]
        LZMA => Some(Box::new(Lzma::Header {
            header: [0; 9],
            read: 0,
            size,
        })),
        #[cfg(feature = "zstd")]
        ZSTD => Some(Box::new(filer_pack::Zstd::new())),
        #[cfg(feature = "xz")]
        XZ => Some(Box::new(filer_pack::Xz::new())),
        _ => None,
    }
}

/// A decoder of LZMA as ZIP stores it (APPNOTE 5.8): the LZMA SDK's
/// version, two bytes, the length of the properties, two bytes,
/// little-endian, and the properties, five, before the raw stream, whose
/// size is the member's.
#[cfg(feature = "lzma")]
enum Lzma {
    /// Reading the header, `read` bytes of it so far.
    Header {
        header: [u8; 9],
        read: usize,
        size: u64,
    },
    Stream(filer_pack::Lzma),
}

#[cfg(feature = "lzma")]
impl Decoder for Lzma {
    fn decode(&mut self, input: &[u8], output: &mut [u8]) -> Result<(usize, usize), Error> {
        match self {
            Self::Stream(lzma) => lzma.decode(input, output),
            Self::Header { header, read, size } => {
                let rest = header.get_mut(*read..).unwrap_or_default();
                let n = rest.len().min(input.len());
                if let (Some(to), Some(from)) = (rest.get_mut(..n), input.get(..n)) {
                    to.copy_from_slice(from);
                }
                *read = read.saturating_add(n);
                if *read < header.len() {
                    return Ok((n, 0));
                }
                // The properties are always five bytes.
                let [_, _, 5, 0, properties @ ..] = *header else {
                    return Err(Error::new(ErrorKind::Corrupt, OpKind::Read));
                };
                *self = Self::Stream(filer_pack::Lzma::new(properties, Some(*size))?);
                let rest = input.get(n..).unwrap_or_default();
                let (consumed, produced) = self.decode(rest, output)?;
                Ok((consumed.saturating_add(n), produced))
            },
        }
    }

    fn finish(&mut self) -> Result<(), Error> {
        match self {
            Self::Stream(lzma) => lzma.finish(),
            Self::Header { .. } => Err(Error::new(ErrorKind::Corrupt, OpKind::Read)),
        }
    }
}

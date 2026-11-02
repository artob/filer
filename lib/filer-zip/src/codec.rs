// This is free and unencumbered software released into the public domain.

//! Decoding members: the compression methods this build supports, each
//! checked against the member's CRC-32.

use alloc::boxed::Box;
use filer_api::{
    Error, ErrorKind, OpKind,
    archive::{Codec, Decoder},
};

/// Compression methods.
pub(crate) const STORED: u16 = 0;
#[cfg_attr(not(feature = "deflate"), allow(dead_code))]
pub(crate) const DEFLATE: u16 = 8;

/// How a member is encoded: its method, CRC-32, and whether it is
/// encrypted, which this crate does not support.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Method {
    pub(crate) method: u16,
    pub(crate) crc: u32,
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
        let inner = inner(self.method).ok_or(unsupported)?;
        Ok(Box::new(Checked {
            inner,
            crc: Crc32::new(),
            expected: self.crc,
        }))
    }
}

/// Returns a decoder for `method`, if this build supports it.
fn inner(method: u16) -> Option<Box<dyn Inner>> {
    match method {
        #[cfg(feature = "deflate")]
        DEFLATE => Some(Box::new(deflate::Deflate::new())),
        _ => None,
    }
}

/// A method's decoder, before the CRC check.
trait Inner: Send {
    fn decode(&mut self, input: &[u8], output: &mut [u8]) -> Result<(usize, usize), Error>;
}

/// A decoder checking the CRC-32 of what it produced once the member ends.
struct Checked {
    inner: Box<dyn Inner>,
    crc: Crc32,
    expected: u32,
}

impl Decoder for Checked {
    fn decode(&mut self, input: &[u8], output: &mut [u8]) -> Result<(usize, usize), Error> {
        let (consumed, produced) = self.inner.decode(input, output)?;
        self.crc.update(output.get(..produced).unwrap_or_default());
        Ok((consumed, produced))
    }

    fn finish(&mut self) -> Result<(), Error> {
        if self.crc.value() == self.expected {
            Ok(())
        } else {
            Err(Error::new(ErrorKind::Corrupt, OpKind::Read))
        }
    }
}

#[cfg(feature = "deflate")]
mod deflate {
    use super::Inner;
    use alloc::boxed::Box;
    use filer_api::{Error, ErrorKind, OpKind};
    use miniz_oxide::{
        DataFormat, MZError, MZFlush,
        inflate::stream::{InflateState, inflate},
    };

    /// A raw Deflate stream (RFC 1951), as ZIP stores it.
    pub(super) struct Deflate(Box<InflateState>);

    impl Deflate {
        pub(super) fn new() -> Self {
            Self(InflateState::new_boxed(DataFormat::Raw))
        }
    }

    impl Inner for Deflate {
        fn decode(&mut self, input: &[u8], output: &mut [u8]) -> Result<(usize, usize), Error> {
            let result = inflate(&mut self.0, input, output, MZFlush::None);
            match result.status {
                // No progress: the archive sees the member end early.
                Ok(_) | Err(MZError::Buf) => Ok((result.bytes_consumed, result.bytes_written)),
                Err(_) => Err(Error::new(ErrorKind::Corrupt, OpKind::Read)),
            }
        }
    }
}

/// CRC-32 as ZIP uses it (ISO-HDLC: reflected, polynomial 0x04C11DB7).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Crc32(u32);

/// The CRC of each byte value, for a byte at a time.
// Evaluated at compile time, where overflow fails the build.
#[allow(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::indexing_slicing
)]
const TABLE: [u32; 256] = {
    let mut table = [0; 256];
    let mut n = 0;
    while n < 256 {
        let mut crc = n;
        let mut bit = 0;
        while bit < 8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
            bit += 1;
        }
        table[n as usize] = crc;
        n += 1;
    }
    table
};

impl Crc32 {
    pub(crate) const fn new() -> Self {
        Self(!0)
    }

    pub(crate) fn update(&mut self, data: &[u8]) {
        for &byte in data {
            let [low, ..] = self.0.to_le_bytes();
            let entry = TABLE.get(usize::from(low ^ byte)).copied().unwrap_or(0);
            self.0 = (self.0 >> 8) ^ entry;
        }
    }

    pub(crate) const fn value(&self) -> u32 {
        !self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_crc32() {
        let mut crc = Crc32::new();
        assert_eq!(crc.value(), 0);
        crc.update(b"123456789");
        assert_eq!(crc.value(), 0xCBF4_3926);
    }
}

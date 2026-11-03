// This is free and unencumbered software released into the public domain.

//! Checking members: `newc` archives with checksums (magic `070702`)
//! record the sum of each regular file's bytes, checked once a read
//! reaches the file's end. GNU cpio records 0 for other members, such as
//! symlinks, so theirs are not checked.

use alloc::boxed::Box;
use filer_api::{Error, ErrorKind, OpKind};
use filer_pack::{Codec, Decoder, Digest, Passthrough, Verified};

/// How a member is stored: as is, or with the sum of its bytes to check.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) struct Sum(pub(crate) Option<u32>);

impl Codec for Sum {
    fn is_stored(&self) -> bool {
        self.0.is_none()
    }

    fn decoder(&self) -> Result<Box<dyn Decoder>, Error> {
        let expected = self
            .0
            .ok_or_else(|| Error::new(ErrorKind::Unsupported, OpKind::Read))?;
        Ok(Box::new(Verified::new(Passthrough, ByteSum(0), expected)))
    }
}

/// The sum of a member's bytes, modulo 2^32, as `newc` records it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
struct ByteSum(u32);

impl Digest for ByteSum {
    type Output = u32;

    fn update(&mut self, bytes: &[u8]) {
        self.0 = bytes
            .iter()
            .fold(self.0, |sum, &b| sum.wrapping_add(u32::from(b)));
    }

    fn finish(&self) -> u32 {
        self.0
    }
}

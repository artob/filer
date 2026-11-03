// This is free and unencumbered software released into the public domain.

//! Checking members: `newc` archives with checksums (magic `070702`)
//! record the sum of each regular file's bytes, checked once a read
//! reaches the file's end. GNU cpio records 0 for other members, such as
//! symlinks, so theirs are not checked.

use alloc::boxed::Box;
use filer_api::{
    Error, ErrorKind, OpKind,
    archive::{Codec, Decoder},
};

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
        Ok(Box::new(Summing { expected, sum: 0 }))
    }
}

fn add(sum: u32, bytes: &[u8]) -> u32 {
    bytes
        .iter()
        .fold(sum, |sum, &b| sum.wrapping_add(u32::from(b)))
}

/// A decoder that passes bytes through, summing them.
struct Summing {
    expected: u32,
    sum: u32,
}

impl Decoder for Summing {
    fn decode(&mut self, input: &[u8], output: &mut [u8]) -> Result<(usize, usize), Error> {
        let n = input.len().min(output.len());
        let (Some(from), Some(to)) = (input.get(..n), output.get_mut(..n)) else {
            return Ok((0, 0));
        };
        to.copy_from_slice(from);
        self.sum = add(self.sum, from);
        Ok((n, n))
    }

    fn finish(&mut self) -> Result<(), Error> {
        if self.sum == self.expected {
            Ok(())
        } else {
            Err(Error::new(ErrorKind::Corrupt, OpKind::Read))
        }
    }
}

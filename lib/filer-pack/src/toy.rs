// This is free and unencumbered software released into the public domain.

//! A toy codec for tests: each packed byte decodes to two copies of
//! itself.

use crate::{Codec, Decoder};
use alloc::boxed::Box;
use core::sync::atomic::{AtomicUsize, Ordering};
use filer_api::{Error, ErrorKind, OpKind};

/// Decoders started, across every test.
pub(crate) static STARTED: AtomicUsize = AtomicUsize::new(0);

/// A toy codec: each packed byte decodes to two copies of itself, and
/// the decoded bytes must sum to `sum`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Doubled {
    pub(crate) sum: u64,
}

pub(crate) struct Doubler {
    pending: Option<u8>,
    sum: u64,
    expected: u64,
}

impl Decoder for Doubler {
    fn decode(&mut self, input: &[u8], output: &mut [u8]) -> Result<(usize, usize), Error> {
        let (mut consumed, mut produced) = (0, 0);
        while produced < output.len() {
            let byte = match self.pending.take() {
                Some(byte) => byte,
                None => {
                    let Some(&byte) = input.get(consumed) else {
                        break;
                    };
                    consumed += 1;
                    self.pending = Some(byte);
                    byte
                },
            };
            output[produced] = byte;
            produced += 1;
            self.sum += u64::from(byte);
        }
        Ok((consumed, produced))
    }

    fn finish(&mut self) -> Result<(), Error> {
        if self.sum == self.expected {
            Ok(())
        } else {
            Err(Error::new(ErrorKind::Corrupt, OpKind::Read))
        }
    }
}

impl Codec for Doubled {
    fn is_stored(&self) -> bool {
        false
    }

    fn decoder(&self) -> Result<Box<dyn Decoder>, Error> {
        STARTED.fetch_add(1, Ordering::Relaxed);
        Ok(Box::new(Doubler {
            pending: None,
            sum: 0,
            expected: self.sum,
        }))
    }
}

// This is free and unencumbered software released into the public domain.

use crate::{Error, ErrorKind, OpKind};
use alloc::boxed::Box;

/// How a format's members are encoded, and how to decode them.
///
/// Each [`Member`](crate::archive::Member) carries a value of its format's
/// codec type: a format without compression uses [`Stored`], while a
/// format with it records each member's method and whatever its decoder
/// needs, such as a checksum. [`Archive`](crate::archive::Archive) reads
/// stored members straight from the source, and decodes the others as a
/// stream through a [`Decoder`], which it keeps between reads.
///
/// # Examples
///
/// ```
/// use filer_api::{Error, archive::{Codec, Decoder}};
///
/// /// A codec whose members are stored as is, or with each byte inverted.
/// #[derive(Clone, Copy)]
/// struct Inverted(bool);
///
/// struct Invert;
///
/// impl Decoder for Invert {
///     fn decode(&mut self, input: &[u8], output: &mut [u8]) -> Result<(usize, usize), Error> {
///         let n = input.len().min(output.len());
///         for (out, byte) in output.iter_mut().zip(input) {
///             *out = !byte;
///         }
///         Ok((n, n))
///     }
/// }
///
/// impl Codec for Inverted {
///     fn is_stored(&self) -> bool {
///         !self.0
///     }
///
///     fn decoder(&self) -> Result<Box<dyn Decoder>, Error> {
///         Ok(Box::new(Invert))
///     }
/// }
/// ```
pub trait Codec: Copy + Send + Sync + 'static {
    /// Returns whether the member's bytes are stored as is, so that reads
    /// go straight to the source at any offset.
    fn is_stored(&self) -> bool;

    /// Returns a new decoder for a member that is not stored as is,
    /// positioned at the member's start.
    ///
    /// # Errors
    ///
    /// [`Unsupported`](ErrorKind::Unsupported) for an encoding this build
    /// cannot decode, such as a method behind a disabled feature or an
    /// encrypted member.
    fn decoder(&self) -> Result<Box<dyn Decoder>, Error>;
}

/// A member's decompressor, part way through the member.
///
/// [`Archive`](crate::archive::Archive) feeds it the member's packed bytes
/// in order and never asks for more output than the member's declared
/// size, so a decoder needs no limits of its own against decompression
/// bombs.
pub trait Decoder: Send {
    /// Decodes from `input`, the member's next packed bytes, into
    /// `output`, and returns how many bytes of each it consumed and
    /// produced. `input` is empty once the packed bytes are exhausted;
    /// returning no progress then means the member's data ended early.
    ///
    /// # Errors
    ///
    /// [`Corrupt`](ErrorKind::Corrupt) for malformed data.
    fn decode(&mut self, input: &[u8], output: &mut [u8]) -> Result<(usize, usize), Error>;

    /// Checks the whole member once its declared size has been decoded,
    /// for example against a checksum. Members that are never read to
    /// their end are never checked.
    ///
    /// # Errors
    ///
    /// [`Corrupt`](ErrorKind::Corrupt) if the check fails.
    fn finish(&mut self) -> Result<(), Error> {
        Ok(())
    }
}

/// The codec of formats without compression, such as TAR: every member is
/// stored as is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Stored;

impl Codec for Stored {
    fn is_stored(&self) -> bool {
        true
    }

    fn decoder(&self) -> Result<Box<dyn Decoder>, Error> {
        Err(Error::new(ErrorKind::Unsupported, OpKind::Read))
    }
}

// This is free and unencumbered software released into the public domain.

use crate::{IoBuf, IoBufMut, Result};
use alloc::boxed::Box;
use core::any::Any;

/// A read buffer of any [`IoBufMut`] type, boxed so that operations can
/// carry it as data.
///
/// The frontend erases the caller's buffer into an [`Op`](crate::Op),
/// the driver fills it through [`IoBufMut`], and the frontend recovers the
/// original type with [`downcast`](Self::downcast) when the operation
/// completes. Boxing costs one allocation per read.
///
/// # Examples
///
/// ```
/// use filer_api::{ErasedBufMut, IoBufMut};
///
/// let mut erased = ErasedBufMut::new(Vec::<u8>::with_capacity(4));
/// erased.fill(&mut |spare| { spare[0] = b'x'; Ok(1) }).unwrap();
/// let buf: Vec<u8> = erased.downcast().unwrap();
/// assert_eq!(buf, b"x");
/// ```
pub struct ErasedBufMut(Box<dyn IoBufMut>);

impl ErasedBufMut {
    /// Erases `buf`.
    pub fn new<B: IoBufMut>(buf: B) -> Self {
        Self(Box::new(buf))
    }

    /// Returns `true` if the erased buffer is a `B`.
    #[must_use]
    pub fn is<B: IoBufMut>(&self) -> bool {
        let any: &dyn Any = &*self.0;
        any.is::<B>()
    }

    /// Recovers the original buffer.
    ///
    /// # Errors
    ///
    /// Returns `self` unchanged if the erased buffer is not a `B`.
    pub fn downcast<B: IoBufMut>(self) -> core::result::Result<B, Self> {
        if !self.is::<B>() {
            return Err(self);
        }
        let any: Box<dyn Any> = self.0;
        Ok(*any.downcast::<B>().expect("type checked above"))
    }
}

impl IoBuf for ErasedBufMut {
    fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl IoBufMut for ErasedBufMut {
    fn capacity(&self) -> usize {
        self.0.capacity()
    }

    fn fill(&mut self, read: &mut dyn FnMut(&mut [u8]) -> Result<usize>) -> Result<usize> {
        self.0.fill(read)
    }
}

impl core::fmt::Debug for ErasedBufMut {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ErasedBufMut")
            .field("len", &self.as_bytes().len())
            .field("capacity", &self.capacity())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    #[test]
    fn round_trips_a_vec() {
        let mut buf = Vec::with_capacity(8);
        buf.extend_from_slice(b"ab");
        let ptr = buf.as_ptr();
        let mut erased = ErasedBufMut::new(buf);
        assert_eq!((erased.as_bytes(), erased.capacity()), (&b"ab"[..], 8));
        erased.fill(&mut |spare| Ok(spare.len())).unwrap();
        let buf: Vec<u8> = erased.downcast().unwrap();
        assert_eq!(buf.len(), 8);
        assert_eq!(buf.as_ptr(), ptr);
    }

    #[test]
    fn refuses_the_wrong_type() {
        let erased = ErasedBufMut::new(Vec::<u8>::new());
        let erased = erased.downcast::<ErasedBufMut>().unwrap_err();
        assert!(erased.is::<Vec<u8>>());
    }
}

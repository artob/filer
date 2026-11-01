// This is free and unencumbered software released into the public domain.

use crate::Result;
use alloc::{boxed::Box, vec::Vec};

mod sealed {
    /// Restricts buffer implementations to this crate.
    pub trait Sealed {}
}

/// A buffer whose contents can be written to a file.
///
/// I/O operations take buffers by value and return them when done, because
/// completion-based kernels (io_uring, IOCP) may access the memory after
/// submission, even if the caller's future is dropped. Buffers are
/// therefore owned, `Send`, and `'static`, and their heap storage must not
/// move when the buffer value moves.
///
/// The trait is sealed: only this crate implements it, for types it knows
/// satisfy that requirement. It extends [`Any`](core::any::Any) so that an
/// erased buffer ([`ErasedBufMut`](crate::ErasedBufMut)) can be turned
/// back into its original type.
pub trait IoBuf: sealed::Sealed + core::any::Any + Send {
    /// Returns the buffer's contents.
    fn as_bytes(&self) -> &[u8];
}

/// A buffer that a read can fill, appending to its contents.
///
/// A read appends at most [`spare`](Self::spare) bytes, the capacity beyond
/// the current contents; a buffer with no spare capacity reads nothing. Use
/// [`Vec::with_capacity`] to make room.
///
/// The trait is sealed, like [`IoBuf`].
///
/// # Examples
///
/// ```
/// use filer_api::IoBufMut;
///
/// let mut buf = Vec::with_capacity(8);
/// buf.extend_from_slice(b"ab");
/// let n = buf.fill(&mut |spare| {
///     spare[..3].copy_from_slice(b"cde");
///     Ok(3)
/// });
/// assert_eq!(n.unwrap(), 3);
/// assert_eq!(buf, b"abcde");
/// ```
pub trait IoBufMut: IoBuf {
    /// Returns the total capacity in bytes, contents included.
    fn capacity(&self) -> usize;

    /// Returns how many more bytes a read can append.
    fn spare(&self) -> usize {
        self.capacity() - self.as_bytes().len()
    }

    /// Lets `read` fill the spare capacity, then appends what it filled.
    ///
    /// `read` receives the spare capacity, zero-filled, and returns how many
    /// bytes from its start it filled; those become part of the contents.
    /// If `read` fails, the contents are left as they were. The buffer is
    /// never reallocated, so its storage stays put while `read` runs.
    ///
    /// Zero-filling keeps buffers free of `unsafe`; it costs a `memset`
    /// per read, which is small next to a system call.
    ///
    /// # Errors
    ///
    /// Returns the error from `read`.
    ///
    /// # Panics
    ///
    /// Panics if `read` claims to have filled more than it was given, which
    /// is a bug in the caller.
    fn fill(&mut self, read: &mut dyn FnMut(&mut [u8]) -> Result<usize>) -> Result<usize>;
}

impl sealed::Sealed for Vec<u8> {}

impl IoBuf for Vec<u8> {
    fn as_bytes(&self) -> &[u8] {
        self
    }
}

impl IoBufMut for Vec<u8> {
    fn capacity(&self) -> usize {
        Vec::capacity(self)
    }

    fn fill(&mut self, read: &mut dyn FnMut(&mut [u8]) -> Result<usize>) -> Result<usize> {
        let len = self.len();
        // Resizing to exactly the capacity never reallocates.
        self.resize(Vec::capacity(self), 0);
        let result = read(&mut self[len..]);
        let filled = match &result {
            Ok(n) => *n,
            Err(_) => 0,
        };
        let spare = self.len() - len;
        assert!(filled <= spare, "read filled {filled} of {spare} bytes");
        self.truncate(len + filled);
        result
    }
}

/// Write-only: a boxed slice has no spare capacity to read into.
impl sealed::Sealed for Box<[u8]> {}

impl sealed::Sealed for crate::ErasedBufMut {}

impl IoBuf for Box<[u8]> {
    fn as_bytes(&self) -> &[u8] {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Error, ErrorKind, OpKind};
    use alloc::vec;

    #[test]
    fn fills_spare_capacity() {
        let mut buf = Vec::with_capacity(4);
        assert_eq!(buf.spare(), 4);
        assert_eq!(buf.fill(&mut |spare| Ok(spare.len())).unwrap(), 4);
        assert_eq!(buf, [0, 0, 0, 0]);
        assert_eq!(buf.spare(), 0);
    }

    #[test]
    fn zeroes_stale_bytes() {
        let mut buf = vec![7, 7, 7];
        buf.truncate(1);
        buf.fill(&mut |spare| {
            assert!(spare.iter().all(|&b| b == 0));
            Ok(0)
        })
        .unwrap();
        assert_eq!(buf, [7]);
    }

    #[test]
    fn keeps_contents_on_error() {
        let mut buf = Vec::with_capacity(4);
        buf.push(1);
        let result = buf.fill(&mut |spare| {
            spare.fill(9);
            Err(Error::new(ErrorKind::Io, OpKind::Read))
        });
        assert_eq!(result.unwrap_err().kind(), ErrorKind::Io);
        assert_eq!(buf, [1]);
    }

    #[test]
    fn does_not_reallocate() {
        let mut buf = Vec::with_capacity(16);
        let before = buf.as_ptr();
        buf.fill(&mut |spare| Ok(spare.len())).unwrap();
        assert_eq!(buf.as_ptr(), before);
    }

    #[test]
    #[should_panic(expected = "read filled 5 of 4 bytes")]
    fn rejects_overfilling() {
        let mut buf = Vec::with_capacity(4);
        let _ = buf.fill(&mut |_| Ok(5));
    }

    #[test]
    fn exposes_boxed_slices_for_writing() {
        let buf: Box<[u8]> = Box::from(&b"abc"[..]);
        assert_eq!(buf.as_bytes(), b"abc");
    }
}

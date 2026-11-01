// This is free and unencumbered software released into the public domain.

use crate::Result;

/// The result of an I/O operation together with its buffer, which is
/// returned whatever happened.
///
/// Operations take buffers by value (see [`IoBuf`](crate::IoBuf)), so a
/// failed read still hands back the caller's buffer for reuse.
///
/// # Examples
///
/// ```
/// use filer_api::BufResult;
///
/// let BufResult(result, buf) = BufResult(Ok(3), vec![1, 2, 3]);
/// assert_eq!(result.unwrap(), buf.len());
/// ```
#[derive(Debug)]
pub struct BufResult<T, B>(pub Result<T>, pub B);

impl<T, B> BufResult<T, B> {
    /// Maps the success value, keeping the buffer.
    #[must_use]
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> BufResult<U, B> {
        BufResult(self.0.map(f), self.1)
    }
}

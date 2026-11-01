// This is free and unencumbered software released into the public domain.

use crate::{ErasedBufMut, Output, Result};

/// The outcome of an [`Op`](crate::Op): its result, plus the buffer it
/// carried, which is returned whether or not the operation succeeded.
///
/// # Examples
///
/// ```
/// use filer_api::{Completion, ErasedBufMut, Output};
///
/// let completion = Completion::new(Ok(Output::Read(0)))
///     .with_buf(ErasedBufMut::new(Vec::<u8>::new()));
/// assert!(completion.buf.is_some());
/// ```
#[derive(Debug)]
#[non_exhaustive]
pub struct Completion {
    /// The operation's result.
    pub result: Result<Output>,

    /// The read buffer the operation carried, if any.
    pub buf: Option<ErasedBufMut>,
}

impl Completion {
    /// Returns a completion with `result` and no buffer.
    #[must_use]
    pub fn new(result: Result<Output>) -> Self {
        Self { result, buf: None }
    }

    /// Returns the operation's buffer with the completion.
    #[must_use]
    pub fn with_buf(mut self, buf: ErasedBufMut) -> Self {
        self.buf = Some(buf);
        self
    }
}

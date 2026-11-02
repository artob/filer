// This is free and unencumbered software released into the public domain.

//! Reading from the source.

use alloc::vec::Vec;
use filer_api::{BufResult, Error, ReadAt};

/// Reads exactly `len` bytes of `source` at `off`. Callers bound `len` by
/// a block, an inode, or a read's buffer.
pub(crate) async fn read_exact<S: ReadAt + ?Sized>(
    source: &S,
    off: u64,
    len: usize,
) -> Result<Vec<u8>, Error> {
    let BufResult(result, buf) = source.read_exact_at(Vec::with_capacity(len), off).await;
    result.map(|()| buf)
}

// This is free and unencumbered software released into the public domain.

use alloc::vec::Vec;
use filer_api::{BufResult, Error, OpKind, ReadAt};

/// Reads exactly `len` bytes of `source` at `off`, for the operation `op`.
///
/// Callers bound `len`, by a block, a header, or a read's buffer: it is
/// allocated up front. An error reading `source` keeps its kind but is
/// reported as from `op`, the operation that needed the bytes, as it is
/// what the caller asked for; reading past the end is
/// [`UnexpectedEof`](filer_api::ErrorKind::UnexpectedEof), which a driver
/// that checked the source holds the volume can let stand.
///
/// # Errors
///
/// Any error reading `source`.
///
/// # Examples
///
/// ```
/// use filer_api::{ErrorKind, OpKind};
///
/// # filer_spec::block_on(async {
/// let volume = vec![1, 2, 3, 4];
/// let bytes = filer_fs::read_exact(&volume, 1, 2, OpKind::Open).await?;
/// assert_eq!(bytes, [2, 3]);
/// let error = filer_fs::read_exact(&volume, 3, 2, OpKind::Open).await.unwrap_err();
/// assert_eq!((error.kind(), error.op()), (ErrorKind::UnexpectedEof, OpKind::Open));
/// # Ok::<_, filer_api::Error>(())
/// # }).unwrap();
/// ```
pub async fn read_exact<S: ReadAt + ?Sized>(
    source: &S,
    off: u64,
    len: usize,
    op: OpKind,
) -> Result<Vec<u8>, Error> {
    let BufResult(result, bytes) = source.read_exact_at(Vec::with_capacity(len), off).await;
    result.map_err(|error| match error.op() {
        OpKind::Read if op != OpKind::Read => Error::new(error.kind(), op),
        _ => error,
    })?;
    Ok(bytes)
}

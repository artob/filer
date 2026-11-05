// This is free and unencumbered software released into the public domain.

//! Reading the image's structures, where they are said to be.

use alloc::vec::Vec;
use filer_api::{Error, ErrorKind, OpKind, ReadAt};

/// Reads `len` bytes at `at` of `source`, as [`filer_fs::read_exact`]
/// does, but for a read past its end, where only a corrupt image points:
/// that is the error `bad` returns.
pub(crate) async fn read<S: ReadAt + ?Sized>(
    source: &S,
    at: u64,
    len: usize,
    op: OpKind,
    bad: impl FnOnce() -> Error,
) -> Result<Vec<u8>, Error> {
    filer_fs::read_exact(source, at, len, op)
        .await
        .map_err(|error| {
            if error.kind() == ErrorKind::UnexpectedEof {
                bad()
            } else {
                error
            }
        })
}

// This is free and unencumbered software released into the public domain.

//! The extended-attribute operations [`File`](crate::File) and
//! [`Dir`](crate::Dir) share, without rights checks or error context,
//! which each adds.

use crate::{
    BufResult, Completion, Driver, ErasedBufMut, Error, ErrorKind, Handle, IoBufMut, Op, OpKind,
    Output, XattrEntry, XattrOp, dir::unexpected,
};
use alloc::vec::Vec;

/// The capacity of the first buffer [`read`] reads into, enough for most
/// attributes.
const FIRST_READ: usize = 256;

/// How many times [`read`] retries a value that keeps growing.
const ATTEMPTS: usize = 4;

/// Converts a name argument, mapping a conversion error to
/// [`InvalidName`](ErrorKind::InvalidName).
pub(crate) fn name<N>(name: N, op: OpKind) -> crate::Result<XattrEntry>
where
    N: TryInto<XattrEntry>,
    N::Error: core::error::Error + Send + Sync + 'static,
{
    name.try_into()
        .map_err(|error| Error::new(ErrorKind::InvalidName, op).with_source(error))
}

/// Reads as much of the value of `name` as fits into `buf`.
///
/// # Panics
///
/// Panics if the driver breaks its contract by not returning the buffer.
pub(crate) async fn get<D: Driver, B: IoBufMut>(
    driver: &D,
    handle: Handle,
    name: XattrEntry,
    buf: B,
) -> BufResult<Option<u64>, B> {
    let buf = ErasedBufMut::new(buf);
    let op = Op::Xattr(XattrOp::Get { handle, name, buf });
    let Completion { result, buf, .. } = driver.submit(op).await;
    let buf = buf
        .and_then(|buf| buf.downcast::<B>().ok())
        .expect("driver returned the attribute buffer");
    let result = match result {
        Ok(Output::Xattr(len)) => Ok(len),
        Ok(output) => Err(unexpected(output, OpKind::GetXattr)),
        Err(error) => Err(error),
    };
    BufResult(result, buf)
}

/// Reads the whole value of `name`, or `None` if it does not exist.
pub(crate) async fn read<D: Driver>(
    driver: &D,
    handle: &Handle,
    name: &XattrEntry,
) -> crate::Result<Option<Vec<u8>>> {
    let op = OpKind::GetXattr;
    let mut want = FIRST_READ;
    for _ in 0..ATTEMPTS {
        let mut buf = Vec::new();
        // A value too large to allocate is an error, not an abort.
        buf.try_reserve_exact(want)
            .map_err(|_| Error::new(ErrorKind::LimitExceeded, op))?;
        let BufResult(result, buf) = get(driver, handle.clone(), name.clone(), buf).await;
        let Some(len) = result? else {
            return Ok(None);
        };
        if len <= buf.len() as u64 {
            return Ok(Some(buf));
        }
        want = usize::try_from(len).map_err(|_| Error::new(ErrorKind::LimitExceeded, op))?;
    }
    // The value grew between attempts each time, as it may under
    // concurrent writes.
    Err(Error::new(ErrorKind::Interrupted, op))
}

/// Lists the attributes of `handle`.
pub(crate) async fn list<D: Driver>(driver: &D, handle: Handle) -> crate::Result<Vec<XattrEntry>> {
    let op = Op::Xattr(XattrOp::List { handle });
    let Completion { result, .. } = driver.submit(op).await;
    match result? {
        Output::XattrNames(names) => Ok(names),
        output => Err(unexpected(output, OpKind::ListXattrs)),
    }
}

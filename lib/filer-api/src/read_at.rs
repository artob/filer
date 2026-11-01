// This is free and unencumbered software released into the public domain.

use crate::{
    BufResult, Driver, Error, ErrorKind, File, IoBufMut, MetaMask, OpKind, Readable, Result,
};
use core::future::Future;

/// A source of bytes read by position, with a known size: what archive
/// drivers such as `filer-tar` and `filer-zip` read from.
///
/// It is implemented by readable [`File`]s, so an archive can sit in any
/// driver, and by in-memory bytes (anything that is `AsRef<[u8]>`:
/// `&'static [u8]` from `include_bytes!`, `Vec<u8>`, `Arc<[u8]>`, and so
/// on), so archives also work without a filesystem.
///
/// # Examples
///
/// ```
/// use filer_api::{BufResult, ReadAt};
/// # filer_api_block_on(async {
/// let source: &'static [u8] = b"hello, world";
/// assert_eq!(source.size().await?, 12);
/// let BufResult(result, buf) = source.read_exact_at(Vec::with_capacity(5), 7).await;
/// result?;
/// assert_eq!(buf, b"world");
/// # Ok::<(), filer_api::Error>(())
/// # }).unwrap();
/// # fn filer_api_block_on<F: core::future::Future>(f: F) -> F::Output {
/// #     let mut f = core::pin::pin!(f);
/// #     let mut cx = core::task::Context::from_waker(core::task::Waker::noop());
/// #     loop {
/// #         if let core::task::Poll::Ready(v) = f.as_mut().poll(&mut cx) {
/// #             return v;
/// #         }
/// #     }
/// # }
/// ```
pub trait ReadAt: Send + Sync {
    /// Reads into the spare capacity of `buf` from offset `off`, returning
    /// the number of bytes read, zero at the end, and the buffer.
    fn read_at<B: IoBufMut>(
        &self,
        buf: B,
        off: u64,
    ) -> impl Future<Output = BufResult<usize, B>> + Send;

    /// Returns the length in bytes.
    ///
    /// Named `size` rather than `len` because the trait is implemented for
    /// slice references, where a `len` method would shadow the slice's own
    /// wherever the trait is in scope.
    ///
    /// # Errors
    ///
    /// Any error determining it, such as a driver that does not report
    /// file lengths ([`Unsupported`](ErrorKind::Unsupported)).
    fn size(&self) -> impl Future<Output = Result<u64>> + Send;

    /// Reads from offset `off` until the spare capacity of `buf` is full.
    ///
    /// # Errors
    ///
    /// [`UnexpectedEof`](ErrorKind::UnexpectedEof) if the source ends
    /// first, with the bytes read so far left in the buffer; any error from
    /// [`read_at`](Self::read_at).
    fn read_exact_at<B: IoBufMut>(
        &self,
        mut buf: B,
        mut off: u64,
    ) -> impl Future<Output = BufResult<(), B>> + Send {
        async move {
            while buf.spare() > 0 {
                let BufResult(result, returned) = self.read_at(buf, off).await;
                buf = returned;
                match result {
                    Ok(0) => {
                        let error = Error::new(ErrorKind::UnexpectedEof, OpKind::Read);
                        return BufResult(Err(error), buf);
                    },
                    Ok(n) => off += n as u64,
                    Err(error) => return BufResult(Err(error), buf),
                }
            }
            BufResult(Ok(()), buf)
        }
    }
}

impl<M: Readable, D: Driver> ReadAt for File<M, D> {
    fn read_at<B: IoBufMut>(
        &self,
        buf: B,
        off: u64,
    ) -> impl Future<Output = BufResult<usize, B>> + Send {
        File::read_at(self, buf, off)
    }

    async fn size(&self) -> Result<u64> {
        self.metadata(MetaMask::LEN)
            .await?
            .len
            .ok_or_else(|| Error::new(ErrorKind::Unsupported, OpKind::Stat))
    }
}

impl<T: AsRef<[u8]> + Send + Sync + ?Sized> ReadAt for T {
    fn read_at<B: IoBufMut>(
        &self,
        mut buf: B,
        off: u64,
    ) -> impl Future<Output = BufResult<usize, B>> + Send {
        let bytes = self.as_ref();
        let start = usize::try_from(off).unwrap_or(usize::MAX);
        let data = bytes.get(start..).unwrap_or_default();
        let result = buf.fill(&mut |spare| {
            let n = spare.len().min(data.len());
            spare[..n].copy_from_slice(&data[..n]);
            Ok(n)
        });
        core::future::ready(BufResult(result, buf))
    }

    fn size(&self) -> impl Future<Output = Result<u64>> + Send {
        core::future::ready(Ok(self.as_ref().len() as u64))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Dir,
        driver::tests::{Toy, block_on},
    };
    use alloc::{sync::Arc, vec, vec::Vec};

    /// Reads all of `source` through the trait, in small pieces.
    fn read_all<S: ReadAt + ?Sized>(source: &S) -> Vec<u8> {
        let len = block_on(source.size()).unwrap();
        let mut all = Vec::new();
        while (all.len() as u64) < len {
            let BufResult(result, buf) =
                block_on(source.read_at(Vec::with_capacity(3), all.len() as u64));
            assert!(result.unwrap() > 0);
            all.extend(buf);
        }
        all
    }

    #[test]
    fn reads_in_memory_bytes() {
        let bytes: &'static [u8] = b"in memory";
        assert_eq!(read_all(bytes), b"in memory");
        assert_eq!(read_all(&vec![1_u8, 2, 3, 4]), [1, 2, 3, 4]);
        let shared: Arc<[u8]> = Arc::from(&b"shared"[..]);
        assert_eq!(read_all(&shared), b"shared");
        assert_eq!(read_all(&b""[..]), b"");
    }

    #[test]
    fn leaves_slice_len_alone() {
        // With `ReadAt` in scope, `len` must still be the slice's own.
        let bytes: &[u8] = b"ab";
        let len: usize = bytes.len();
        assert_eq!(len, 2);
    }

    #[test]
    fn reads_past_the_end_as_empty() {
        let BufResult(result, buf) = block_on(b"abc".read_at(Vec::with_capacity(4), u64::MAX));
        assert_eq!(result.unwrap(), 0);
        assert!(buf.is_empty());
    }

    #[test]
    fn reads_exactly_or_fails_at_the_end() {
        let source = b"hello";
        let BufResult(result, buf) = block_on(source.read_exact_at(Vec::with_capacity(4), 1));
        result.unwrap();
        assert_eq!(buf, b"ello");
        let BufResult(result, buf) = block_on(source.read_exact_at(Vec::with_capacity(4), 3));
        assert_eq!(result.unwrap_err().kind(), ErrorKind::UnexpectedEof);
        assert_eq!(buf, b"lo");
    }

    #[test]
    fn reads_files() {
        let (toy, root) = Toy::new(&[("a", b"from a file")]);
        let dir = Dir::new(toy, root).unwrap();
        let file = block_on(dir.open_file("a")).unwrap();
        assert_eq!(read_all(&file), b"from a file");
    }
}

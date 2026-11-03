// This is free and unencumbered software released into the public domain.

use alloc::sync::Arc;
use core::{
    future::Future,
    sync::atomic::{AtomicUsize, Ordering},
};
use filer_api::{BufResult, IoBufMut, ReadAt, Result};

/// A source counting the reads made of it, for tests of how a driver reads
/// its source, such as how many reads opening an archive takes.
///
/// # Examples
///
/// ```
/// use core::sync::atomic::Ordering;
/// use filer_api::ReadAt;
/// use filer_spec::{Counted, block_on};
///
/// let source = Counted::new(b"hello".to_vec());
/// let reads = source.reads();
/// block_on(source.read_exact_at(Vec::with_capacity(5), 0)).0?;
/// assert_eq!(reads.load(Ordering::Relaxed), 1);
/// # Ok::<(), filer_api::Error>(())
/// ```
#[derive(Debug)]
pub struct Counted<S> {
    inner: S,
    reads: Arc<AtomicUsize>,
}

impl<S> Counted<S> {
    /// Returns `inner`, counting its reads from none.
    #[must_use]
    pub fn new(inner: S) -> Self {
        Self {
            inner,
            reads: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Returns the count of reads, which goes on counting after the source
    /// moves into a driver.
    #[must_use]
    pub fn reads(&self) -> Arc<AtomicUsize> {
        self.reads.clone()
    }
}

impl<S: ReadAt> ReadAt for Counted<S> {
    fn read_at<B: IoBufMut>(
        &self,
        buf: B,
        off: u64,
    ) -> impl Future<Output = BufResult<usize, B>> + Send {
        self.reads.fetch_add(1, Ordering::Relaxed);
        self.inner.read_at(buf, off)
    }

    fn size(&self) -> impl Future<Output = Result<u64>> + Send {
        self.inner.size()
    }
}

// This is free and unencumbered software released into the public domain.

use crate::{
    BufResult, Completion, Driver, DynDriver, ErasedBufMut, Error, ErrorKind, Handle, IoBufMut,
    MetaMask, Metadata, Mode, Op, OpKind, Output, ReadOnly, Readable, Rights, Target, XattrEntry,
    dir::unexpected, xattr,
};
use alloc::{sync::Arc, vec::Vec};
use core::marker::PhantomData;

/// The capacity of the first buffer [`File::read_to_end`] reads into.
const FIRST_READ: usize = 8 * 1024;

/// An open file, its [`Mode`] fixed in its type.
///
/// I/O is positional: every read names its offset, and there is no shared
/// cursor, so one `File` can serve concurrent reads. Reads take an owned
/// buffer ([`IoBufMut`]) and give it back in a [`BufResult`], whatever the
/// outcome, so that completion-based drivers can fill it safely.
///
/// Like [`Dir`](crate::Dir), methods borrow the file, cloning is cheap, and
/// dropping a returned future does not cancel an operation the driver has
/// started. The OS file is closed when the last clone is dropped, or
/// explicitly with [`close`](Self::close).
///
/// # Examples
///
/// ```
/// # use filer_api::{BufResult, Driver, File};
/// # async fn example<D: Driver>(file: File<filer_api::ReadOnly, D>) -> filer_api::Result<()> {
/// let BufResult(result, buf) = file.read_at(Vec::with_capacity(4096), 0).await;
/// let n = result?;
/// assert_eq!(buf.len(), n);
/// # Ok(())
/// # }
/// ```
pub struct File<M: Mode = ReadOnly, D: Driver = DynDriver> {
    driver: D,
    handle: Handle,
    rights: Rights,
    label: Option<Arc<str>>,
    mode: PhantomData<fn() -> M>,
}

impl<M: Mode, D: Driver> File<M, D> {
    pub(crate) fn new(driver: D, handle: Handle, rights: Rights, label: Option<Arc<str>>) -> Self {
        Self {
            driver,
            handle,
            rights,
            label,
            mode: PhantomData,
        }
    }

    /// Returns the driver.
    pub fn driver(&self) -> &D {
        &self.driver
    }

    /// Returns the driver's handle for this file.
    pub fn handle(&self) -> &Handle {
        &self.handle
    }

    /// Returns this file's rights.
    pub fn rights(&self) -> Rights {
        self.rights
    }

    /// Reads the file's metadata.
    ///
    /// # Errors
    ///
    /// [`RightsDenied`](ErrorKind::RightsDenied) without
    /// [`Rights::READ`]; any error from the driver.
    pub async fn metadata(&self, mask: MetaMask) -> crate::Result<Metadata> {
        let op = OpKind::Stat;
        self.check(Rights::READ, op)?;
        let target = Target::Handle(self.handle.clone());
        let Completion { result, .. } = self.driver.submit(Op::Stat { target, mask }).await;
        match result.map_err(|error| self.context(error))? {
            Output::Metadata(metadata) => Ok(metadata),
            output => Err(unexpected(output, op)),
        }
    }

    /// Reads the whole value of the extended attribute `name` of this
    /// file, such as `"user.comment"`, or `None` if it has no such
    /// attribute.
    ///
    /// # Errors
    ///
    /// [`InvalidName`](ErrorKind::InvalidName) if `name` is not an
    /// attribute name; [`RightsDenied`](ErrorKind::RightsDenied) without
    /// [`Rights::READ`]; [`Unsupported`](ErrorKind::Unsupported) if the
    /// driver has no extended attributes;
    /// [`LimitExceeded`](ErrorKind::LimitExceeded) if the value is too
    /// large to allocate; any error from the driver.
    pub async fn xattr<N>(&self, name: N) -> crate::Result<Option<Vec<u8>>>
    where
        N: TryInto<XattrEntry>,
        N::Error: core::error::Error + Send + Sync + 'static,
    {
        let op = OpKind::GetXattr;
        let name = xattr::name(name, op).map_err(|error| self.context(error))?;
        self.check(Rights::READ, op)?;
        xattr::read(&self.driver, &self.handle, &name)
            .await
            .map_err(|error| self.context(error))
    }

    /// Reads as much of the value of the extended attribute `name` of this
    /// file as fits into the spare capacity of `buf`, returning the
    /// value's whole length, or `None` if it has no such attribute. A
    /// length above the spare capacity means the value was cut short.
    ///
    /// # Errors
    ///
    /// As for [`xattr`](Self::xattr).
    ///
    /// # Panics
    ///
    /// Panics if the driver breaks its contract by not returning the
    /// buffer.
    pub async fn xattr_into<N, B>(&self, name: N, buf: B) -> BufResult<Option<u64>, B>
    where
        N: TryInto<XattrEntry>,
        N::Error: core::error::Error + Send + Sync + 'static,
        B: IoBufMut,
    {
        let op = OpKind::GetXattr;
        let name = match xattr::name(name, op) {
            Ok(name) => name,
            Err(error) => return BufResult(Err(self.context(error)), buf),
        };
        if let Err(error) = self.check(Rights::READ, op) {
            return BufResult(Err(error), buf);
        }
        let BufResult(result, buf) = xattr::get(&self.driver, self.handle.clone(), name, buf).await;
        BufResult(result.map_err(|error| self.context(error)), buf)
    }

    /// Lists the extended attributes of this file, in no particular
    /// order.
    ///
    /// # Errors
    ///
    /// [`RightsDenied`](ErrorKind::RightsDenied) without
    /// [`Rights::READ`]; [`Unsupported`](ErrorKind::Unsupported) if the
    /// driver has no extended attributes; any error from the driver.
    pub async fn xattr_names(&self) -> crate::Result<Vec<XattrEntry>> {
        let op = OpKind::ListXattrs;
        self.check(Rights::READ, op)?;
        xattr::list(&self.driver, self.handle.clone())
            .await
            .map_err(|error| self.context(error))
    }

    /// Closes the file, reporting errors that some filesystems (such as
    /// NFS) only surface at close. Other clones of this file become
    /// unusable.
    ///
    /// # Errors
    ///
    /// Any error the driver reports.
    pub async fn close(self) -> crate::Result<()> {
        let op = Op::Close {
            handle: self.handle.clone(),
        };
        let Completion { result, .. } = self.driver.submit(op).await;
        match result.map_err(|error| self.context(error))? {
            Output::Done => Ok(()),
            output => Err(unexpected(output, OpKind::Close)),
        }
    }

    fn check(&self, rights: Rights, op: OpKind) -> crate::Result<()> {
        if self.rights.contains(rights) {
            return Ok(());
        }
        Err(self.context(Error::new(ErrorKind::RightsDenied, op)))
    }

    fn context(&self, error: Error) -> Error {
        match &self.label {
            Some(label) => error.with_label(label),
            None => error,
        }
    }
}

impl<M: Readable, D: Driver> File<M, D> {
    /// Reads into the spare capacity of `buf` from offset `off`, returning
    /// the number of bytes read and the buffer.
    ///
    /// Reading fewer bytes than the spare capacity is normal; reading none
    /// into a buffer with spare capacity means end of file.
    ///
    /// # Errors
    ///
    /// [`RightsDenied`](ErrorKind::RightsDenied) without
    /// [`Rights::READ`]; any error from the driver.
    ///
    /// # Panics
    ///
    /// Panics if the driver breaks its contract by not returning the
    /// buffer.
    pub async fn read_at<B: IoBufMut>(&self, buf: B, off: u64) -> BufResult<usize, B> {
        if let Err(error) = self.check(Rights::READ, OpKind::Read) {
            return BufResult(Err(error), buf);
        }
        let op = Op::Read {
            file: self.handle.clone(),
            buf: ErasedBufMut::new(buf),
            off,
        };
        let Completion { result, buf, .. } = self.driver.submit(op).await;
        let buf = buf
            .and_then(|buf| buf.downcast::<B>().ok())
            .expect("driver returned the read buffer");
        let result = match result {
            Ok(Output::Read(n)) => Ok(n),
            Ok(output) => Err(unexpected(output, OpKind::Read)),
            Err(error) => Err(self.context(error)),
        };
        BufResult(result, buf)
    }

    /// Reads from offset `off` until the spare capacity of `buf` is full.
    ///
    /// # Errors
    ///
    /// [`UnexpectedEof`](ErrorKind::UnexpectedEof) if the file ends first,
    /// with the bytes read so far left in the buffer; any error from
    /// [`read_at`](Self::read_at).
    pub async fn read_exact_at<B: IoBufMut>(&self, mut buf: B, mut off: u64) -> BufResult<(), B> {
        while buf.spare() > 0 {
            let BufResult(result, returned) = self.read_at(buf, off).await;
            buf = returned;
            match result {
                Ok(0) => {
                    let error = Error::new(ErrorKind::UnexpectedEof, OpKind::Read);
                    return BufResult(Err(self.context(error)), buf);
                },
                Ok(n) => off += n as u64,
                Err(error) => return BufResult(Err(error), buf),
            }
        }
        BufResult(Ok(()), buf)
    }

    /// Reads the whole file, from offset 0 to its end.
    ///
    /// # Errors
    ///
    /// Any error from [`read_at`](Self::read_at).
    pub async fn read_to_end(&self) -> crate::Result<Vec<u8>> {
        let mut buf = Vec::with_capacity(FIRST_READ);
        loop {
            if buf.spare() == 0 {
                // Doubles, by `Vec`'s growth policy; between reads, so the
                // buffer never moves while a driver holds it.
                buf.reserve(buf.len());
            }
            let off = buf.len() as u64;
            let BufResult(result, returned) = self.read_at(buf, off).await;
            buf = returned;
            if result? == 0 {
                return Ok(buf);
            }
        }
    }
}

impl<M: Mode, D: Driver> Clone for File<M, D> {
    fn clone(&self) -> Self {
        Self::new(
            self.driver.clone(),
            self.handle.clone(),
            self.rights,
            self.label.clone(),
        )
    }
}

impl<M: Mode, D: Driver> core::fmt::Debug for File<M, D> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("File")
            .field("handle", &self.handle)
            .field("rights", &self.rights)
            .field("label", &self.label)
            .finish_non_exhaustive()
    }
}

// This is free and unencumbered software released into the public domain.

use crate::{Completion, DriverId, Op};
use core::future::Future;

/// A filesystem driver: anything that performs [`Op`]s.
///
/// Concrete drivers (an OS backend, an in-memory filesystem), middleware,
/// and routing drivers all implement this one trait, so they compose by
/// wrapping each other. The single entry point keeps the trait small and
/// stable: new operations are new [`Op`] variants, not new methods.
///
/// # Contract
///
/// - [`id`](Self::id) is fixed for the driver instance and its clones, and
///   comes from the instance's [`DriverKey`](crate::DriverKey).
/// - The future returned by [`submit`](Self::submit) owns everything it
///   needs: it borrows neither the driver nor anything else, so it may
///   outlive both. It does nothing until first polled.
/// - Once polled, an operation that changes the filesystem runs to
///   completion even if the future is dropped; dropping only discards the
///   result. Pure reads may be canceled instead.
/// - A handle or foreign name minted by another driver instance fails the
///   operation with [`CrossDriver`](crate::ErrorKind::CrossDriver) or
///   [`ForeignNameMismatch`](crate::ErrorKind::ForeignNameMismatch); it
///   never panics.
/// - The [`Completion`] returns the operation's buffer, if it had one,
///   whether or not the operation succeeded, and on success its
///   [`Output`](crate::Output) variant matches the operation. Operations
///   the driver does not implement, including those added after it was
///   written, complete with [`Op::unsupported`], which returns the buffer.
///
/// # Implementing
///
/// The trait declares the future as `+ Send + use<Self>`; implementations
/// write `+ Send + use<>`, which proves the future borrows nothing. Clone
/// whatever state the future needs into it:
///
/// ```
/// use core::future::Future;
/// use filer_api::{Completion, Driver, DriverId, DriverKey, Op};
/// use std::sync::Arc;
///
/// /// A driver that supports nothing.
/// #[derive(Clone)]
/// struct Nothing(Arc<DriverKey>);
///
/// impl Driver for Nothing {
///     fn id(&self) -> DriverId {
///         self.0.id()
///     }
///
///     fn submit(&self, op: Op) -> impl Future<Output = Completion> + Send + use<> {
///         // Returns the buffer a read carries, as every completion must.
///         let completion = op.unsupported();
///         async move { completion }
///     }
/// }
/// ```
pub trait Driver: Clone + Send + Sync + 'static {
    /// Returns the identity of this driver instance.
    fn id(&self) -> DriverId;

    /// Performs `op`, resolving to its [`Completion`].
    fn submit(&self, op: Op) -> impl Future<Output = Completion> + Send + use<Self>;
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::{
        DriverKey, ErasedBufMut, Error, ErrorKind, FileKind, Handle, HandleKind, IoBufMut,
        MetaMask, Metadata, OpKind, OpenHow, Output, Target,
        path::{Name, NameBuf},
        rel,
    };
    use alloc::{borrow::ToOwned, collections::BTreeMap, sync::Arc, vec::Vec};
    use core::task::{Context, Poll, Waker};

    /// Polls `future` to completion on the current thread.
    pub(crate) fn block_on<F: Future>(future: F) -> F::Output {
        let mut future = core::pin::pin!(future);
        let mut cx = Context::from_waker(Waker::noop());
        loop {
            if let Poll::Ready(output) = future.as_mut().poll(&mut cx) {
                return output;
            }
        }
    }

    /// A flat, read-only, in-memory driver with one root directory.
    #[derive(Clone)]
    pub(crate) struct Toy {
        key: Arc<DriverKey>,
        files: Arc<BTreeMap<NameBuf, Vec<u8>>>,
    }

    /// A toy handle's payload: the root, or a file by name.
    enum Node {
        Root,
        File(NameBuf),
    }

    impl Toy {
        pub(crate) fn new(files: &[(&str, &[u8])]) -> (Self, Handle) {
            let key = DriverKey::new();
            let root = key.handle(HandleKind::Dir, Node::Root);
            let files = files
                .iter()
                .map(|(name, data)| (name.parse().unwrap(), data.to_vec()))
                .collect();
            let toy = Self {
                key: Arc::new(key),
                files: Arc::new(files),
            };
            (toy, root)
        }

        fn node<'h>(&self, handle: &'h Handle, op: OpKind) -> Result<&'h Node, Error> {
            self.key
                .payload(handle)
                .ok_or_else(|| Error::new(ErrorKind::CrossDriver, op))
        }

        fn file(&self, name: &Name, op: OpKind) -> Result<&Vec<u8>, Error> {
            self.files
                .get(name)
                .ok_or_else(|| Error::new(ErrorKind::NotFound, op).with_path(name))
        }

        fn perform(&self, op: Op) -> Completion {
            let kind = op.kind();
            match op {
                Op::Open { at, path, how } => {
                    let result = self.node(&at, kind).and_then(|_| {
                        let mut names = path.components();
                        match (names.next(), names.next(), how.kind) {
                            (Some(name), None, HandleKind::File) => {
                                self.file(name, kind)?;
                                let node = Node::File(name.to_owned());
                                Ok(Output::Opened(self.key.handle(HandleKind::File, node)))
                            },
                            _ => Err(Error::new(ErrorKind::Unsupported, kind)),
                        }
                    });
                    Completion::new(result)
                },
                Op::Read { file, mut buf, off } => {
                    let result = match self.node(&file, kind) {
                        Ok(Node::File(name)) => self.file(name, kind).and_then(|data| {
                            let data = data.get(off as usize..).unwrap_or_default();
                            buf.fill(&mut |spare| {
                                let n = spare.len().min(data.len());
                                spare[..n].copy_from_slice(&data[..n]);
                                Ok(n)
                            })
                            .map(Output::Read)
                        }),
                        Ok(Node::Root) => Err(Error::new(ErrorKind::IsADirectory, kind)),
                        Err(error) => Err(error),
                    };
                    Completion::new(result).with_buf(buf)
                },
                Op::Stat {
                    target: Target::Handle(handle),
                    ..
                } => {
                    let result = self.node(&handle, kind).and_then(|node| {
                        let mut metadata = Metadata::default();
                        match node {
                            Node::Root => metadata.kind = Some(FileKind::Dir),
                            Node::File(name) => {
                                metadata.kind = Some(FileKind::File);
                                metadata.len = Some(self.file(name, kind)?.len() as u64);
                            },
                        }
                        Ok(Output::Metadata(metadata))
                    });
                    Completion::new(result)
                },
                op => op.unsupported(),
            }
        }
    }

    impl Driver for Toy {
        fn id(&self) -> DriverId {
            self.key.id()
        }

        fn submit(&self, op: Op) -> impl Future<Output = Completion> + Send + use<> {
            let toy = self.clone();
            async move { toy.perform(op) }
        }
    }

    fn open<D: Driver>(driver: &D, root: &Handle, name: &str) -> Completion {
        block_on(driver.submit(Op::Open {
            at: root.clone(),
            path: name.parse().unwrap(),
            how: OpenHow::file(),
        }))
    }

    // Each check takes `wrap`, so it runs on `Toy` directly and through
    // wrappers such as `DynDriver`.

    pub(crate) fn check_open_read_stat<D: Driver>(wrap: impl Fn(Toy) -> D) {
        let (toy, root) = Toy::new(&[("a.txt", b"hello")]);
        let driver = wrap(toy);
        let Ok(Output::Opened(file)) = open(&driver, &root, "a.txt").result else {
            panic!("open failed");
        };
        let read = block_on(driver.submit(Op::Read {
            file: file.clone(),
            buf: ErasedBufMut::new(Vec::<u8>::with_capacity(4)),
            off: 1,
        }));
        assert!(matches!(read.result, Ok(Output::Read(4))));
        let buf: Vec<u8> = read.buf.unwrap().downcast().unwrap();
        assert_eq!(buf, b"ello");
        let stat = block_on(driver.submit(Op::Stat {
            target: Target::Handle(file),
            mask: MetaMask::all(),
        }));
        let Ok(Output::Metadata(metadata)) = stat.result else {
            panic!("stat failed");
        };
        assert_eq!(
            (metadata.kind, metadata.len),
            (Some(FileKind::File), Some(5))
        );
    }

    pub(crate) fn check_missing_entries<D: Driver>(wrap: impl Fn(Toy) -> D) {
        let (toy, root) = Toy::new(&[]);
        let error = open(&wrap(toy), &root, "nope").result.unwrap_err();
        assert_eq!(
            (error.kind(), error.op()),
            (ErrorKind::NotFound, OpKind::Open)
        );
        assert_eq!(error.path(), Some(rel!("nope")));
    }

    pub(crate) fn check_cross_driver<D: Driver>(wrap: impl Fn(Toy) -> D) {
        let (toy, _) = Toy::new(&[]);
        let (_, foreign_root) = Toy::new(&[]);
        let read = block_on(wrap(toy).submit(Op::Read {
            file: foreign_root,
            buf: ErasedBufMut::new(Vec::<u8>::with_capacity(4)),
            off: 0,
        }));
        assert_eq!(read.result.unwrap_err().kind(), ErrorKind::CrossDriver);
        assert!(read.buf.unwrap().is::<Vec<u8>>());
    }

    pub(crate) fn check_detached_futures<D: Driver>(wrap: impl Fn(Toy) -> D) {
        fn detach<D: Driver>(
            driver: D,
            op: Op,
        ) -> impl Future<Output = Completion> + Send + 'static {
            let future = driver.submit(op);
            drop(driver);
            future
        }
        let (toy, root) = Toy::new(&[("a", b"")]);
        let op = Op::Stat {
            target: Target::Handle(root),
            mask: MetaMask::KIND,
        };
        assert!(block_on(detach(wrap(toy), op)).result.is_ok());
    }

    #[test]
    fn opens_reads_and_stats() {
        check_open_read_stat(|toy| toy);
    }

    #[test]
    fn reports_missing_entries() {
        check_missing_entries(|toy| toy);
    }

    #[test]
    fn rejects_handles_from_other_drivers_and_returns_the_buffer() {
        check_cross_driver(|toy| toy);
    }

    #[test]
    fn futures_outlive_their_driver() {
        check_detached_futures(|toy| toy);
    }
}

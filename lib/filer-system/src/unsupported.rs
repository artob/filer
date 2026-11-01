// This is free and unencumbered software released into the public domain.

use core::future::Future;
use filer_api::{
    AmbientAuthority, Completion, Driver, DriverId, DriverKey, Error, ErrorKind, Handle, Op, OpKind,
};
use std::{path::Path, sync::Arc};

/// The driver for targets without a native one yet: every operation fails
/// with [`ErrorKind::Unsupported`], so that portable code still compiles.
#[derive(Clone, Debug)]
pub(crate) struct Unsupported {
    key: Arc<DriverKey>,
}

impl Unsupported {
    pub(crate) fn blocking() -> Self {
        Self {
            key: Arc::new(DriverKey::new()),
        }
    }

    pub(crate) fn open_ambient_dir(
        &self,
        path: &Path,
        authority: AmbientAuthority,
    ) -> Result<Handle, Error> {
        let _ = authority;
        Err(Error::new(ErrorKind::Unsupported, OpKind::Open).with_label(&path.to_string_lossy()))
    }
}

impl Driver for Unsupported {
    fn id(&self) -> DriverId {
        self.key.id()
    }

    fn submit(&self, op: Op) -> impl Future<Output = Completion> + Send + use<> {
        let error = Error::new(ErrorKind::Unsupported, op.kind());
        async move { Completion::new(Err(error)) }
    }
}

// This is free and unencumbered software released into the public domain.

use crate::{DriverId, HandleKind};
use alloc::sync::Arc;
use core::any::Any;

/// A driver's reference to an open file or directory.
///
/// A handle records the [`DriverId`] of the driver instance that minted it,
/// its [`HandleKind`], and a driver-private payload, such as an `OwnedFd`
/// or an in-memory inode number. Only the minting driver can read the
/// payload, through its [`DriverKey`](crate::DriverKey); handles cannot be
/// forged for another driver.
///
/// Cloning is a reference-count increment. The payload is dropped, which
/// for an OS driver closes the descriptor, when the last clone is dropped,
/// on whatever thread drops it.
///
/// # Examples
///
/// ```
/// use filer_api::{DriverKey, HandleKind};
///
/// let key = DriverKey::new();
/// let handle = key.handle(HandleKind::Dir, 42_u64);
/// assert_eq!(handle.driver(), key.id());
/// assert_eq!(key.payload::<u64>(&handle), Some(&42));
/// assert_eq!(DriverKey::new().payload::<u64>(&handle), None);
/// ```
#[derive(Clone)]
pub struct Handle {
    driver: DriverId,
    kind: HandleKind,
    payload: Arc<dyn Any + Send + Sync>,
}

impl Handle {
    /// Returns a handle; see [`DriverKey::handle`](crate::DriverKey::handle).
    pub(crate) fn new<P: Any + Send + Sync>(
        driver: DriverId,
        kind: HandleKind,
        payload: P,
    ) -> Self {
        Self {
            driver,
            kind,
            payload: Arc::new(payload),
        }
    }

    /// Returns the id of the driver instance that minted this handle.
    #[must_use]
    pub fn driver(&self) -> DriverId {
        self.driver
    }

    /// Returns what this handle refers to.
    #[must_use]
    pub fn kind(&self) -> HandleKind {
        self.kind
    }

    /// Returns the payload if it is a `P`; see
    /// [`DriverKey::payload`](crate::DriverKey::payload).
    pub(crate) fn payload<P: Any>(&self) -> Option<&P> {
        self.payload.downcast_ref()
    }
}

impl core::fmt::Debug for Handle {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Handle")
            .field("driver", &self.driver)
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

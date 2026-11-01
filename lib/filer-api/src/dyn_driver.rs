// This is free and unencumbered software released into the public domain.

use crate::{Completion, Driver, DriverId, Op};
use alloc::{boxed::Box, sync::Arc};
use core::{any::Any, future::Future, pin::Pin};

/// A boxed, `Send` future resolving to a [`Completion`].
type BoxedCompletion = Pin<Box<dyn Future<Output = Completion> + Send>>;

/// The object-safe form of [`Driver`], implemented for every driver.
///
/// Its methods have their own names because the blanket impl also covers
/// [`DynDriver`], whose `Driver` methods would otherwise be ambiguous.
trait ErasedDriver: Send + Sync {
    fn erased_id(&self) -> DriverId;
    fn erased_submit(&self, op: Op) -> BoxedCompletion;
}

impl<D: Driver> ErasedDriver for D {
    fn erased_id(&self) -> DriverId {
        Driver::id(self)
    }

    fn erased_submit(&self, op: Op) -> BoxedCompletion {
        Box::pin(Driver::submit(self, op))
    }
}

/// A driver of any type, for when the concrete type is not known or not
/// wanted, such as in a mount table holding several kinds of drivers.
///
/// `DynDriver` is itself a [`Driver`] that forwards to the wrapped one,
/// with the same [`id`](Driver::id), so handles and foreign names work
/// through either. Cloning is a reference-count increment; each submitted
/// operation costs one boxed future.
///
/// # Examples
///
/// ```
/// use core::future::Future;
/// use filer_api::{Completion, Driver, DriverId, DriverKey, DynDriver, Error, ErrorKind, Op};
/// use std::sync::Arc;
///
/// #[derive(Clone)]
/// struct Nothing(Arc<DriverKey>);
///
/// impl Driver for Nothing {
///     fn id(&self) -> DriverId {
///         self.0.id()
///     }
///
///     fn submit(&self, op: Op) -> impl Future<Output = Completion> + Send + use<> {
///         let error = Error::new(ErrorKind::Unsupported, op.kind());
///         async move { Completion::new(Err(error)) }
///     }
/// }
///
/// let driver = Nothing(Arc::new(DriverKey::new()));
/// let id = driver.id();
/// let erased = DynDriver::new(driver);
/// assert_eq!(erased.id(), id);
/// ```
#[derive(Clone)]
pub struct DynDriver(Arc<dyn ErasedDriver>);

impl DynDriver {
    /// Erases `driver`'s type. Erasing a `DynDriver` returns it as is,
    /// rather than wrapping it again.
    pub fn new<D: Driver>(driver: D) -> Self {
        let any: &dyn Any = &driver;
        if let Some(erased) = any.downcast_ref::<Self>() {
            return erased.clone();
        }
        Self(Arc::new(driver))
    }
}

impl Driver for DynDriver {
    fn id(&self) -> DriverId {
        self.0.erased_id()
    }

    fn submit(&self, op: Op) -> impl Future<Output = Completion> + Send + use<> {
        self.0.erased_submit(op)
    }
}

impl core::fmt::Debug for DynDriver {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("DynDriver").field(&self.id()).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::driver::tests::{
        Toy, check_cross_driver, check_detached_futures, check_missing_entries,
        check_open_read_stat,
    };

    #[test]
    fn opens_reads_and_stats() {
        check_open_read_stat(DynDriver::new);
    }

    #[test]
    fn reports_missing_entries() {
        check_missing_entries(DynDriver::new);
    }

    #[test]
    fn rejects_handles_from_other_drivers_and_returns_the_buffer() {
        check_cross_driver(DynDriver::new);
    }

    #[test]
    fn futures_outlive_their_driver() {
        check_detached_futures(DynDriver::new);
    }

    #[test]
    fn keeps_the_driver_id() {
        let (toy, _) = Toy::new(&[]);
        assert_eq!(DynDriver::new(toy.clone()).id(), toy.id());
    }

    #[test]
    fn does_not_wrap_twice() {
        let (toy, _) = Toy::new(&[]);
        let once = DynDriver::new(toy);
        let twice = DynDriver::new(once.clone());
        assert!(Arc::ptr_eq(&once.0, &twice.0));
    }
}

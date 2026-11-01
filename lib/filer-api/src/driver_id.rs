// This is free and unencumbered software released into the public domain.

/// The identity of one driver instance.
///
/// Handles and [`ForeignName`](crate::ForeignName)s record the id of the
/// driver instance that produced them, and a driver refuses those carrying
/// another id. Ids are unique within a process and come only from
/// [`DriverKey::new`](crate::DriverKey::new); they are meaningless across
/// processes and are not serializable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DriverId(usize);

impl DriverId {
    /// Returns a fresh id, distinct from every id returned before.
    ///
    /// # Panics
    ///
    /// Panics if the id space is exhausted, which on 32-bit targets takes
    /// about four billion driver instances. A `usize` counter is used
    /// because targets such as `thumbv7em-none-eabihf` lack 64-bit atomics.
    pub(crate) fn next() -> Self {
        use core::sync::atomic::{AtomicUsize, Ordering::Relaxed};
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        // A hand-written `fetch_update`, which is deprecated in newer Rust
        // in favor of `try_update`, unavailable at our MSRV.
        let mut id = NEXT.load(Relaxed);
        loop {
            let next = id.checked_add(1).expect("driver id space exhausted");
            match NEXT.compare_exchange_weak(id, next, Relaxed, Relaxed) {
                Ok(_) => return Self(id),
                Err(actual) => id = actual,
            }
        }
    }
}

impl core::fmt::Display for DriverId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "driver#{}", self.0)
    }
}

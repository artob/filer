// This is free and unencumbered software released into the public domain.

// Cargo cannot leave a dependency out where a feature is on: `spin` locks
// only without `std`.
#[cfg(feature = "std")]
use spin as _;

/// A lock for short, synchronous critical sections that never span an
/// `.await` (`DESIGN.md` §7.7), such as a driver's cache of decoded
/// blocks: the standard mutex under `std`, and a spin lock without it.
///
/// # Examples
///
/// ```
/// use filer_fs::Lock;
///
/// let cache = Lock::new(Vec::new());
/// cache.with(|blocks| blocks.push(7));
/// assert_eq!(cache.with(|blocks| blocks.len()), 1);
/// ```
pub struct Lock<T>(
    #[cfg(feature = "std")] std::sync::Mutex<T>,
    #[cfg(not(feature = "std"))] spin::Mutex<T>,
);

impl<T> Lock<T> {
    /// Returns a lock holding `value`.
    pub const fn new(value: T) -> Self {
        #[cfg(feature = "std")]
        return Self(std::sync::Mutex::new(value));
        #[cfg(not(feature = "std"))]
        return Self(spin::Mutex::new(value));
    }

    /// Runs `f` on the locked value, and returns what it returns.
    pub fn with<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        // A panic while locked leaves the value as consistent as the
        // critical section left it, which callers keep short.
        #[cfg(feature = "std")]
        return f(&mut self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner));
        #[cfg(not(feature = "std"))]
        return f(&mut self.0.lock());
    }
}

impl<T: Default> Default for Lock<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T> core::fmt::Debug for Lock<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Lock").finish_non_exhaustive()
    }
}

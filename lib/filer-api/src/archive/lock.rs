// This is free and unencumbered software released into the public domain.

/// A lock for short, synchronous critical sections that never span an
/// `.await` (`DESIGN.md` §7.7): the standard mutex under `std`, and a spin
/// lock without it.
pub(crate) struct Lock<T>(
    #[cfg(feature = "std")] std::sync::Mutex<T>,
    #[cfg(not(feature = "std"))] spin::Mutex<T>,
);

impl<T> Lock<T> {
    pub(crate) fn new(value: T) -> Self {
        #[cfg(feature = "std")]
        return Self(std::sync::Mutex::new(value));
        #[cfg(not(feature = "std"))]
        return Self(spin::Mutex::new(value));
    }

    /// Runs `f` on the locked value.
    pub(crate) fn with<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        // A panic while locked leaves the value consistent here: every
        // critical section is a single collection operation.
        #[cfg(feature = "std")]
        return f(&mut self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner));
        #[cfg(not(feature = "std"))]
        return f(&mut self.0.lock());
    }
}

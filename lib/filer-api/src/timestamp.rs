// This is free and unencumbered software released into the public domain.

use core::time::Duration;

/// A point in time relative to the Unix epoch, with the granularity the
/// filesystem stores it at, when known.
///
/// Granularity matters for change detection: FAT stores modification times
/// in 2-second steps, so an edit within one step leaves the time unchanged.
/// Comparisons order by instant first, then by granularity, so equal
/// instants with different known granularities are not equal.
///
/// # Examples
///
/// ```
/// use core::time::Duration;
/// use filer_api::Timestamp;
///
/// let t = Timestamp::new(1_700_000_000, 500).with_granularity(Duration::from_secs(2));
/// assert_eq!((t.secs(), t.nanos()), (1_700_000_000, 500));
/// assert!(Timestamp::new(-1, 0) < Timestamp::new(0, 0));
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp {
    secs: i64,
    nanos: u32,
    granularity: Option<Duration>,
}

impl Timestamp {
    /// Returns the time `secs` seconds plus `nanos` nanoseconds after the
    /// Unix epoch; negative `secs` are before it.
    ///
    /// # Panics
    ///
    /// Panics if `nanos` is a full second or more.
    #[must_use]
    pub const fn new(secs: i64, nanos: u32) -> Self {
        assert!(nanos < 1_000_000_000, "nanoseconds out of range");
        Self {
            secs,
            nanos,
            granularity: None,
        }
    }

    /// Records the granularity the filesystem stores this time at.
    #[must_use]
    pub const fn with_granularity(mut self, granularity: Duration) -> Self {
        self.granularity = Some(granularity);
        self
    }

    /// Returns the whole seconds since the Unix epoch.
    #[must_use]
    pub const fn secs(&self) -> i64 {
        self.secs
    }

    /// Returns the nanoseconds past [`secs`](Self::secs), always below one
    /// second.
    #[must_use]
    pub const fn nanos(&self) -> u32 {
        self.nanos
    }

    /// Returns the granularity the filesystem stores this time at, if known.
    #[must_use]
    pub const fn granularity(&self) -> Option<Duration> {
        self.granularity
    }

    /// Converts to a [`std::time::SystemTime`], or `None` if the platform
    /// cannot represent this time.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn to_system_time(&self) -> Option<std::time::SystemTime> {
        let epoch = std::time::SystemTime::UNIX_EPOCH;
        let nanos = Duration::from_nanos(u64::from(self.nanos));
        if self.secs >= 0 {
            epoch
                .checked_add(Duration::from_secs(self.secs.unsigned_abs()))?
                .checked_add(nanos)
        } else {
            epoch
                .checked_sub(Duration::from_secs(self.secs.unsigned_abs()))?
                .checked_add(nanos)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic(expected = "nanoseconds out of range")]
    fn rejects_a_full_second_of_nanos() {
        let _ = Timestamp::new(0, 1_000_000_000);
    }

    #[test]
    fn orders_by_instant() {
        assert!(Timestamp::new(-1, 999_999_999) < Timestamp::new(0, 0));
        assert!(Timestamp::new(0, 1) > Timestamp::new(0, 0));
    }

    #[cfg(feature = "std")]
    #[test]
    fn converts_to_system_time() {
        use std::time::SystemTime;
        let after = Timestamp::new(1, 500).to_system_time().unwrap();
        assert_eq!(
            after.duration_since(SystemTime::UNIX_EPOCH).unwrap(),
            Duration::new(1, 500)
        );
        let before = Timestamp::new(-2, 500_000_000).to_system_time().unwrap();
        let gap = SystemTime::UNIX_EPOCH.duration_since(before).unwrap();
        assert_eq!(gap, Duration::from_millis(1500));
    }
}

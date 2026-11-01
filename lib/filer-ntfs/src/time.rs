// This is free and unencumbered software released into the public domain.

//! NTFS times: 100-nanosecond intervals since 1601-01-01, UTC.

use core::time::Duration;
use filer_api::Timestamp;

/// The granularity of NTFS times.
const GRANULARITY: Duration = Duration::from_nanos(100);

/// Seconds from 1601-01-01 to the Unix epoch.
const EPOCH_DIFFERENCE: i64 = 11_644_473_600;

const TICKS_PER_SECOND: u64 = 10_000_000;

/// Returns the time of NTFS time `ticks`.
pub(crate) fn timestamp(ticks: u64) -> Option<Timestamp> {
    let secs = i64::try_from(ticks / TICKS_PER_SECOND)
        .ok()?
        .checked_sub(EPOCH_DIFFERENCE)?;
    // Below a second, as the remainder is below 10^7 ticks.
    let nanos = u32::try_from(ticks % TICKS_PER_SECOND)
        .ok()?
        .checked_mul(100)?;
    Some(Timestamp::new(secs, nanos).with_granularity(GRANULARITY))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_ntfs_times() {
        let epoch = timestamp(116_444_736_000_000_000).unwrap();
        assert_eq!((epoch.secs(), epoch.nanos()), (0, 0));
        let t = timestamp(133_485_408_461_234_567).unwrap();
        assert_eq!((t.secs(), t.nanos()), (1_704_067_246, 123_456_700));
        assert_eq!(t.granularity(), Some(GRANULARITY));
        assert_eq!(timestamp(0).unwrap().secs(), -EPOCH_DIFFERENCE);
        assert!(timestamp(u64::MAX).is_some());
    }
}

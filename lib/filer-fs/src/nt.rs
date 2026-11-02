// This is free and unencumbered software released into the public domain.

//! Windows NT's conventions, which NTFS keeps on disk and Windows reports:
//! times as 100-nanosecond intervals since 1601-01-01, UTC, and file
//! attributes, some of them flags.
//!
//! # Examples
//!
//! ```
//! use filer_api::FileAttrs;
//! use filer_fs::nt;
//!
//! let epoch = nt::timestamp(116_444_736_000_000_000).unwrap();
//! assert_eq!((epoch.secs(), epoch.nanos()), (0, 0));
//! // Hidden, and a directory, which is a kind rather than a flag.
//! assert_eq!(nt::flags(0x12), FileAttrs::HIDDEN);
//! ```

use core::time::Duration;
use filer_api::{FileAttrs, Timestamp};

/// The granularity of NT times.
const GRANULARITY: Duration = Duration::from_nanos(100);

/// Seconds from 1601-01-01 to the Unix epoch.
const EPOCH_DIFFERENCE: i64 = 11_644_473_600;

const TICKS_PER_SECOND: u64 = 10_000_000;

/// The file attributes that are flags, and the flags they map to.
/// Read-only is a permission; directory and reparse point, the kind.
const FLAGS: [(u32, FileAttrs); 9] = [
    (0x0000_0002, FileAttrs::HIDDEN),
    (0x0000_0004, FileAttrs::SYSTEM),
    (0x0000_0020, FileAttrs::ARCHIVE),
    (0x0000_0200, FileAttrs::SPARSE),
    (0x0000_0800, FileAttrs::COMPRESSED),
    (0x0000_1000, FileAttrs::OFFLINE),
    (0x0000_4000, FileAttrs::ENCRYPTED),
    // Recall on open, and recall on data access.
    (0x0004_0000, FileAttrs::DATALESS),
    (0x0040_0000, FileAttrs::DATALESS),
];

/// Returns the time of NT time `ticks`, or `None` if it is out of range.
#[must_use]
pub fn timestamp(ticks: u64) -> Option<Timestamp> {
    let secs = i64::try_from(ticks / TICKS_PER_SECOND)
        .ok()?
        .checked_sub(EPOCH_DIFFERENCE)?;
    // Below a second, as the remainder is below 10^7 ticks.
    let nanos = u32::try_from(ticks % TICKS_PER_SECOND)
        .ok()?
        .checked_mul(100)?;
    Some(Timestamp::new(secs, nanos).with_granularity(GRANULARITY))
}

/// Returns the flags among the file attributes `attributes`.
#[must_use]
pub fn flags(attributes: u32) -> FileAttrs {
    let mut flags = FileAttrs::empty();
    for (bit, flag) in FLAGS {
        if attributes & bit != 0 {
            flags |= flag;
        }
    }
    flags
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_nt_times() {
        let epoch = timestamp(116_444_736_000_000_000).unwrap();
        assert_eq!((epoch.secs(), epoch.nanos()), (0, 0));
        let t = timestamp(133_485_408_461_234_567).unwrap();
        assert_eq!((t.secs(), t.nanos()), (1_704_067_246, 123_456_700));
        assert_eq!(t.granularity(), Some(GRANULARITY));
        assert_eq!(timestamp(0).unwrap().secs(), -EPOCH_DIFFERENCE);
        assert!(timestamp(u64::MAX).is_some());
    }

    #[test]
    fn maps_attributes_that_are_flags() {
        assert_eq!(flags(0), FileAttrs::empty());
        // Read-only, directory, and reparse point are not flags.
        assert_eq!(flags(0x0000_0411), FileAttrs::empty());
        assert_eq!(
            flags(0x0004_4826),
            FileAttrs::HIDDEN
                | FileAttrs::SYSTEM
                | FileAttrs::ARCHIVE
                | FileAttrs::COMPRESSED
                | FileAttrs::ENCRYPTED
                | FileAttrs::DATALESS
        );
    }
}

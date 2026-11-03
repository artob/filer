// This is free and unencumbered software released into the public domain.

//! DOS dates and times: local times with no zone, as FAT's directory
//! entries hold them, and after FAT exFAT's, ZIP's, and RAR 4's headers.
//!
//! A date packs the years since 1980, the month, and the day into 16
//! bits, and a time packs the hour, the minute, and the second, in
//! 2-second steps, into 16 more. FAT adds hundredths of a second, up to
//! 1.99 seconds, to creation times, and keeps access times as dates alone.
//! As no zone is recorded, a reader supplies the [`UtcOffset`] of the
//! clock that wrote them, and the granularity of the field it read, which
//! [`Timestamp`]s keep.
//!
//! A field may be unset or invalid without the volume or archive holding
//! it being corrupt: FAT volumes written before Windows 95 left creation
//! and access times zero, or reserved and arbitrary. [`Stamp`] keeps the
//! three cases apart.
//!
//! # Examples
//!
//! ```
//! use filer_msft::dos::{Stamp, TWO_SECONDS, UtcOffset};
//!
//! let date = (44 << 9) | (2 << 5) | 29; // 2024-02-29
//! let time = (12 << 11) | (34 << 5) | 28; // 12:34:56
//! let stamp = Stamp::decode(date, time, 0);
//! let utc = stamp.to_timestamp(UtcOffset::UTC, TWO_SECONDS).unwrap();
//! assert_eq!(utc.secs(), 1_709_210_096);
//! assert_eq!(Stamp::decode(0, 0, 0), Stamp::Unset);
//! ```

mod utc_offset;
pub use utc_offset::*;

use core::time::Duration;
use filer_api::Timestamp;

/// The granularity of a time: 2 seconds.
pub const TWO_SECONDS: Duration = Duration::from_secs(2);

/// The granularity of a time with hundredths of a second, such as FAT's
/// creation times: 10 milliseconds.
pub const TEN_MILLIS: Duration = Duration::from_millis(10);

/// The granularity of a date alone, such as FAT's access times: a day.
pub const ONE_DAY: Duration = Duration::from_secs(SECS_PER_DAY);

const SECS_PER_DAY: u64 = 86_400;

/// Days from 1970-01-01 to 1980-01-01, the DOS epoch.
const DAYS_TO_1980: i64 = 3652;

/// A valid local date and time, from 1980-01-01 to 2107-12-31, to
/// hundredths of a second.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DateTime {
    year: u16,
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
    second: u8,
    /// Hundredths of a second, below 100.
    centis: u8,
}

impl DateTime {
    /// Decodes a date, a time, and hundredths of a second past the time
    /// (up to 199), or returns `None` if any field is out of range.
    fn decode(date: u16, time: u16, centis: u8) -> Option<Self> {
        let year = 1980u16.checked_add(date >> 9)?;
        let month = u8::try_from((date >> 5) & 0x0F).ok()?;
        let day = u8::try_from(date & 0x1F).ok()?;
        let hour = u8::try_from(time >> 11).ok()?;
        let minute = u8::try_from((time >> 5) & 0x3F).ok()?;
        let second = u8::try_from(time & 0x1F).ok()?.checked_mul(2)?;
        if !(1..=12).contains(&month)
            || day == 0
            || day > days_in_month(year, month)
            || hour > 23
            || minute > 59
            || second > 58
            || centis > 199
        {
            return None;
        }
        Some(Self {
            year,
            month,
            day,
            hour,
            minute,
            second: second.checked_add(centis / 100)?,
            centis: centis % 100,
        })
    }

    /// Returns this local time, at `offset` from UTC, as a [`Timestamp`]
    /// with `granularity`, or returns `None` if it is out of range.
    #[must_use]
    pub fn to_timestamp(self, offset: UtcOffset, granularity: Duration) -> Option<Timestamp> {
        let mut days = DAYS_TO_1980;
        for year in 1980..self.year {
            days = days.checked_add(if is_leap(year) { 366 } else { 365 })?;
        }
        for month in 1..self.month {
            days = days.checked_add(i64::from(days_in_month(self.year, month)))?;
        }
        days = days.checked_add(i64::from(self.day))?.checked_sub(1)?;
        let secs = days
            .checked_mul(86_400)?
            .checked_add(i64::from(self.hour).checked_mul(3600)?)?
            .checked_add(i64::from(self.minute).checked_mul(60)?)?
            .checked_add(i64::from(self.second))?
            .checked_sub(i64::from(offset.seconds()))?;
        // Below a second, as `centis` is below 100.
        let nanos = u32::from(self.centis).checked_mul(10_000_000)?;
        Some(Timestamp::new(secs, nanos).with_granularity(granularity))
    }
}

const fn is_leap(year: u16) -> bool {
    year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))
}

const fn days_in_month(year: u16, month: u8) -> u8 {
    match month {
        2 if is_leap(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// A date and time field, as stored: unset, valid, or invalid.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Stamp {
    /// All zero: the time was never recorded.
    Unset,

    /// A valid date and time.
    Valid(DateTime),

    /// Out of range: a month or day of zero, an hour past 23, and so on.
    /// The raw fields are kept, as written.
    Invalid {
        /// The date, as written.
        date: u16,
        /// The time of day, as written.
        time: u16,
        /// The hundredths of a second past the time, as written.
        centis: u8,
    },
}

impl Stamp {
    /// Decodes a date, a time (zero for a date alone), and hundredths of a
    /// second past the time (zero where the field has none).
    #[must_use]
    pub fn decode(date: u16, time: u16, centis: u8) -> Self {
        if date == 0 && time == 0 && centis == 0 {
            return Self::Unset;
        }
        DateTime::decode(date, time, centis)
            .map_or(Self::Invalid { date, time, centis }, Self::Valid)
    }

    /// Returns the time as a [`Timestamp`], if it is valid; see
    /// [`DateTime::to_timestamp`].
    #[must_use]
    pub fn to_timestamp(self, offset: UtcOffset, granularity: Duration) -> Option<Timestamp> {
        match self {
            Self::Valid(time) => time.to_timestamp(offset, granularity),
            Self::Unset | Self::Invalid { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(year: u16, month: u16, day: u16) -> u16 {
        ((year - 1980) << 9) | (month << 5) | day
    }

    fn time(hour: u16, minute: u16, second: u16) -> u16 {
        (hour << 11) | (minute << 5) | (second / 2)
    }

    fn secs(stamp: Stamp) -> i64 {
        stamp
            .to_timestamp(UtcOffset::UTC, TWO_SECONDS)
            .unwrap()
            .secs()
    }

    #[test]
    fn converts_to_unix_time() {
        assert_eq!(secs(Stamp::decode(date(1980, 1, 1), 0, 0)), 315_532_800);
        // 2024-02-29T12:34:56Z, a leap day.
        let stamp = Stamp::decode(date(2024, 2, 29), time(12, 34, 56), 0);
        assert_eq!(secs(stamp), 1_709_210_096);
        assert_eq!(
            stamp
                .to_timestamp(UtcOffset::from_minutes(60).unwrap(), TWO_SECONDS)
                .unwrap()
                .secs(),
            1_709_210_096 - 3600
        );
        assert_eq!(
            secs(Stamp::decode(date(2107, 12, 31), time(23, 59, 58), 0)),
            4_354_819_198
        );
    }

    #[test]
    fn adds_hundredths_to_creation_times() {
        let stamp = Stamp::decode(date(2000, 1, 1), time(0, 0, 58), 199);
        let t = stamp.to_timestamp(UtcOffset::UTC, TEN_MILLIS).unwrap();
        assert_eq!((t.secs(), t.nanos()), (946_684_859, 990_000_000));
        assert_eq!(t.granularity(), Some(TEN_MILLIS));
    }

    #[test]
    fn keeps_unset_and_invalid_fields_apart() {
        assert_eq!(Stamp::decode(0, 0, 0), Stamp::Unset);
        for (d, t, c) in [
            (0, time(1, 0, 0), 0),
            (date(1990, 0, 1), 0, 0),
            (date(1990, 13, 1), 0, 0),
            (date(1990, 1, 0), 0, 0),
            (date(1990, 2, 29), 0, 0),
            (date(1990, 4, 31), 0, 0),
            (date(1990, 1, 1), time(24, 0, 0), 0),
            (date(1990, 1, 1), time(0, 60, 0), 0),
            (date(1990, 1, 1), 30, 0),
            (date(1990, 1, 1), 0, 200),
        ] {
            let stamp = Stamp::decode(d, t, c);
            assert_eq!(
                stamp,
                Stamp::Invalid {
                    date: d,
                    time: t,
                    centis: c
                }
            );
            assert_eq!(stamp.to_timestamp(UtcOffset::UTC, ONE_DAY), None);
        }
        assert!(matches!(
            Stamp::decode(date(2000, 2, 29), 0, 0),
            Stamp::Valid(_)
        ));
    }
}

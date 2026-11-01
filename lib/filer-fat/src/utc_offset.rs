// This is free and unencumbered software released into the public domain.

/// A fixed offset from UTC, east positive, in which a volume's times are
/// read.
///
/// FAT stores local times with no time zone, so the offset of the clock
/// that wrote them has to be supplied; the default is
/// [`UTC`](Self::UTC). It is fixed: like Linux's `time_offset` mount
/// option, it cannot follow daylight saving time, which would need the
/// zone's rules and would make times ambiguous for an hour each year.
///
/// # Examples
///
/// ```
/// use filer_fat::UtcOffset;
///
/// let helsinki = UtcOffset::from_minutes(120).unwrap();
/// assert_eq!(helsinki.seconds(), 7200);
/// assert_eq!(UtcOffset::from_minutes(24 * 60), None);
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UtcOffset(i16);

impl UtcOffset {
    /// Coordinated Universal Time: no offset.
    pub const UTC: Self = Self(0);

    /// The largest offset, in minutes, either way: just under a day.
    pub const MAX_MINUTES: i16 = 24 * 60 - 1;

    /// Returns the offset of `minutes` east of UTC (west if negative), or
    /// `None` if it is a full day or more.
    #[must_use]
    pub const fn from_minutes(minutes: i16) -> Option<Self> {
        if minutes >= -Self::MAX_MINUTES && minutes <= Self::MAX_MINUTES {
            Some(Self(minutes))
        } else {
            None
        }
    }

    /// Returns the offset in minutes, east positive.
    #[must_use]
    pub const fn minutes(self) -> i16 {
        self.0
    }

    /// Returns the offset in seconds, east positive.
    #[must_use]
    pub fn seconds(self) -> i32 {
        i32::from(self.0).saturating_mul(60)
    }
}

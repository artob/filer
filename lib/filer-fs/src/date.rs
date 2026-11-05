// This is free and unencumbered software released into the public domain.

//! Calendar dates, as on-disk formats record them: a year, a month, a
//! day, an hour, a minute, and a second, rather than a count of seconds.
//!
//! # Examples
//!
//! ```
//! use filer_fs::date;
//!
//! assert_eq!(date::unix_seconds(2024, 1, 2, 3, 4, 6), Some(1_704_164_646));
//! assert_eq!(date::unix_seconds(1969, 12, 31, 23, 59, 59), Some(-1));
//! assert_eq!(date::unix_seconds(2024, 13, 1, 0, 0, 0), None);
//! ```

/// Returns the seconds since the Unix epoch of a date and time in the
/// proleptic Gregorian calendar, at Greenwich; or `None` if a field is
/// out of range: a year from 0 to 9999, a month from 1 to 12, a day from
/// 1 to 31 (a day past a month's end runs into the next), an hour below
/// 24, and a minute and a second below 60.
#[must_use]
#[allow(
    clippy::arithmetic_side_effects,
    reason = "the fields are bounded, so the sums are far from overflowing"
)]
pub fn unix_seconds(
    year: i64,
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
    second: u8,
) -> Option<i64> {
    let valid = (0..=9999).contains(&year)
        && (1..=12).contains(&month)
        && (1..=31).contains(&day)
        && hour < 24
        && minute < 60
        && second < 60;
    if !valid {
        return None;
    }
    let days = days_from_civil(year, i64::from(month), i64::from(day));
    let time = i64::from(hour) * 3600 + i64::from(minute) * 60 + i64::from(second);
    Some(days * 86_400 + time)
}

/// Returns the days since 1970-01-01 of a date, counting years from March
/// so that leap days come last.
#[allow(
    clippy::arithmetic_side_effects,
    reason = "years from 0 to 9999, months, and days are far from overflowing"
)]
const fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let of_era = year - era * 400;
    let of_year = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    let of_era_days = of_era * 365 + of_era / 4 - of_era / 100 + of_year;
    era * 146_097 + of_era_days - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_seconds_from_the_epoch() {
        assert_eq!(unix_seconds(1970, 1, 1, 0, 0, 0), Some(0));
        assert_eq!(unix_seconds(2000, 2, 29, 0, 0, 0), Some(951_782_400));
        assert_eq!(unix_seconds(2000, 3, 1, 0, 0, 0), Some(951_868_800));
        assert_eq!(unix_seconds(1900, 3, 1, 0, 0, 0), Some(-2_203_891_200));
        assert_eq!(unix_seconds(0, 1, 1, 0, 0, 0), Some(-62_167_219_200));
        assert_eq!(
            unix_seconds(9999, 12, 31, 23, 59, 59),
            Some(253_402_300_799)
        );
        for invalid in [
            unix_seconds(10_000, 1, 1, 0, 0, 0),
            unix_seconds(-1, 1, 1, 0, 0, 0),
            unix_seconds(2024, 0, 1, 0, 0, 0),
            unix_seconds(2024, 1, 0, 0, 0, 0),
            unix_seconds(2024, 1, 32, 0, 0, 0),
            unix_seconds(2024, 1, 1, 24, 0, 0),
            unix_seconds(2024, 1, 1, 0, 60, 0),
            unix_seconds(2024, 1, 1, 0, 0, 60),
        ] {
            assert_eq!(invalid, None);
        }
    }
}

// This is free and unencumbered software released into the public domain.

//! Dates, in the two forms ISO 9660 records them: seven bytes in a
//! directory record, and seventeen digits and a byte in a volume
//! descriptor, or in a Rock Ridge `TF` entry's long form. Each ends with
//! the offset from Greenwich in quarters of an hour, from -48 to 52, which
//! is subtracted; an offset outside that range is taken as none.

use core::time::Duration;
use filer_api::Timestamp;

/// Returns the time a directory record's seven bytes give: the year since
/// 1900, the month, the day, the hour, the minute, the second, and the
/// offset; or `None` for all zeros, which records no time, or fields out
/// of range.
pub(crate) fn short(bytes: &[u8]) -> Option<Timestamp> {
    let &[year, month, day, hour, minute, second, offset] = bytes else {
        return None;
    };
    let secs = seconds(
        i64::from(year).saturating_add(1900),
        [month, day, hour, minute, second],
        offset,
    )?;
    Some(Timestamp::new(secs, 0).with_granularity(Duration::from_secs(1)))
}

/// Returns the time seventeen bytes give: sixteen ASCII digits, of the
/// year, month, day, hour, minute, second, and hundredths of a second,
/// then the offset; or `None` for all zeros, which records no time, or
/// fields out of range.
pub(crate) fn long(bytes: &[u8]) -> Option<Timestamp> {
    let (digits, rest) = bytes.split_first_chunk::<16>()?;
    let &[offset] = rest else {
        return None;
    };
    let number = |from: usize, to: usize| -> Option<u32> {
        digits.get(from..to)?.iter().try_fold(0_u32, |n, &digit| {
            let digit = char::from(digit).to_digit(10)?;
            n.checked_mul(10)?.checked_add(digit)
        })
    };
    let year = number(0, 4)?;
    let field =
        |from: usize| number(from, from.saturating_add(2)).and_then(|n| u8::try_from(n).ok());
    let fields = [field(4)?, field(6)?, field(8)?, field(10)?, field(12)?];
    let hundredths = number(14, 16)?;
    if year == 0 && fields == [0; 5] {
        return None;
    }
    let secs = seconds(i64::from(year), fields, offset)?;
    let nanos = hundredths.checked_mul(10_000_000)?;
    Some(Timestamp::new(secs, nanos).with_granularity(Duration::from_millis(10)))
}

/// Returns the seconds since the Unix epoch of `year` and `[month, day,
/// hour, minute, second]` at `offset`, or `None` if a field is out of
/// range.
#[allow(
    clippy::arithmetic_side_effects,
    reason = "the fields are bounded, so the sums are far from overflowing"
)]
fn seconds(year: i64, [month, day, hour, minute, second]: [u8; 5], offset: u8) -> Option<i64> {
    let valid = (1..=12).contains(&month)
        && (1..=31).contains(&day)
        && hour < 24
        && minute < 60
        && second < 60;
    if !valid || !(0..=9999).contains(&year) {
        return None;
    }
    let quarters = i64::from(i8::from_le_bytes([offset]));
    let offset = if (-48..=52).contains(&quarters) {
        quarters * 900
    } else {
        0
    };
    let days = days_from_civil(year, i64::from(month), i64::from(day));
    let time = i64::from(hour) * 3600 + i64::from(minute) * 60 + i64::from(second);
    Some(days * 86_400 + time - offset)
}

/// Returns the days since 1970-01-01 of a date in the proleptic Gregorian
/// calendar, counting years from March so that leap days come last.
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
    fn reads_short_dates() {
        // 2024-01-02T03:04:06, at Greenwich and an hour east of it.
        assert_eq!(
            short(&[124, 1, 2, 3, 4, 6, 0]).unwrap().secs(),
            1_704_164_646
        );
        assert_eq!(
            short(&[124, 1, 2, 4, 4, 6, 4]).unwrap().secs(),
            1_704_164_646
        );
        assert_eq!(short(&[70, 1, 1, 0, 0, 0, 0]).unwrap().secs(), 0);
        // Before the epoch, a leap day, and an offset out of range.
        assert_eq!(short(&[69, 12, 31, 23, 59, 59, 0]).unwrap().secs(), -1);
        assert_eq!(
            short(&[100, 2, 29, 0, 0, 0, 0]).unwrap().secs(),
            951_782_400
        );
        assert_eq!(short(&[70, 1, 1, 0, 0, 0, 100]).unwrap().secs(), 0);
        assert_eq!(short(&[0; 7]), None);
        assert_eq!(short(&[124, 13, 2, 3, 4, 6, 0]), None);
        assert_eq!(short(&[124, 1, 2, 3, 4]), None);
    }

    #[test]
    fn reads_long_dates() {
        let date = long(b"2024010203040650\xF8").unwrap();
        assert_eq!(
            (date.secs(), date.nanos()),
            (1_704_164_646 + 7200, 500_000_000)
        );
        assert_eq!(long(b"0000000000000000\0"), None);
        assert_eq!(long(b"2024130203040650\0"), None);
        assert_eq!(long(b"2024x10203040650\0"), None);
        assert_eq!(long(b"20240102030406"), None);
    }
}

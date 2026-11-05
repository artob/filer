// This is free and unencumbered software released into the public domain.

//! Names and times, as UDF records them: names in OSTA's compressed
//! Unicode, a byte saying whether 8 or 16 bits encode each character;
//! times in twelve bytes, of a type and a time zone, the date and time,
//! and hundredths, hundreds of microseconds, and microseconds.

use alloc::vec::Vec;
use core::time::Duration;
use filer_api::Timestamp;
use filer_fs::le;

/// Returns a name in OSTA's compressed Unicode as UTF-8, or WTF-8 for
/// 16-bit characters that are unpaired surrogates; `None` if it is empty
/// or of no known compression.
pub(crate) fn name(bytes: &[u8]) -> Option<Vec<u8>> {
    let (&compression, rest) = bytes.split_first()?;
    let name = match compression {
        // Each byte a character from U+0000 to U+00FF.
        8 => {
            let mut name = Vec::with_capacity(rest.len());
            for &byte in rest {
                let mut utf8 = [0; 2];
                name.extend_from_slice(char::from(byte).encode_utf8(&mut utf8).as_bytes());
            }
            name
        },
        // UTF-16, big-endian.
        16 => {
            let (units, odd) = rest.as_chunks::<2>();
            if !odd.is_empty() {
                return None;
            }
            let units: Vec<u16> = units.iter().map(|&pair| u16::from_be_bytes(pair)).collect();
            filer_msft::wtf8::encode(&units)
        },
        _ => return None,
    };
    (!name.is_empty()).then_some(name)
}

/// Returns the time of a timestamp's twelve bytes, or `None` for one
/// recording no time, or fields out of range. A local time is at the
/// offset it gives, in minutes, unless that is -2047, for none.
pub(crate) fn time(bytes: &[u8]) -> Option<Timestamp> {
    let &[
        _,
        _,
        _,
        _,
        month,
        day,
        hour,
        minute,
        second,
        centis,
        hundreds,
        micros,
    ] = bytes
    else {
        return None;
    };
    let kind_and_zone = le::u16_at(bytes, 0)?;
    let year = i16::from_le_bytes([*bytes.get(2)?, *bytes.get(3)?]);
    if centis > 99 || hundreds > 99 || micros > 99 {
        return None;
    }
    let secs = filer_fs::date::unix_seconds(i64::from(year), month, day, hour, minute, second)?;
    // A 12-bit offset, signed, of a local time.
    let zone = i16::from_le_bytes((kind_and_zone << 4).to_le_bytes()) >> 4;
    let offset = if kind_and_zone >> 12 == 1 && zone != -2047 && (-1440..=1440).contains(&zone) {
        i64::from(zone).saturating_mul(60)
    } else {
        0
    };
    let micros = u32::from(centis)
        .saturating_mul(10_000)
        .saturating_add(u32::from(hundreds).saturating_mul(100))
        .saturating_add(u32::from(micros));
    Some(
        Timestamp::new(secs.saturating_sub(offset), micros.saturating_mul(1000))
            .with_granularity(Duration::from_micros(1)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_names() {
        assert_eq!(name(b"\x08caf\xE9").unwrap(), "café".as_bytes());
        assert_eq!(name(b"\x10\0a\xD8\x3D\xDE\x00").unwrap(), "a😀".as_bytes());
        // An unpaired surrogate, as WTF-8.
        assert_eq!(name(b"\x10\xD8\x00").unwrap(), b"\xED\xA0\x80");
        assert_eq!(name(b"\x10\0"), None);
        assert_eq!(name(b"\x08"), None);
        assert_eq!(name(b"\x09ab"), None);
        assert_eq!(name(b""), None);
    }

    #[test]
    fn reads_times() {
        // 2024-01-02T03:04:06.123456, at Greenwich and an hour east.
        let at = |kind_and_zone: u16, hour: u8| {
            let mut bytes = kind_and_zone.to_le_bytes().to_vec();
            bytes.extend(2024_i16.to_le_bytes());
            bytes.extend([1, 2, hour, 4, 6, 12, 34, 56]);
            time(&bytes)
        };
        let utc = at(0x1000, 3).unwrap();
        assert_eq!((utc.secs(), utc.nanos()), (1_704_164_646, 123_456_000));
        assert_eq!(at(0x1000 | 60, 4).unwrap().secs(), 1_704_164_646);
        // West, and no offset.
        assert_eq!(at(0x1000 | 0xFC4, 2).unwrap().secs(), 1_704_164_646);
        assert_eq!(at(0x1000 | 0x801, 3).unwrap().secs(), 1_704_164_646);
        assert_eq!(time(&[0; 12]), None);
        assert_eq!(time(&[0; 11]), None);
    }
}

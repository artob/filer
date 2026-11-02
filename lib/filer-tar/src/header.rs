// This is free and unencumbered software released into the public domain.

//! Parsing of 512-byte TAR headers and PAX records: pure functions, kept
//! apart from I/O so that they can be fuzzed.

use alloc::vec::Vec;

/// The size of a header and of the unit member data is padded to.
pub(crate) const BLOCK: usize = 512;

/// [`BLOCK`], as offsets in the archive count it.
pub(crate) const BLOCK_LEN: u64 = 512;

/// One 512-byte header block.
pub(crate) struct Header<'a>(pub(crate) &'a [u8; BLOCK]);

impl Header<'_> {
    /// Returns the bytes of the field at `range`, which is within a block.
    fn field(&self, range: core::ops::Range<usize>) -> &[u8] {
        self.0.get(range).unwrap_or_default()
    }

    /// Returns `true` for an all-zero block, which ends the archive.
    pub(crate) fn is_zero(&self) -> bool {
        self.0.iter().all(|&b| b == 0)
    }

    /// Checks the header checksum, accepting the unsigned sum POSIX
    /// specifies and the signed sum some old tools wrote.
    pub(crate) fn checksum_ok(&self) -> bool {
        let Some(stored) = number(self.field(148..156)) else {
            return false;
        };
        // At most 512 bytes of 255: far within an `i64`.
        let (mut unsigned, mut signed) = (0_i64, 0_i64);
        for (i, &b) in self.0.iter().enumerate() {
            let b = if (148..156).contains(&i) { b' ' } else { b };
            unsigned = unsigned.saturating_add(i64::from(b));
            signed = signed.saturating_add(i64::from(i8::from_le_bytes([b])));
        }
        i64::try_from(stored).is_ok_and(|stored| stored == unsigned || stored == signed)
    }

    pub(crate) fn typeflag(&self) -> u8 {
        self.0[156]
    }

    pub(crate) fn size(&self) -> Option<u64> {
        number(self.field(124..136))
    }

    pub(crate) fn mtime(&self) -> Option<u64> {
        number(self.field(136..148))
    }

    /// Returns the member name, joining the POSIX ustar prefix if present.
    pub(crate) fn name(&self) -> Vec<u8> {
        let name = text(self.field(0..100));
        let prefix = text(self.field(345..500));
        // POSIX ustar has a prefix field; GNU tar uses those bytes for other
        // purposes and marks itself with "ustar  ".
        if self.field(257..263) == b"ustar\0" && !prefix.is_empty() {
            let len = prefix.len().saturating_add(name.len()).saturating_add(1);
            let mut joined = Vec::with_capacity(len);
            joined.extend_from_slice(prefix);
            joined.push(b'/');
            joined.extend_from_slice(name);
            joined
        } else {
            name.to_vec()
        }
    }

    pub(crate) fn linkname(&self) -> &[u8] {
        text(self.field(157..257))
    }
}

/// Returns a field up to its first NUL.
pub(crate) fn text(field: &[u8]) -> &[u8] {
    let end = field.iter().position(|&b| b == 0).unwrap_or(field.len());
    field.get(..end).unwrap_or(field)
}

/// Parses a numeric field: octal, padded with spaces or NULs, or GNU
/// base-256 (high bit of the first byte set) for large values.
pub(crate) fn number(field: &[u8]) -> Option<u64> {
    if let Some((&first, rest)) = field.split_first()
        && first & 0x80 != 0
    {
        // Base-256; 0xFF marks a negative number, which no size or time
        // here should be.
        if first == 0xFF {
            return None;
        }
        let mut n = u64::from(first & 0x7F);
        for &b in rest {
            n = n.checked_mul(256)?.checked_add(u64::from(b))?;
        }
        return Some(n);
    }
    let digits = field
        .iter()
        .skip_while(|&&b| b == b' ')
        .take_while(|&&b| b != b' ' && b != 0);
    let mut n: u64 = 0;
    for &b in digits {
        if !(b'0'..=b'7').contains(&b) {
            return None;
        }
        n = n
            .checked_mul(8)?
            .checked_add(u64::from(b.wrapping_sub(b'0')))?;
    }
    Some(n)
}

/// Parses PAX extended header records, `"LEN key=value\n"` repeated, into
/// key and value pairs.
pub(crate) fn pax_records(mut data: &[u8]) -> Option<Vec<(&[u8], &[u8])>> {
    let mut records = Vec::new();
    while !data.is_empty() {
        let space = data.iter().position(|&b| b == b' ')?;
        let len = decimal(data.get(..space)?)?;
        let len = usize::try_from(len).ok()?;
        let start = space.checked_add(1)?;
        if len <= start || len > data.len() {
            return None;
        }
        let record = data.get(start..len)?.strip_suffix(b"\n")?;
        let equals = record.iter().position(|&b| b == b'=')?;
        records.push((record.get(..equals)?, record.get(equals.checked_add(1)?..)?));
        data = data.get(len..)?;
    }
    Some(records)
}

/// Parses a non-negative decimal number.
pub(crate) fn decimal(digits: &[u8]) -> Option<u64> {
    if digits.is_empty() {
        return None;
    }
    let mut n: u64 = 0;
    for &b in digits {
        if !b.is_ascii_digit() {
            return None;
        }
        n = n
            .checked_mul(10)?
            .checked_add(u64::from(b.wrapping_sub(b'0')))?;
    }
    Some(n)
}

/// Parses a PAX time, `SECONDS[.FRACTION]`, keeping whole seconds and
/// nanoseconds.
pub(crate) fn pax_time(value: &[u8]) -> Option<(i64, u32)> {
    let (negative, value) = match value.strip_prefix(b"-") {
        Some(rest) => (true, rest),
        None => (false, value),
    };
    let (secs, fraction) = match value.iter().position(|&b| b == b'.') {
        Some(dot) => (value.get(..dot)?, value.get(dot.checked_add(1)?..)?),
        None => (value, &b""[..]),
    };
    let secs = i64::try_from(decimal(secs)?).ok()?;
    let mut nanos: u32 = 0;
    // The place of the next digit, in nanoseconds: tenths first.
    let mut place: u32 = 100_000_000;
    for &b in fraction.iter().take(9) {
        if !b.is_ascii_digit() {
            return None;
        }
        nanos = nanos.checked_add(u32::from(b.wrapping_sub(b'0')).checked_mul(place)?)?;
        place = place.checked_div(10)?;
    }
    if negative {
        // -1.5 is 1.5 seconds before the epoch: -2 seconds plus 0.5.
        if nanos == 0 {
            Some((secs.checked_neg()?, 0))
        } else {
            Some((
                secs.checked_neg()?.checked_sub(1)?,
                1_000_000_000_u32.checked_sub(nanos)?,
            ))
        }
    } else {
        Some((secs, nanos))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_octal_and_base_256() {
        assert_eq!(number(b"0000644\0"), Some(0o644));
        assert_eq!(number(b"   12 \0"), Some(0o12));
        assert_eq!(number(b"\0\0\0"), Some(0));
        assert_eq!(number(b"9"), None);
        assert_eq!(number(&[0x80, 0, 0, 1, 0]), Some(256));
        assert_eq!(number(&[0xFF, 0xFF]), None);
    }

    #[test]
    fn parses_pax_records() {
        let records = pax_records(b"20 path=a/very/long\n13 size=1024\n").unwrap();
        assert_eq!(
            records,
            [(&b"path"[..], &b"a/very/long"[..]), (b"size", b"1024")]
        );
        assert!(pax_records(b"99 path=x\n").is_none());
        assert!(pax_records(b"10 pathx\n").is_none());
    }

    #[test]
    fn parses_pax_times() {
        assert_eq!(pax_time(b"1700000000"), Some((1_700_000_000, 0)));
        assert_eq!(pax_time(b"1.25"), Some((1, 250_000_000)));
        assert_eq!(pax_time(b"-1.5"), Some((-2, 500_000_000)));
        assert_eq!(pax_time(b"x"), None);
    }
}

// This is free and unencumbered software released into the public domain.

//! Names as UTF-16 units and as WTF-8 bytes: UTF-8, with unpaired
//! surrogates encoded as if they were scalar values, so that every name
//! NTFS can hold has a byte form, and back.

use alloc::vec::Vec;

/// Encodes UTF-16 `units` as WTF-8.
pub(crate) fn encode(units: &[u16]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(units.len());
    for unit in char::decode_utf16(units.iter().copied()) {
        match unit {
            Ok(c) => bytes.extend_from_slice(c.encode_utf8(&mut [0; 4]).as_bytes()),
            Err(error) => {
                // Surrogates are 0xD800 to 0xDFFF: three bytes.
                let [high, low] = error.unpaired_surrogate().to_be_bytes();
                bytes.extend_from_slice(&[
                    0xE0 | (high >> 4),
                    0x80 | ((high & 0x0F) << 2) | (low >> 6),
                    0x80 | (low & 0x3F),
                ]);
            },
        }
    }
    bytes
}

/// Decodes WTF-8 `bytes` into UTF-16 units, or returns `None` if they are
/// not WTF-8 (such as raw bytes from another driver).
pub(crate) fn decode(bytes: &[u8]) -> Option<Vec<u16>> {
    let mut units = Vec::with_capacity(bytes.len());
    let mut rest = bytes;
    while !rest.is_empty() {
        match core::str::from_utf8(rest) {
            Ok(text) => {
                units.extend(text.encode_utf16());
                break;
            },
            Err(error) => {
                let (valid, invalid) = rest.split_at(error.valid_up_to());
                units.extend(core::str::from_utf8(valid).ok()?.encode_utf16());
                // Only a three-byte surrogate may follow.
                let &[a, b, c, ..] = invalid else {
                    return None;
                };
                if a != 0xED || !(0xA0..=0xBF).contains(&b) || c & 0xC0 != 0x80 {
                    return None;
                }
                let unit =
                    (u16::from(a & 0x0F) << 12) | (u16::from(b & 0x3F) << 6) | u16::from(c & 0x3F);
                units.push(unit);
                rest = invalid.get(3..)?;
            },
        }
    }
    Some(units)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_names_with_unpaired_surrogates() {
        let units = [0x61, 0xDC00, 0xD800, 0xD83D, 0xDE00, 0x62];
        let bytes = encode(&units);
        assert_eq!(
            bytes,
            [
                b'a', 0xED, 0xB0, 0x80, 0xED, 0xA0, 0x80, 0xF0, 0x9F, 0x98, 0x80, b'b'
            ]
        );
        assert_eq!(decode(&bytes).unwrap(), units);
        assert_eq!(
            decode("Ünïcödé ✓".as_bytes()).unwrap(),
            "Ünïcödé ✓".encode_utf16().collect::<Vec<_>>()
        );
    }

    #[test]
    fn rejects_bytes_that_are_not_wtf8() {
        for bytes in [&b"caf\xE9"[..], b"\xED\xA0", b"\xED\xA0\x41", b"\xFF"] {
            assert_eq!(decode(bytes), None, "{bytes:02X?}");
        }
    }
}

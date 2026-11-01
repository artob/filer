// This is free and unencumbered software released into the public domain.

/// Lossy rendering of bytes that may not be UTF-8: valid text as is, each
/// invalid byte as `\xNN`. Not round-trippable, since a literal `\x41` in
/// the text renders the same as an escaped byte.
pub(crate) struct Escaped<'a>(pub(crate) &'a [u8]);

impl core::fmt::Display for Escaped<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for chunk in self.0.utf8_chunks() {
            f.write_str(chunk.valid())?;
            for byte in chunk.invalid() {
                write!(f, "\\x{byte:02X}")?;
            }
        }
        Ok(())
    }
}

impl core::fmt::Debug for Escaped<'_> {
    /// Like `str`'s `Debug`, quoted and escaped, with `\xNN` for invalid
    /// bytes.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("\"")?;
        for chunk in self.0.utf8_chunks() {
            for c in chunk.valid().chars() {
                write!(f, "{}", c.escape_debug())?;
            }
            for byte in chunk.invalid() {
                write!(f, "\\x{byte:02X}")?;
            }
        }
        f.write_str("\"")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::format;

    #[test]
    fn escapes_invalid_bytes_only() {
        let bytes = b"caf\xC3\xA9 \xFF\xFE\"x\"";
        assert_eq!(format!("{}", Escaped(bytes)), "café \\xFF\\xFE\"x\"");
        assert_eq!(
            format!("{:?}", Escaped(bytes)),
            "\"café \\xFF\\xFE\\\"x\\\"\""
        );
    }

    #[test]
    fn escapes_wtf8_surrogates_bytewise() {
        // An unpaired surrogate (U+D800) in WTF-8.
        assert_eq!(format!("{}", Escaped(b"a\xED\xA0\x80")), "a\\xED\\xA0\\x80");
    }
}

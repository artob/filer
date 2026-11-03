// This is free and unencumbered software released into the public domain.

//! ar member headers: 60 bytes of space-padded ASCII fields, each followed
//! by its member's data and, after an odd length, a byte of padding.
//!
//! | Offset | Length | Field                              |
//! | ------ | ------ | ---------------------------------- |
//! | 0      | 16     | name                               |
//! | 16     | 12     | modification time, decimal seconds |
//! | 28     | 6      | owner id, decimal                  |
//! | 34     | 6      | group id, decimal                  |
//! | 40     | 8      | mode, octal                        |
//! | 48     | 10     | size, decimal                      |
//! | 58     | 2      | `` `\n ``                          |
//!
//! Names come in three dialects. System V and Debian write a short name,
//! padded with spaces; GNU ends a short name with `/` and keeps longer
//! ones in a table, the member `//`, which `/` and an offset name; BSD
//! writes `#1/` and a length, and the name begins the data. GNU and
//! Windows name their symbol tables `/` (or `/SYM64/`, and Windows other
//! names in `/<`...`>/`), BSD `__.SYMDEF` and its variants.

/// The magic number that begins every archive.
pub(crate) const MAGIC: &[u8; 8] = b"!<arch>\n";

/// The magic number of a GNU thin archive, whose members' data lie in
/// files outside it.
pub(crate) const THIN_MAGIC: &[u8; 8] = b"!<thin>\n";

/// The size of a member header.
pub(crate) const HEADER: usize = 60;
pub(crate) const HEADER_LEN: u64 = 60;

/// A member header.
pub(crate) struct Header<'a>(pub(crate) &'a [u8; HEADER]);

impl Header<'_> {
    fn field(&self, at: usize, len: usize) -> &[u8] {
        self.0
            .get(at..)
            .and_then(|rest| rest.get(..len))
            .unwrap_or(&[])
    }

    /// Returns whether the header ends as every header must.
    pub(crate) fn is_valid(&self) -> bool {
        self.field(58, 2) == b"`\n"
    }

    /// Returns the name field, without its padding.
    pub(crate) fn raw_name(&self) -> &[u8] {
        trim_end(self.field(0, 16), b' ')
    }

    /// Returns the modification time, in seconds since the Unix epoch:
    /// `Some(None)` if the field is blank, as for some special members,
    /// and `None` if it is not a number.
    pub(crate) fn mtime(&self) -> Option<Option<u64>> {
        optional(self.field(16, 12))
    }

    /// Returns the size of the member's data, or `None` if it is not a
    /// number.
    pub(crate) fn size(&self) -> Option<u64> {
        decimal(self.field(48, 10))
    }
}

/// What a member's name field says the member is.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Name<'a> {
    /// A name stored in the field.
    Short(&'a [u8]),
    /// A GNU or Windows long name, at this offset in the name table.
    Long(u64),
    /// A BSD long name, of this length, at the start of the data.
    Bsd(u64),
    /// The GNU or Windows table of long names.
    Table,
    /// A symbol table or another member for the linker, not a file.
    Special,
    /// A malformed name.
    Bad,
}

/// Classifies a name field, already trimmed of its padding.
pub(crate) fn classify(name: &[u8]) -> Name<'_> {
    match name {
        b"//" => Name::Table,
        [b'/', digits @ ..] if !digits.is_empty() && digits.iter().all(u8::is_ascii_digit) => {
            decimal(digits).map_or(Name::Bad, Name::Long)
        },
        // `/`, `/SYM64/`, `/<ECSYMBOLS>/`, and the like.
        [b'/', ..] => Name::Special,
        [b'#', b'1', b'/', len @ ..] => decimal(len).map_or(Name::Bad, Name::Bsd),
        _ if is_bsd_symbol_table(name) => Name::Special,
        // GNU ends a short name with `/`.
        [name @ .., b'/'] => Name::Short(name),
        _ => Name::Short(name),
    }
}

/// Returns whether `name` is one BSD gives its symbol tables.
pub(crate) fn is_bsd_symbol_table(name: &[u8]) -> bool {
    matches!(
        name,
        b"__.SYMDEF" | b"__.SYMDEF SORTED" | b"__.SYMDEF_64" | b"__.SYMDEF_64 SORTED"
    )
}

/// Returns the long name at `offset` in a GNU or Windows name table:
/// up to `/\n` (GNU) or NUL (Windows), or the table's end.
pub(crate) fn long_name(table: &[u8], offset: u64) -> Option<&[u8]> {
    let rest = table.get(usize::try_from(offset).ok()?..)?;
    let end = rest
        .iter()
        .position(|&b| b == b'\n' || b == 0)
        .unwrap_or(rest.len());
    let name = rest.get(..end)?;
    Some(name.strip_suffix(b"/").unwrap_or(name))
}

/// Returns a BSD long name without the NULs that pad it.
pub(crate) fn bsd_name(data: &[u8]) -> &[u8] {
    trim_end(data, 0)
}

fn trim_end(bytes: &[u8], pad: u8) -> &[u8] {
    let end = bytes
        .iter()
        .rposition(|&b| b != pad)
        .map_or(0, |at| at.saturating_add(1));
    bytes.get(..end).unwrap_or(&[])
}

/// Parses a decimal field, padded with spaces after the digits.
pub(crate) fn decimal(field: &[u8]) -> Option<u64> {
    let digits = trim_end(field, b' ');
    if digits.is_empty() {
        return None;
    }
    digits.iter().try_fold(0_u64, |n, &b| {
        let digit = b.checked_sub(b'0').filter(|&d| d < 10)?;
        n.checked_mul(10)?.checked_add(u64::from(digit))
    })
}

/// Parses a decimal field that may be blank.
fn optional(field: &[u8]) -> Option<Option<u64>> {
    if field.iter().all(|&b| b == b' ') {
        Some(None)
    } else {
        decimal(field).map(Some)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fields() {
        let mut header = [b' '; HEADER];
        header[..8].copy_from_slice(b"hello.o/");
        header[16..26].copy_from_slice(b"1700000000");
        header[48..52].copy_from_slice(b"1208");
        header[58..].copy_from_slice(b"`\n");
        let header = Header(&header);
        assert!(header.is_valid());
        assert_eq!(header.raw_name(), b"hello.o/");
        assert_eq!(header.mtime(), Some(Some(1_700_000_000)));
        assert_eq!(header.size(), Some(1208));
        assert_eq!(decimal(b"12 4"), None);
        assert_eq!(decimal(b"99999999999999999999"), None);
        assert_eq!(optional(b"    "), Some(None));
        assert_eq!(optional(b"-1  "), None);
    }

    #[test]
    fn classifies_names() {
        assert_eq!(classify(b"debian-binary"), Name::Short(b"debian-binary"));
        assert_eq!(classify(b"hello.o/"), Name::Short(b"hello.o"));
        assert_eq!(classify(b"/"), Name::Special);
        assert_eq!(classify(b"/SYM64/"), Name::Special);
        assert_eq!(classify(b"/<ECSYMBOLS>/"), Name::Special);
        assert_eq!(classify(b"//"), Name::Table);
        assert_eq!(classify(b"/42"), Name::Long(42));
        assert_eq!(classify(b"#1/20"), Name::Bsd(20));
        assert_eq!(classify(b"#1/x"), Name::Bad);
        assert_eq!(classify(b"__.SYMDEF SORTED"), Name::Special);
        assert_eq!(classify(b""), Name::Short(b""));
    }

    #[test]
    fn reads_long_names() {
        let gnu = b"a_long_name.o/\nother.o/\n";
        assert_eq!(long_name(gnu, 0), Some(&b"a_long_name.o"[..]));
        assert_eq!(long_name(gnu, 15), Some(&b"other.o"[..]));
        let windows = b"a_long_name.o\0other.o\0";
        assert_eq!(long_name(windows, 14), Some(&b"other.o"[..]));
        assert_eq!(long_name(gnu, 100), None);
        assert_eq!(bsd_name(b"__.SYMDEF SORTED\0\0\0\0"), b"__.SYMDEF SORTED");
    }
}

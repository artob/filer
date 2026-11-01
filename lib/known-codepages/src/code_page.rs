// This is free and unencumbered software released into the public domain.

use crate::tables;

/// A single-byte code page: a character set of DOS and Windows consoles.
///
/// Formats from that era store names in the code page of the system that
/// wrote them, and record not which one: FAT short names, and ZIP entry
/// names without the UTF-8 flag. Code page 437, the original IBM PC
/// character set, is their nominal default, and is always available; each
/// other page is behind a feature of its number, such as `cp850`, and the
/// `all` feature, enabled by default, enables them all.
///
/// The pages are the single-byte ones that the Linux kernel supports for
/// FAT (`codepage=`), plus 720 (Arabic) and 858 (850 with the euro sign).
/// Decoding is by table, and total on every page but 857, 864, 869, and
/// 874, which leave a few bytes undefined.
///
/// # Examples
///
/// ```
/// use known_codepages::CodePage;
///
/// assert_eq!(CodePage::default(), CodePage::Cp437);
/// assert_eq!(CodePage::from_number(437), Some(CodePage::Cp437));
/// assert_eq!(CodePage::Cp437.decode(0x82), Some('é'));
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CodePage {
    /// United States: the original IBM PC character set.
    #[default]
    Cp437,
    /// Arabic.
    #[cfg(feature = "cp720")]
    Cp720,
    /// Greek.
    #[cfg(feature = "cp737")]
    Cp737,
    /// Baltic.
    #[cfg(feature = "cp775")]
    Cp775,
    /// Western Europe.
    #[cfg(feature = "cp850")]
    Cp850,
    /// Central Europe.
    #[cfg(feature = "cp852")]
    Cp852,
    /// Cyrillic, IBM.
    #[cfg(feature = "cp855")]
    Cp855,
    /// Turkish.
    #[cfg(feature = "cp857")]
    Cp857,
    /// Western Europe, with the euro sign.
    #[cfg(feature = "cp858")]
    Cp858,
    /// Portuguese.
    #[cfg(feature = "cp860")]
    Cp860,
    /// Icelandic.
    #[cfg(feature = "cp861")]
    Cp861,
    /// Hebrew.
    #[cfg(feature = "cp862")]
    Cp862,
    /// Canadian French.
    #[cfg(feature = "cp863")]
    Cp863,
    /// Arabic, IBM.
    #[cfg(feature = "cp864")]
    Cp864,
    /// Nordic.
    #[cfg(feature = "cp865")]
    Cp865,
    /// Cyrillic, Russian.
    #[cfg(feature = "cp866")]
    Cp866,
    /// Greek, modern.
    #[cfg(feature = "cp869")]
    Cp869,
    /// Thai.
    #[cfg(feature = "cp874")]
    Cp874,
}

impl CodePage {
    /// Every code page available, by number.
    pub const ALL: &[Self] = &[
        Self::Cp437,
        #[cfg(feature = "cp720")]
        Self::Cp720,
        #[cfg(feature = "cp737")]
        Self::Cp737,
        #[cfg(feature = "cp775")]
        Self::Cp775,
        #[cfg(feature = "cp850")]
        Self::Cp850,
        #[cfg(feature = "cp852")]
        Self::Cp852,
        #[cfg(feature = "cp855")]
        Self::Cp855,
        #[cfg(feature = "cp857")]
        Self::Cp857,
        #[cfg(feature = "cp858")]
        Self::Cp858,
        #[cfg(feature = "cp860")]
        Self::Cp860,
        #[cfg(feature = "cp861")]
        Self::Cp861,
        #[cfg(feature = "cp862")]
        Self::Cp862,
        #[cfg(feature = "cp863")]
        Self::Cp863,
        #[cfg(feature = "cp864")]
        Self::Cp864,
        #[cfg(feature = "cp865")]
        Self::Cp865,
        #[cfg(feature = "cp866")]
        Self::Cp866,
        #[cfg(feature = "cp869")]
        Self::Cp869,
        #[cfg(feature = "cp874")]
        Self::Cp874,
    ];

    /// Returns the code page's number, such as 437.
    #[must_use]
    pub const fn number(self) -> u16 {
        match self {
            Self::Cp437 => 437,
            #[cfg(feature = "cp720")]
            Self::Cp720 => 720,
            #[cfg(feature = "cp737")]
            Self::Cp737 => 737,
            #[cfg(feature = "cp775")]
            Self::Cp775 => 775,
            #[cfg(feature = "cp850")]
            Self::Cp850 => 850,
            #[cfg(feature = "cp852")]
            Self::Cp852 => 852,
            #[cfg(feature = "cp855")]
            Self::Cp855 => 855,
            #[cfg(feature = "cp857")]
            Self::Cp857 => 857,
            #[cfg(feature = "cp858")]
            Self::Cp858 => 858,
            #[cfg(feature = "cp860")]
            Self::Cp860 => 860,
            #[cfg(feature = "cp861")]
            Self::Cp861 => 861,
            #[cfg(feature = "cp862")]
            Self::Cp862 => 862,
            #[cfg(feature = "cp863")]
            Self::Cp863 => 863,
            #[cfg(feature = "cp864")]
            Self::Cp864 => 864,
            #[cfg(feature = "cp865")]
            Self::Cp865 => 865,
            #[cfg(feature = "cp866")]
            Self::Cp866 => 866,
            #[cfg(feature = "cp869")]
            Self::Cp869 => 869,
            #[cfg(feature = "cp874")]
            Self::Cp874 => 874,
        }
    }

    /// Returns the code page numbered `number`, or `None` if it is not
    /// available: unknown, or its feature is not enabled.
    #[must_use]
    pub fn from_number(number: u16) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|page| page.number() == number)
    }

    /// Returns the character for `byte`, or `None` if the code page leaves
    /// it undefined.
    #[must_use]
    pub fn decode(self, byte: u8) -> Option<char> {
        self.table().get(usize::from(byte)).copied().flatten()
    }

    fn table(self) -> &'static [Option<char>; 256] {
        match self {
            Self::Cp437 => &tables::CP437,
            #[cfg(feature = "cp720")]
            Self::Cp720 => &tables::CP720,
            #[cfg(feature = "cp737")]
            Self::Cp737 => &tables::CP737,
            #[cfg(feature = "cp775")]
            Self::Cp775 => &tables::CP775,
            #[cfg(feature = "cp850")]
            Self::Cp850 => &tables::CP850,
            #[cfg(feature = "cp852")]
            Self::Cp852 => &tables::CP852,
            #[cfg(feature = "cp855")]
            Self::Cp855 => &tables::CP855,
            #[cfg(feature = "cp857")]
            Self::Cp857 => &tables::CP857,
            #[cfg(feature = "cp858")]
            Self::Cp858 => &tables::CP858,
            #[cfg(feature = "cp860")]
            Self::Cp860 => &tables::CP860,
            #[cfg(feature = "cp861")]
            Self::Cp861 => &tables::CP861,
            #[cfg(feature = "cp862")]
            Self::Cp862 => &tables::CP862,
            #[cfg(feature = "cp863")]
            Self::Cp863 => &tables::CP863,
            #[cfg(feature = "cp864")]
            Self::Cp864 => &tables::CP864,
            #[cfg(feature = "cp865")]
            Self::Cp865 => &tables::CP865,
            #[cfg(feature = "cp866")]
            Self::Cp866 => &tables::CP866,
            #[cfg(feature = "cp869")]
            Self::Cp869 => &tables::CP869,
            #[cfg(feature = "cp874")]
            Self::Cp874 => &tables::CP874,
        }
    }
}

impl core::fmt::Display for CodePage {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "code page {}", self.number())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_code_page_437() {
        let page = CodePage::Cp437;
        assert_eq!(page.decode(b'A'), Some('A'));
        assert_eq!(page.decode(0x7F), Some('\u{7F}'));
        assert_eq!(page.decode(0x80), Some('Ç'));
        assert_eq!(page.decode(0xE1), Some('ß'));
        assert_eq!(page.decode(0xE5), Some('σ'));
        assert_eq!(page.decode(0xFF), Some('\u{A0}'));
    }

    #[test]
    fn numbers_every_page_uniquely() {
        for &page in CodePage::ALL {
            assert_eq!(CodePage::from_number(page.number()), Some(page));
        }
        assert_eq!(CodePage::from_number(65001), None);
    }

    #[cfg(feature = "all")]
    #[test]
    fn decodes_other_code_pages() {
        assert_eq!(CodePage::Cp850.decode(0x9B), Some('ø'));
        assert_eq!(CodePage::Cp852.decode(0xA5), Some('ą'));
        assert_eq!(CodePage::Cp866.decode(0x80), Some('А'));
        assert_eq!(CodePage::Cp858.decode(0xD5), Some('€'));
        // The Arabic percent sign replaces ASCII's.
        assert_eq!(CodePage::Cp864.decode(b'%'), Some('٪'));
        // Undefined bytes.
        assert_eq!(CodePage::Cp857.decode(0xD5), None);
        assert_eq!(CodePage::Cp874.decode(0xDB), None);
    }
}

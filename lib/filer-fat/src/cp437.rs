// This is free and unencumbered software released into the public domain.

//! Code page 437, the original IBM PC character set, in which short names
//! are decoded.
//!
//! Short names may hold bytes from the OEM code page of the system that
//! wrote them, and the volume does not record which page that was. Code
//! page 437 is the most common, and the default of the Linux kernel's FAT
//! driver (`codepage=437`). Decoding is total, so every short name has a
//! UTF-8 form; a name written under another code page decodes to other
//! characters than its writer saw, but still to the same name every time.

/// Returns the character for `byte`: itself if ASCII, else its code page
/// 437 character.
pub(crate) fn decode(byte: u8) -> char {
    match byte.checked_sub(0x80) {
        None => char::from(byte),
        // `UPPER` has an entry for every byte from 0x80.
        Some(high) => UPPER
            .get(usize::from(high))
            .copied()
            .unwrap_or(char::REPLACEMENT_CHARACTER),
    }
}

/// Characters for bytes `0x80` to `0xFF`.
#[rustfmt::skip]
const UPPER: [char; 128] = [
    '\u{00C7}', '\u{00FC}', '\u{00E9}', '\u{00E2}', '\u{00E4}', '\u{00E0}', '\u{00E5}', '\u{00E7}', // 80
    '\u{00EA}', '\u{00EB}', '\u{00E8}', '\u{00EF}', '\u{00EE}', '\u{00EC}', '\u{00C4}', '\u{00C5}', // 88
    '\u{00C9}', '\u{00E6}', '\u{00C6}', '\u{00F4}', '\u{00F6}', '\u{00F2}', '\u{00FB}', '\u{00F9}', // 90
    '\u{00FF}', '\u{00D6}', '\u{00DC}', '\u{00A2}', '\u{00A3}', '\u{00A5}', '\u{20A7}', '\u{0192}', // 98
    '\u{00E1}', '\u{00ED}', '\u{00F3}', '\u{00FA}', '\u{00F1}', '\u{00D1}', '\u{00AA}', '\u{00BA}', // A0
    '\u{00BF}', '\u{2310}', '\u{00AC}', '\u{00BD}', '\u{00BC}', '\u{00A1}', '\u{00AB}', '\u{00BB}', // A8
    '\u{2591}', '\u{2592}', '\u{2593}', '\u{2502}', '\u{2524}', '\u{2561}', '\u{2562}', '\u{2556}', // B0
    '\u{2555}', '\u{2563}', '\u{2551}', '\u{2557}', '\u{255D}', '\u{255C}', '\u{255B}', '\u{2510}', // B8
    '\u{2514}', '\u{2534}', '\u{252C}', '\u{251C}', '\u{2500}', '\u{253C}', '\u{255E}', '\u{255F}', // C0
    '\u{255A}', '\u{2554}', '\u{2569}', '\u{2566}', '\u{2560}', '\u{2550}', '\u{256C}', '\u{2567}', // C8
    '\u{2568}', '\u{2564}', '\u{2565}', '\u{2559}', '\u{2558}', '\u{2552}', '\u{2553}', '\u{256B}', // D0
    '\u{256A}', '\u{2518}', '\u{250C}', '\u{2588}', '\u{2584}', '\u{258C}', '\u{2590}', '\u{2580}', // D8
    '\u{03B1}', '\u{00DF}', '\u{0393}', '\u{03C0}', '\u{03A3}', '\u{03C3}', '\u{00B5}', '\u{03C4}', // E0
    '\u{03A6}', '\u{0398}', '\u{03A9}', '\u{03B4}', '\u{221E}', '\u{03C6}', '\u{03B5}', '\u{2229}', // E8
    '\u{2261}', '\u{00B1}', '\u{2265}', '\u{2264}', '\u{2320}', '\u{2321}', '\u{00F7}', '\u{2248}', // F0
    '\u{00B0}', '\u{2219}', '\u{00B7}', '\u{221A}', '\u{207F}', '\u{00B2}', '\u{25A0}', '\u{00A0}', // F8
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_bytes() {
        assert_eq!(decode(b'A'), 'A');
        assert_eq!(decode(0x7F), '\u{7F}');
        assert_eq!(decode(0x80), 'Ç');
        assert_eq!(decode(0x82), 'é');
        assert_eq!(decode(0xE1), 'ß');
        assert_eq!(decode(0xFF), '\u{A0}');
    }
}

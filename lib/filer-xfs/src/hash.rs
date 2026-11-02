// This is free and unencumbered software released into the public domain.

//! The name hash that orders directory leaves and attribute leaves, and
//! the case folding of `ASCII_CI` directories.
//!
//! The hash takes a name four bytes at a time, each group's bytes shifted
//! by 21, 14, 7, and 0 bits, XORed with the hash so far rotated left by 28;
//! a last group of fewer bytes shifts them less, and rotates by 7 bits for
//! each. `ASCII_CI` directories hash and compare names folded to lower
//! case, which, despite the name, Linux does for Latin-1's letters too:
//! bytes 0x41 to 0x5A and 0xC0 to 0xDE but 0xD7 gain 0x20.

/// Returns the hash of `name`.
pub(crate) fn hash(name: &[u8]) -> u32 {
    let (groups, rest) = name.as_chunks::<4>();
    let hash = groups.iter().fold(0u32, |hash, group| {
        let [a, b, c, d] = group.map(u32::from);
        (a << 21) ^ (b << 14) ^ (c << 7) ^ d ^ hash.rotate_left(28)
    });
    // At most three bytes are left: shifted by 14, 7, and 0 bits, or less.
    let mut shift = 0u32;
    let mut last = 0u32;
    for &byte in rest.iter().rev() {
        last ^= u32::from(byte).wrapping_shl(shift);
        shift = shift.wrapping_add(7);
    }
    hash.rotate_left(shift) ^ last
}

/// Folds `byte` to lower case, as `ASCII_CI` directories do.
pub(crate) const fn fold(byte: u8) -> u8 {
    match byte {
        0x41..=0x5A | 0xC0..=0xD6 | 0xD8..=0xDE => byte.wrapping_add(0x20),
        _ => byte,
    }
}

/// Returns the hash of `name` folded to lower case.
pub(crate) fn hash_folded(name: &[u8]) -> u32 {
    let folded: alloc::vec::Vec<u8> = name.iter().map(|&byte| fold(byte)).collect();
    hash(&folded)
}

/// Returns whether `a` and `b` are the same name folded to lower case.
pub(crate) fn eq_folded(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(&a, &b)| fold(a) == fold(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_as_xfs_db_does() {
        // From `xfs_db`'s `hash` command.
        for (name, expected) in [
            (&b""[..], 0),
            (b"a", 0x61),
            (b"ab", 0x30E2),
            (b"abc", 0x0018_7163),
            (b"abcd", 0x0C38_B1E4),
            (b"abcde", 0x1C58_F263),
            (b"a00", 0x0018_5830),
            (b"f00000", 0x060C_1B03),
            // `xfs_db hash hello world` hashes `hello`.
            (b"hello", 0x8CBB_3669),
        ] {
            assert_eq!(hash(name), expected, "{}", name.escape_ascii());
        }
    }

    #[test]
    fn folds_as_ascii_ci_directories_do() {
        // From `xfs_db`'s `hash -d` on an `ASCII_CI` volume.
        assert_eq!(hash_folded(b"ABC"), 0x0018_7163);
        assert_eq!(hash_folded(b"caf\xC9"), 0x0C78_73E9);
        assert_eq!(hash_folded(b"x\xDEy"), 0x001E_7F79);
        assert_eq!(hash_folded(b"Z[@`"), 0x0F56_E060);
        for (byte, folded) in [
            (0x40, 0x40),
            (0x41, 0x61),
            (0x5A, 0x7A),
            (0x5B, 0x5B),
            (0x61, 0x61),
            (0xBF, 0xBF),
            (0xC0, 0xE0),
            (0xD6, 0xF6),
            (0xD7, 0xD7),
            (0xD8, 0xF8),
            (0xDE, 0xFE),
            (0xDF, 0xDF),
            (0xFF, 0xFF),
        ] {
            assert_eq!(fold(byte), folded, "{byte:#04X}");
        }
        assert!(eq_folded(b"CAF\xC9", b"caf\xE9"));
        assert!(!eq_folded(b"a[", b"a{"));
        assert!(!eq_folded(b"a", b"ab"));
    }
}

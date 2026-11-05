// This is free and unencumbered software released into the public domain.

/// Which characters separate components in a string that
/// [`RelPathBuf::normalize_lexically`](super::RelPathBuf::normalize_lexically)
/// reads.
///
/// [`RelPath`](super::RelPath) itself always uses `/`; this only says how
/// to read a string typed by a user or taken from elsewhere.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Separators {
    /// Only `/`, as on POSIX; a `\` is part of a name.
    #[default]
    Slash,

    /// Both `/` and `\`, as on Windows.
    SlashOrBackslash,
}

impl Separators {
    /// Returns `true` if `c` separates components.
    #[must_use]
    pub const fn contains(self, c: char) -> bool {
        match self {
            Self::Slash => c == '/',
            Self::SlashOrBackslash => c == '/' || c == '\\',
        }
    }
}

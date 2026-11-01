// This is free and unencumbered software released into the public domain.

/// Why a string is not a valid [`Name`](super::Name).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum NameError {
    /// The string is empty.
    Empty,

    /// The string contains a `/`, which separates path components.
    ContainsSlash,

    /// The string contains a NUL character, which no OS accepts in names.
    ContainsNul,

    /// The string is `.` or `..`, which refer to existing directories and
    /// cannot name an entry.
    DotOrDotDot,
}

impl NameError {
    /// Panics with a fixed message for this error. A `const` panic cannot
    /// format its argument, so the `name!` and `rel!` macros use this to
    /// report invalid literals at compile time.
    pub(super) const fn panic(self) -> ! {
        match self {
            Self::Empty => panic!("invalid literal: empty name or component"),
            Self::ContainsSlash => panic!("invalid literal: name contains '/'"),
            Self::ContainsNul => panic!("invalid literal: contains NUL"),
            Self::DotOrDotDot => panic!("invalid literal: '.' or '..'"),
        }
    }
}

impl core::fmt::Display for NameError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::Empty => "name is empty",
            Self::ContainsSlash => "name contains '/'",
            Self::ContainsNul => "name contains NUL",
            Self::DotOrDotDot => "name is '.' or '..'",
        })
    }
}

impl core::error::Error for NameError {}

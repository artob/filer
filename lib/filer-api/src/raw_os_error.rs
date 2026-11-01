// This is free and unencumbered software released into the public domain.

/// An OS error code, preserved as the driver received it.
///
/// The variant records which numbering the code belongs to, so a code
/// recorded on one platform is never misread on another.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum RawOsError {
    /// A POSIX or WASI `errno` value.
    Errno(i32),

    /// A Windows system error code, as returned by `GetLastError`.
    Windows(u32),
}

impl core::fmt::Display for RawOsError {
    /// Formats as `os error N`, like `std::io::Error` does.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Errno(code) => write!(f, "os error {code}"),
            Self::Windows(code) => write!(f, "os error {code}"),
        }
    }
}

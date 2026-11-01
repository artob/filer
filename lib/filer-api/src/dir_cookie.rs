// This is free and unencumbered software released into the public domain.

/// A driver's position within a directory listing.
///
/// [`Op::ReadDir`](crate::Op::ReadDir) returns a cookie with each batch;
/// passing it back continues the listing after that batch. Its value is
/// private to the driver that returned it (on POSIX, a `d_off`), and it is
/// only meaningful for the same directory handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DirCookie(u64);

impl DirCookie {
    /// Wraps a driver's position value.
    #[must_use]
    pub const fn new(position: u64) -> Self {
        Self(position)
    }

    /// Returns the driver's position value.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

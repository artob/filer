// This is free and unencumbered software released into the public domain.

mod sealed {
    /// Restricts file modes to this crate.
    pub trait Sealed {}
}

/// The mode a [`File`](crate::File) was opened in, fixed in its type so
/// that, say, writing to a read-only file does not compile.
///
/// The trait is sealed; the modes are uninhabited marker types. Only
/// [`ReadOnly`] exists so far.
pub trait Mode: sealed::Sealed + Send + Sync + 'static {}

/// A [`Mode`] that permits reading.
pub trait Readable: Mode {}

/// The mode of a file opened for reading only.
#[derive(Debug)]
pub enum ReadOnly {}

impl sealed::Sealed for ReadOnly {}
impl Mode for ReadOnly {}
impl Readable for ReadOnly {}

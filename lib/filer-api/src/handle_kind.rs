// This is free and unencumbered software released into the public domain.

/// What a [`Handle`](crate::Handle) refers to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum HandleKind {
    /// An open directory, against which paths are resolved.
    Dir,

    /// An open file.
    File,
}

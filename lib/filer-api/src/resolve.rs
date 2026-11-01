// This is free and unencumbered software released into the public domain.

/// How symlinks met during path resolution are treated.
///
/// Whatever the policy, magic links such as `/proc/self/fd/*` are never
/// followed, and resolution never leaves the subtree beneath the handle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Symlinks {
    /// Refuse to traverse any symlink (Linux `RESOLVE_NO_SYMLINKS`).
    Deny,

    /// Follow symlinks, but fail with
    /// [`EscapeBlocked`](crate::ErrorKind::EscapeBlocked) if resolution
    /// would leave the handle's subtree (Linux `RESOLVE_BENEATH`).
    #[default]
    Beneath,

    /// Follow symlinks, treating absolute targets and `..` at the top as
    /// referring to the handle itself, as in a chroot (Linux
    /// `RESOLVE_IN_ROOT`).
    InRoot,
}

/// The path resolution policy for an operation.
///
/// Each directory carries a default policy; individual operations may
/// tighten it but never loosen it. The default follows symlinks beneath
/// the handle and crosses mount points.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct Resolve {
    /// How symlinks are treated.
    pub symlinks: Symlinks,

    /// Whether resolution may cross mount points (if `false`, Linux
    /// `RESOLVE_NO_XDEV`).
    pub cross_mounts: bool,
}

impl Default for Resolve {
    fn default() -> Self {
        Self {
            symlinks: Symlinks::Beneath,
            cross_mounts: true,
        }
    }
}

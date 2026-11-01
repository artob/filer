// This is free and unencumbered software released into the public domain.

/// Proof that the caller deliberately reaches into the ambient namespace:
/// the process's view of the filesystem through native, absolute paths.
///
/// Everything else in Filer is relative to a directory handle the code was
/// given. Functions that turn a native path into a handle, such as a
/// driver's way of opening a root directory, require this token, so a code
/// review can find every place authority enters a program by searching for
/// `ambient_authority()` (available under `std`), and a library cannot
/// conjure access it was not handed. The function can also be denied with
/// Clippy's `disallowed_methods`.
#[derive(Clone, Copy, Debug)]
pub struct AmbientAuthority(());

/// Returns the token that grants access to the ambient namespace; see
/// [`AmbientAuthority`].
#[cfg(feature = "std")]
#[must_use]
pub fn ambient_authority() -> AmbientAuthority {
    AmbientAuthority(())
}

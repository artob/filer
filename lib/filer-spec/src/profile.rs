// This is free and unencumbered software released into the public domain.

/// What a driver's environment can create, so that the specification skips
/// cases it cannot set up rather than failing them.
///
/// The default assumes everything is possible; turn off what is not.
///
/// # Examples
///
/// ```
/// use filer_spec::Profile;
///
/// // macOS (APFS) rejects names that are not valid UTF-8.
/// let profile = Profile::default().with_foreign_names(false);
/// assert!(profile.symlinks && !profile.foreign_names);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Profile {
    /// Symlinks can be created.
    pub symlinks: bool,

    /// Names that are not valid UTF-8 can be created.
    pub foreign_names: bool,

    /// FIFOs (named pipes) can be created.
    pub fifos: bool,

    /// Extended attributes in the `user` namespace can be created, and the
    /// driver reads them.
    pub xattrs: bool,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            symlinks: true,
            foreign_names: true,
            fifos: true,
            xattrs: true,
        }
    }
}

impl Profile {
    /// Sets whether symlinks can be created.
    #[must_use]
    pub const fn with_symlinks(mut self, symlinks: bool) -> Self {
        self.symlinks = symlinks;
        self
    }

    /// Sets whether names that are not valid UTF-8 can be created.
    #[must_use]
    pub const fn with_foreign_names(mut self, foreign_names: bool) -> Self {
        self.foreign_names = foreign_names;
        self
    }

    /// Sets whether FIFOs can be created.
    #[must_use]
    pub const fn with_fifos(mut self, fifos: bool) -> Self {
        self.fifos = fifos;
        self
    }

    /// Sets whether extended attributes can be created and read.
    #[must_use]
    pub const fn with_xattrs(mut self, xattrs: bool) -> Self {
        self.xattrs = xattrs;
        self
    }
}

// This is free and unencumbered software released into the public domain.

bitflags::bitflags! {
    /// What a handle permits, enforced by the library before an operation
    /// reaches the driver.
    ///
    /// Rights are coarse on purpose: WASI preview 1 gave every descriptor
    /// dozens of rights bits, which were rarely used correctly, and WASI 0.2
    /// replaced them with a handful. They can only be removed, never added:
    /// a directory handle can be restricted, and a handle opened through a
    /// directory gets at most that directory's rights.
    ///
    /// | Operation | Rights |
    /// | --- | --- |
    /// | open a file for reading, list a directory, read metadata or a link | [`READ`](Self::READ) |
    /// | open a file for writing | [`WRITE`](Self::WRITE) |
    /// | create, remove, rename, or link entries | [`MUTATE`](Self::MUTATE) |
    /// | change permissions, times, ownership, attributes | [`META`](Self::META) |
    ///
    /// Violations fail with [`RightsDenied`](crate::ErrorKind::RightsDenied),
    /// distinct from the OS's [`PermissionDenied`](crate::ErrorKind::PermissionDenied).
    /// Rights bound what safe code can do with the handles it holds; they
    /// are not a sandbox against hostile native code in the same process.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct Rights: u8 {
        /// Read file contents; list a directory; read metadata, links, and
        /// extended attributes.
        const READ = 1 << 0;
        /// Write file contents, truncate, allocate, punch holes.
        const WRITE = 1 << 1;
        /// Create, remove, rename, and link entries in a directory.
        const MUTATE = 1 << 2;
        /// Change permissions, times, ownership, attributes, and flags.
        const META = 1 << 3;
    }
}

impl Rights {
    /// Returns the rights a file opened through a directory with these
    /// rights receives: [`MUTATE`](Self::MUTATE) means nothing for files and
    /// is dropped.
    #[must_use]
    pub const fn for_file(self) -> Self {
        self.difference(Self::MUTATE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attenuates_by_intersection() {
        let read_only = Rights::all() & Rights::READ;
        assert_eq!(read_only, Rights::READ);
        assert!(!(read_only & Rights::all()).contains(Rights::WRITE));
    }

    #[test]
    fn drops_mutate_for_files() {
        assert_eq!(
            Rights::all().for_file(),
            Rights::READ | Rights::WRITE | Rights::META
        );
    }
}

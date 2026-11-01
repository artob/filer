// This is free and unencumbered software released into the public domain.

use crate::Profile;
use alloc::{format, string::String, vec, vec::Vec};
use filer_api::{XattrName, path::RelPathBuf};

/// One entry for a driver's setup function to create.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum TreeEntry {
    /// A directory.
    Dir(RelPathBuf),

    /// A regular file with the given contents.
    File(RelPathBuf, Vec<u8>),

    /// A symlink with the given target, stored as is.
    Symlink(RelPathBuf, String),

    /// A FIFO (named pipe), only if [`Profile::fifos`].
    Fifo(RelPathBuf),

    /// An extended attribute of the file or directory at the path, created
    /// before it, only if [`Profile::xattrs`]. Its name is always in the
    /// `user` namespace.
    Xattr(RelPathBuf, XattrName, Vec<u8>),

    /// A regular file whose name is not valid UTF-8, only if
    /// [`Profile::foreign_names`].
    ForeignFile {
        /// The directory to create it in.
        dir: RelPathBuf,
        /// The raw name.
        name: Vec<u8>,
        /// The contents.
        contents: Vec<u8>,
    },
}

/// The tree a driver's setup function creates for the specification.
///
/// Entries come in order, each directory before its contents, so a setup
/// function can create them one by one. The tree includes only what the
/// [`Profile`] allows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tree {
    entries: Vec<TreeEntry>,
}

/// The raw name of the foreign file in the standard tree.
pub(crate) const FOREIGN_NAME: &[u8] = b"caf\xE9";

/// How many files the standard tree's `many` directory holds, enough for
/// several batches of a listing.
pub(crate) const MANY: usize = 600;

impl Tree {
    /// Returns the standard tree for `profile`:
    ///
    /// ```text
    /// a/b/c.txt          "contents"
    ///   user.comment     "a comment" (if xattrs)
    ///   user.empty       ""          (if xattrs)
    /// a/z.txt            ""
    /// a/empty/
    /// a/                 user.tag "dir" (if xattrs)
    /// a/link -> b/c.txt  a/up -> ../a/b/c.txt  a/abs -> /a/b/c.txt
    /// a/escape -> ../../c.txt  a/loop -> loop  a/dangling -> nope
    /// many/f000 .. many/f599
    /// special/fifo       (if FIFOs)
    /// foreign/caf\xE9    "foreign" (if foreign names)
    /// ```
    #[must_use]
    pub fn standard(profile: &Profile) -> Self {
        let path = |s: &str| RelPathBuf::try_from(s).expect("valid tree path");
        let mut entries = vec![
            TreeEntry::Dir(path("a")),
            TreeEntry::Dir(path("a/b")),
            TreeEntry::File(path("a/b/c.txt"), b"contents".to_vec()),
            TreeEntry::File(path("a/z.txt"), Vec::new()),
            TreeEntry::Dir(path("a/empty")),
        ];
        if profile.xattrs {
            let name = |s: &str| XattrName::try_from(s).expect("valid attribute name");
            for (at, xattr, value) in [
                ("a/b/c.txt", "user.comment", &b"a comment"[..]),
                ("a/b/c.txt", "user.empty", b""),
                ("a", "user.tag", b"dir"),
            ] {
                entries.push(TreeEntry::Xattr(path(at), name(xattr), value.to_vec()));
            }
        }
        if profile.symlinks {
            for (link, target) in [
                ("a/link", "b/c.txt"),
                ("a/up", "../a/b/c.txt"),
                ("a/abs", "/a/b/c.txt"),
                ("a/escape", "../../c.txt"),
                ("a/loop", "loop"),
                ("a/dangling", "nope"),
            ] {
                entries.push(TreeEntry::Symlink(path(link), String::from(target)));
            }
        }
        entries.push(TreeEntry::Dir(path("many")));
        for i in 0..MANY {
            entries.push(TreeEntry::File(path(&format!("many/f{i:03}")), Vec::new()));
        }
        if profile.fifos {
            entries.push(TreeEntry::Dir(path("special")));
            entries.push(TreeEntry::Fifo(path("special/fifo")));
        }
        if profile.foreign_names {
            entries.push(TreeEntry::Dir(path("foreign")));
            entries.push(TreeEntry::ForeignFile {
                dir: path("foreign"),
                name: FOREIGN_NAME.to_vec(),
                contents: b"foreign".to_vec(),
            });
        }
        Self { entries }
    }

    /// Returns the entries to create, in order.
    #[must_use]
    pub fn entries(&self) -> &[TreeEntry] {
        &self.entries
    }
}

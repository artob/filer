// This is free and unencumbered software released into the public domain.

use crate::{ArchiveError, Member, Stored};
use alloc::vec::Vec;
use filer_api::{
    Error, ErrorKind, FileKind,
    tree::{Ino, Node, ROOT, Tree},
};

/// The index of an archive under construction, applying the rules every
/// archive format shares.
///
/// `C` is the format's [`Codec`](crate::Codec), [`Stored`] for
/// formats without compression. A format's parser reads its headers in order and records each member
/// here by its raw name, a `/`-separated byte string. The index then:
///
/// - refuses names that could escape the archive root (absolute, or with a
///   `..` component, or containing NUL) with
///   [`InvalidName`](ErrorKind::InvalidName), ignoring empty and `.`
///   components, so `./a//b` is `a/b`;
/// - creates the directories a name implies;
/// - lets the later of duplicate members win, even where a file replaces a
///   directory or the reverse;
/// - makes a hardlink share its target's node, and so its file id, whether
///   the format names the target by path ([`hardlink`](Self::hardlink),
///   as TAR does) or by inode ([`add`](Self::add) and
///   [`alias`](Self::alias), as cpio does).
///
/// Errors name the archive format and the offset of the header at fault
/// (see [`ArchiveError`]).
///
/// # Examples
///
/// ```
/// use filer_api::tree::{Node, ROOT};
/// use filer_pack::{Index, Member};
///
/// let mut index = Index::new("tar");
/// index.insert(b"./docs/README", Node::File(Member::new(512, 14)), 0)?;
/// index.hardlink(b"README", b"docs/README", 1024)?;
/// assert!(index.insert(b"../evil", Node::File(Member::new(0, 0)), 1536).is_err());
/// let tree = index.into_tree();
/// let docs = tree.child(ROOT, b"docs").unwrap();
/// assert_eq!(tree.child(docs, b"README"), tree.child(ROOT, b"README"));
/// # Ok::<(), filer_api::Error>(())
/// ```
#[derive(Clone, Debug)]
pub struct Index<C = Stored> {
    format: &'static str,
    tree: Tree<Member<C>>,
}

impl<C> Index<C> {
    /// Returns an empty index for an archive of `format`, such as `"tar"`,
    /// which errors name.
    #[must_use]
    pub fn new(format: &'static str) -> Self {
        Self {
            format,
            tree: Tree::new(),
        }
    }

    /// Records the directory `name`, from the header at `offset`.
    ///
    /// # Errors
    ///
    /// [`InvalidName`](ErrorKind::InvalidName) if `name` could escape the
    /// archive root.
    pub fn dir(&mut self, name: &[u8], offset: u64) -> Result<(), Error> {
        let components = self.components(name, offset)?;
        self.make_dirs(&components, offset)?;
        Ok(())
    }

    /// Records the non-directory member `name` as `node`, from the header
    /// at `offset`, replacing any earlier member of that name.
    ///
    /// # Errors
    ///
    /// [`InvalidName`](ErrorKind::InvalidName) if `name` could escape the
    /// archive root or names the root itself.
    pub fn insert(&mut self, name: &[u8], node: Node<Member<C>>, offset: u64) -> Result<(), Error> {
        self.add(name, node, offset).map(|_| ())
    }

    /// Records the non-directory member `name` as `node`, as
    /// [`insert`](Self::insert) does, and returns the node's inode, for
    /// [`alias`](Self::alias).
    ///
    /// # Errors
    ///
    /// As for [`insert`](Self::insert).
    pub fn add(&mut self, name: &[u8], node: Node<Member<C>>, offset: u64) -> Result<Ino, Error> {
        let components = self.components(name, offset)?;
        let Some((&last, parents)) = components.split_last() else {
            return Err(self.fail(ErrorKind::InvalidName, offset, "member names the root"));
        };
        let parent = self.make_dirs(parents, offset)?;
        let ino = self.tree.add(node);
        self.link(parent, last, ino, offset)?;
        Ok(ino)
    }

    /// Records `name` as another link to `ino`, a node [`add`](Self::add)
    /// returned, from the header at `offset`: a hardlink whose format
    /// names its target by inode rather than by path, as cpio does, so
    /// that the link holds even if a later member takes the target's
    /// name.
    ///
    /// # Errors
    ///
    /// [`InvalidName`](ErrorKind::InvalidName) for unsafe names;
    /// [`Corrupt`](ErrorKind::Corrupt) if `ino` is not a node of this
    /// index, or is a directory.
    pub fn alias(&mut self, name: &[u8], ino: Ino, offset: u64) -> Result<(), Error> {
        let components = self.components(name, offset)?;
        let Some((&last, parents)) = components.split_last() else {
            return Err(self.fail(ErrorKind::InvalidName, offset, "member names the root"));
        };
        let is_file = self
            .tree
            .node(ino)
            .is_some_and(|node| node.kind() != FileKind::Dir);
        if !is_file {
            return Err(self.fail(ErrorKind::Corrupt, offset, "bad hardlink"));
        }
        let parent = self.make_dirs(parents, offset)?;
        self.link(parent, last, ino, offset)
    }

    /// Records `name` as a hardlink to the earlier member `target`, from
    /// the header at `offset`.
    ///
    /// # Errors
    ///
    /// [`InvalidName`](ErrorKind::InvalidName) for unsafe names;
    /// [`Corrupt`](ErrorKind::Corrupt) if `target` is not an earlier
    /// member, or is a directory.
    pub fn hardlink(&mut self, name: &[u8], target: &[u8], offset: u64) -> Result<(), Error> {
        // The name is checked first, so that an unsafe one is reported as
        // such whatever the target.
        self.components(name, offset)?;
        let bad = || self.fail(ErrorKind::Corrupt, offset, "bad hardlink");
        let target = safe_components(target).ok_or_else(bad)?;
        let target = target
            .iter()
            .try_fold(ROOT, |dir, name| self.tree.child(dir, name))
            .filter(|_| !target.is_empty())
            .ok_or_else(bad)?;
        self.alias(name, target, offset)
    }

    /// Returns the tree built so far.
    #[must_use]
    pub fn tree(&self) -> &Tree<Member<C>> {
        &self.tree
    }

    /// Returns the finished tree, for an [`Archive`](crate::Archive).
    #[must_use]
    pub fn into_tree(self) -> Tree<Member<C>> {
        self.tree
    }

    fn fail(&self, kind: ErrorKind, offset: u64, reason: &'static str) -> Error {
        ArchiveError::new(self.format, offset, reason).into_error(kind)
    }

    fn components<'n>(&self, name: &'n [u8], offset: u64) -> Result<Vec<&'n [u8]>, Error> {
        safe_components(name)
            .ok_or_else(|| self.fail(ErrorKind::InvalidName, offset, "unsafe name"))
    }

    /// Returns the directory at `components`, creating or replacing (the
    /// later member wins) whatever is in the way, for the header at
    /// `offset`.
    fn make_dirs(&mut self, components: &[&[u8]], offset: u64) -> Result<Ino, Error> {
        let mut dir = ROOT;
        for &name in components {
            dir = match self.tree.make_dir(dir, name) {
                Ok(child) => child,
                Err(_) => {
                    let child = self.tree.add(Node::Dir(Default::default()));
                    self.link(dir, name, child, offset)?;
                    child
                },
            };
        }
        Ok(dir)
    }

    /// Links `ino` as `name` in directory `dir`, for the header at
    /// `offset`. `dir` is one, as `make_dirs` returns only directories.
    fn link(&mut self, dir: Ino, name: &[u8], ino: Ino, offset: u64) -> Result<(), Error> {
        self.tree
            .link(dir, name, ino)
            .map(|_| ())
            .map_err(|kind| self.fail(kind, offset, "member's parent is not a directory"))
    }
}

/// Splits a member name into its components, dropping empty and `.` ones,
/// or returns `None` for names that could escape the archive root:
/// absolute paths, `..` components, and names containing NUL.
fn safe_components(name: &[u8]) -> Option<Vec<&[u8]>> {
    if name.starts_with(b"/") || name.contains(&0) {
        return None;
    }
    let mut components = Vec::new();
    for component in name.split(|&b| b == b'/') {
        match component {
            b"" | b"." => {},
            b".." => return None,
            _ => components.push(component),
        }
    }
    Some(components)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    fn file(n: u64) -> Node<Member> {
        Node::File(Member::new(n, 0))
    }

    #[test]
    fn refuses_names_that_could_escape() {
        let mut index = Index::new("test");
        for name in [&b"../a"[..], b"a/../../b", b"/etc/passwd", b"a\0b"] {
            let error = index.insert(name, file(0), 7).unwrap_err();
            assert_eq!(error.kind(), ErrorKind::InvalidName, "{name:?}");
        }
        let error = index.insert(b"./", file(0), 9).unwrap_err();
        let source = core::error::Error::source(&error).unwrap().to_string();
        assert_eq!(source, "test header at offset 9: member names the root");
    }

    #[test]
    fn lets_later_members_win() {
        let mut index = Index::new("test");
        index.insert(b"a", file(1), 0).unwrap();
        index.insert(b"a", file(2), 0).unwrap();
        index.insert(b"d", file(3), 0).unwrap();
        index.insert(b"d/f", file(4), 0).unwrap();
        index.dir(b"e/", 0).unwrap();
        index.insert(b"e", file(5), 0).unwrap();
        let tree = index.into_tree();
        let offset = |ino| match tree.node(ino) {
            Some(Node::File(member)) => member.offset,
            _ => u64::MAX,
        };
        assert_eq!(offset(tree.child(ROOT, b"a").unwrap()), 2);
        let d = tree.child(ROOT, b"d").unwrap();
        assert_eq!(offset(tree.child(d, b"f").unwrap()), 4);
        assert_eq!(offset(tree.child(ROOT, b"e").unwrap()), 5);
    }

    #[test]
    fn shares_nodes_between_hardlinks() {
        let mut index = Index::new("test");
        index.insert(b"x/original", file(1), 0).unwrap();
        index.hardlink(b"link", b"./x/original", 0).unwrap();
        for target in [&b"missing"[..], b"x", b"", b"../x"] {
            let error = index.hardlink(b"bad", target, 0).unwrap_err();
            assert_eq!(error.kind(), ErrorKind::Corrupt, "{target:?}");
        }
        let tree = index.into_tree();
        let x = tree.child(ROOT, b"x").unwrap();
        assert_eq!(tree.child(ROOT, b"link"), tree.child(x, b"original"));
    }

    #[test]
    fn shares_nodes_by_inode() {
        let mut index = Index::new("test");
        let ino = index.add(b"original", file(1), 0).unwrap();
        // A later member takes the name; the link keeps the node.
        index.insert(b"original", file(2), 0).unwrap();
        index.alias(b"d/link", ino, 0).unwrap();
        index.dir(b"dir", 0).unwrap();
        let dir = index.tree().child(ROOT, b"dir").unwrap();
        for bad in [dir, 1000] {
            let error = index.alias(b"bad", bad, 0).unwrap_err();
            assert_eq!(error.kind(), ErrorKind::Corrupt, "{bad}");
        }
        let error = index.alias(b"../evil", ino, 0).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::InvalidName);
        let tree = index.into_tree();
        let d = tree.child(ROOT, b"d").unwrap();
        assert_eq!(tree.child(d, b"link"), Some(ino));
        assert_ne!(tree.child(ROOT, b"original"), Some(ino));
    }
}

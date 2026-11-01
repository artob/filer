// This is free and unencumbered software released into the public domain.

use crate::{
    Error, ErrorKind, FileKind,
    archive::{ArchiveError, Member},
    tree::{Ino, Node, ROOT, Tree},
};
use alloc::vec::Vec;

/// The index of an archive under construction, applying the rules every
/// archive format shares.
///
/// A format's parser reads its headers in order and records each member
/// here by its raw name, a `/`-separated byte string. The index then:
///
/// - refuses names that could escape the archive root (absolute, or with a
///   `..` component, or containing NUL) with
///   [`InvalidName`](ErrorKind::InvalidName), ignoring empty and `.`
///   components, so `./a//b` is `a/b`;
/// - creates the directories a name implies;
/// - lets the later of duplicate members win, even where a file replaces a
///   directory or the reverse;
/// - makes a hardlink share its target's node, and so its file id.
///
/// Errors name the archive format and the offset of the header at fault
/// (see [`ArchiveError`]).
///
/// # Examples
///
/// ```
/// use filer_api::{archive::{Index, Member}, tree::{Node, ROOT}};
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
pub struct Index {
    format: &'static str,
    tree: Tree<Member>,
}

impl Index {
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
        self.make_dirs(&components);
        Ok(())
    }

    /// Records the non-directory member `name` as `node`, from the header
    /// at `offset`, replacing any earlier member of that name.
    ///
    /// # Errors
    ///
    /// [`InvalidName`](ErrorKind::InvalidName) if `name` could escape the
    /// archive root or names the root itself.
    pub fn insert(&mut self, name: &[u8], node: Node<Member>, offset: u64) -> Result<(), Error> {
        let components = self.components(name, offset)?;
        let Some((&last, parents)) = components.split_last() else {
            return Err(self.fail(ErrorKind::InvalidName, offset, "member names the root"));
        };
        let parent = self.make_dirs(parents);
        let ino = self.tree.add(node);
        self.tree.link(parent, last, ino).expect("a directory");
        Ok(())
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
        let components = self.components(name, offset)?;
        let Some((&last, parents)) = components.split_last() else {
            return Err(self.fail(ErrorKind::InvalidName, offset, "member names the root"));
        };
        let bad = || {
            ArchiveError::new(self.format, offset, "bad hardlink").into_error(ErrorKind::Corrupt)
        };
        let target = safe_components(target).ok_or_else(bad)?;
        let target = target
            .iter()
            .try_fold(ROOT, |dir, name| self.tree.child(dir, name))
            .filter(|&ino| self.tree.node(ino).kind() != FileKind::Dir)
            .filter(|_| !target.is_empty())
            .ok_or_else(bad)?;
        let parent = self.make_dirs(parents);
        self.tree.link(parent, last, target).expect("a directory");
        Ok(())
    }

    /// Returns the tree built so far.
    #[must_use]
    pub fn tree(&self) -> &Tree<Member> {
        &self.tree
    }

    /// Returns the finished tree, for an [`Archive`](crate::archive::Archive).
    #[must_use]
    pub fn into_tree(self) -> Tree<Member> {
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
    /// later member wins) whatever is in the way.
    fn make_dirs(&mut self, components: &[&[u8]]) -> Ino {
        let mut dir = ROOT;
        for &name in components {
            dir = match self.tree.make_dir(dir, name) {
                Ok(child) => child,
                Err(_) => {
                    let child = self.tree.add(Node::Dir(Default::default()));
                    self.tree.link(dir, name, child).expect("a directory");
                    child
                },
            };
        }
        dir
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
            Node::File(member) => member.offset,
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
}

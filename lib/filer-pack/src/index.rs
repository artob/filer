// This is free and unencumbered software released into the public domain.

use crate::{Limits, Member, Parser, Stored};
use alloc::vec::Vec;
use filer_api::{
    Error, ErrorKind, FileKind,
    tree::{Ino, Node, ROOT, Tree},
};

/// The index of an archive under construction, applying the rules every
/// archive format shares.
///
/// `C` is the format's [`Codec`](crate::Codec), [`Stored`] for
/// formats without compression. A format's parser reads its headers in
/// order and records each member here by its raw name, a `/`-separated
/// byte string. The index then:
///
/// - counts the members recorded, links included, against
///   [`max_entries`](Limits::max_entries), and checks names and link
///   targets against [`max_path_len`](Limits::max_path_len), failing with
///   [`LimitExceeded`](ErrorKind::LimitExceeded); a parser checks lengths
///   its headers declare with [`name_len`](Self::name_len) and
///   [`target_len`](Self::target_len) before reading them;
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
/// (see [`ArchiveError`](crate::ArchiveError)).
///
/// # Examples
///
/// ```
/// use filer_api::tree::{Node, ROOT};
/// use filer_pack::{Index, Limits, Member, Parser};
///
/// let mut index = Index::new(Parser::new("tar"), Limits::default());
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
    parser: Parser,
    limits: Limits,
    /// The members recorded so far, links included.
    members: usize,
    tree: Tree<Member<C>>,
}

impl<C> Index<C> {
    /// Returns an empty index for an archive that `parser` reads, within
    /// `limits`.
    #[must_use]
    pub fn new(parser: Parser, limits: Limits) -> Self {
        Self {
            parser,
            limits,
            members: 0,
            tree: Tree::new(),
        }
    }

    /// Returns the limits the archive is indexed within.
    #[must_use]
    pub const fn limits(&self) -> &Limits {
        &self.limits
    }

    /// Returns `len`, the length of a member name that the header at
    /// `offset` declares, if a name that long may be read.
    ///
    /// # Errors
    ///
    /// [`LimitExceeded`](ErrorKind::LimitExceeded) if it is longer than
    /// [`max_path_len`](Limits::max_path_len).
    pub fn name_len(&self, len: u64, offset: u64) -> Result<usize, Error> {
        self.path_len(len, offset, "name too long")
    }

    /// Returns `len`, the length of a link target that the header at
    /// `offset` declares, if a target that long may be read.
    ///
    /// # Errors
    ///
    /// [`LimitExceeded`](ErrorKind::LimitExceeded) if it is longer than
    /// [`max_path_len`](Limits::max_path_len).
    pub fn target_len(&self, len: u64, offset: u64) -> Result<usize, Error> {
        self.path_len(len, offset, "link target too long")
    }

    /// Records the directory `name`, from the header at `offset`.
    ///
    /// # Errors
    ///
    /// [`InvalidName`](ErrorKind::InvalidName) if `name` could escape the
    /// archive root; [`LimitExceeded`](ErrorKind::LimitExceeded) if it is
    /// one member too many, or its name is too long.
    pub fn dir(&mut self, name: &[u8], offset: u64) -> Result<(), Error> {
        let components = self.record(name, offset)?;
        self.make_dirs(&components, offset)?;
        Ok(())
    }

    /// Records the non-directory member `name` as `node`, from the header
    /// at `offset`, replacing any earlier member of that name.
    ///
    /// # Errors
    ///
    /// [`InvalidName`](ErrorKind::InvalidName) if `name` could escape the
    /// archive root or names the root itself;
    /// [`LimitExceeded`](ErrorKind::LimitExceeded) if it is one member too
    /// many, or its name or link target is too long.
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
        let components = self.record(name, offset)?;
        if let Node::Symlink(target) = &node
            && target.len() > self.limits.max_path_len
        {
            return Err(self.fail(ErrorKind::LimitExceeded, offset, "link target too long"));
        }
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
    /// index, or is a directory; [`LimitExceeded`](ErrorKind::LimitExceeded)
    /// as for [`insert`](Self::insert).
    pub fn alias(&mut self, name: &[u8], ino: Ino, offset: u64) -> Result<(), Error> {
        let components = self.record(name, offset)?;
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
    /// member, or is a directory; [`LimitExceeded`](ErrorKind::LimitExceeded)
    /// as for [`insert`](Self::insert).
    pub fn hardlink(&mut self, name: &[u8], target: &[u8], offset: u64) -> Result<(), Error> {
        // The name is checked first, so that an unsafe one is reported as
        // such whatever the target.
        self.components(name, offset)?;
        if target.len() > self.limits.max_path_len {
            return Err(self.fail(ErrorKind::LimitExceeded, offset, "link target too long"));
        }
        let bad = || self.fail(ErrorKind::Corrupt, offset, "bad hardlink");
        let target = safe_components(target).ok_or_else(bad)?;
        let target = target
            .iter()
            .try_fold(ROOT, |dir, name| self.tree.child(dir, name))
            .filter(|_| !target.is_empty())
            .ok_or_else(bad)?;
        self.alias(name, target, offset)
    }

    /// Replaces the member of the file `ino`, a node [`add`](Self::add)
    /// returned, from the header at `offset`, recording no member: for a
    /// format that stores a hardlinked file's data with a later link than
    /// the first, as cpio's `newc` does.
    ///
    /// # Errors
    ///
    /// [`Corrupt`](ErrorKind::Corrupt) if `ino` is not a file of this
    /// index.
    pub fn set_member(&mut self, ino: Ino, member: Member<C>, offset: u64) -> Result<(), Error> {
        if let Some(Node::File(file)) = self.tree.node_mut(ino) {
            *file = member;
            return Ok(());
        }
        Err(self.fail(ErrorKind::Corrupt, offset, "bad hardlink"))
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
        self.parser.fail(kind, offset, reason)
    }

    fn path_len(&self, len: u64, offset: u64, reason: &'static str) -> Result<usize, Error> {
        usize::try_from(len)
            .ok()
            .filter(|&len| len <= self.limits.max_path_len)
            .ok_or_else(|| self.fail(ErrorKind::LimitExceeded, offset, reason))
    }

    /// Counts the member `name`, from the header at `offset`, and returns
    /// its components.
    fn record<'n>(&mut self, name: &'n [u8], offset: u64) -> Result<Vec<&'n [u8]>, Error> {
        self.members = self.members.saturating_add(1);
        if self.members > self.limits.max_entries {
            return Err(self.fail(ErrorKind::LimitExceeded, offset, "too many members"));
        }
        if name.len() > self.limits.max_path_len {
            return Err(self.fail(ErrorKind::LimitExceeded, offset, "name too long"));
        }
        self.components(name, offset)
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
    use alloc::{boxed::Box, string::String, string::ToString};

    fn file(n: u64) -> Node<Member> {
        Node::File(Member::new(n, 0))
    }

    fn index() -> Index {
        Index::new(Parser::new("test"), Limits::default())
    }

    fn reason(error: &Error) -> String {
        core::error::Error::source(error).unwrap().to_string()
    }

    #[test]
    fn counts_recorded_members() {
        let limits = Limits::default().with_max_entries(3);
        let mut index = Index::new(Parser::new("test"), limits);
        // Implied directories are not members; links are.
        let ino = index.add(b"d/e/a", file(1), 0).unwrap();
        index.alias(b"b", ino, 1).unwrap();
        index.hardlink(b"c", b"b", 2).unwrap();
        let error = index.dir(b"f", 3).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::LimitExceeded);
        assert_eq!(reason(&error), "test header at offset 3: too many members");
        // Replacing a file's member records none.
        index.set_member(ino, Member::new(9, 0), 4).unwrap();
        let dir = index.tree().child(ROOT, b"d").unwrap();
        let error = index.set_member(dir, Member::new(9, 0), 5).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Corrupt);
        let tree = index.into_tree();
        assert_eq!(tree.node(ino), Some(&file(9)));
    }

    #[test]
    fn measures_names_and_link_targets() {
        let limits = Limits::default().with_max_path_len(4);
        let mut index = Index::new(Parser::new("test"), limits);
        assert_eq!(index.name_len(4, 0).unwrap(), 4);
        let error = index.name_len(5, 1).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::LimitExceeded);
        assert_eq!(reason(&error), "test header at offset 1: name too long");
        let error = index.target_len(u64::MAX, 2).unwrap_err();
        assert_eq!(
            reason(&error),
            "test header at offset 2: link target too long"
        );

        let link = |target: &[u8]| Node::Symlink(Box::from(target));
        let error = index.insert(b"abcde", file(0), 3).unwrap_err();
        assert_eq!(reason(&error), "test header at offset 3: name too long");
        let error = index.insert(b"l", link(b"abcde"), 4).unwrap_err();
        assert_eq!(
            reason(&error),
            "test header at offset 4: link target too long"
        );
        index.insert(b"abcd", file(0), 5).unwrap();
        let error = index.hardlink(b"h", b"abcde", 6).unwrap_err();
        assert_eq!(
            reason(&error),
            "test header at offset 6: link target too long"
        );
        index.insert(b"l", link(b"abcd"), 7).unwrap();
        index.hardlink(b"h", b"abcd", 8).unwrap();
    }

    #[test]
    fn refuses_names_that_could_escape() {
        let mut index = index();
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
        let mut index = index();
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
        let mut index = index();
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
        let mut index = index();
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

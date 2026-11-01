// This is free and unencumbered software released into the public domain.

use crate::{Error, ErrorKind, FileKind, LinkTarget, OpKind, Resolve, Symlinks};
use alloc::vec::Vec;

/// The most symlinks one resolution follows before failing with
/// [`SymlinkLoop`](ErrorKind::SymlinkLoop), as Linux `MAXSYMLINKS`.
pub const MAX_SYMLINK_HOPS: usize = 40;

/// Confined path resolution in userspace, one component at a time.
///
/// Drivers whose OS cannot confine resolution itself (macOS, Windows, older
/// Linux) or that have no OS at all (the in-memory driver) implement the
/// four primitives below; the provided [`resolve`](Self::resolve) method
/// then walks a path beneath a starting directory under a [`Resolve`]
/// policy. A driver with kernel-confined resolution, such as Linux
/// `openat2`, may override `resolve` instead.
///
/// The walk is race-free as long as the primitives are: every step looks up
/// one name in a directory node already held, so no step re-parses a path
/// from the root, and `..` steps back along the chain of directories the
/// walk itself entered, so it can never reach a directory outside them.
///
/// Paths and symlink targets are handled as bytes, so that non-UTF-8
/// targets and foreign names resolve too. Components are separated by `/`;
/// drivers translate other separators in link targets before returning
/// them, and Windows drivers use WTF-8.
pub trait Resolver {
    /// A driver's reference to a file, directory, or symlink, such as an
    /// open descriptor or an inode number.
    type Node: Clone;

    /// Returns the entry `name` of directory `dir`, without following it if
    /// it is a symlink.
    ///
    /// # Errors
    ///
    /// Typically [`NotFound`](ErrorKind::NotFound).
    fn lookup(&self, dir: &Self::Node, name: &[u8], op: OpKind) -> Result<Self::Node, Error>;

    /// Returns the type of `node`.
    fn kind(&self, node: &Self::Node) -> FileKind;

    /// Returns the target of the symlink `name` in directory `dir`.
    ///
    /// # Errors
    ///
    /// Any error reading the link.
    fn read_link(&self, dir: &Self::Node, name: &[u8], op: OpKind) -> Result<LinkTarget, Error>;

    /// Returns `true` if entering `child` from `dir` crosses a mount point.
    /// The default never does, which suits drivers without mounts.
    fn crosses_mount(&self, dir: &Self::Node, child: &Self::Node) -> bool {
        let _ = (dir, child);
        false
    }

    /// Resolves `path` beneath the directory `start` under `policy`.
    ///
    /// `path` is a relative path in bytes, such as a
    /// [`RelPath`](crate::path::RelPath) or a foreign name. Empty and `.`
    /// components are skipped and `..` steps back toward `start`, so `path`
    /// is treated like a relative symlink target. Symlinks are followed per
    /// [`Symlinks`], except a final one when `follow_final` is `false`,
    /// which is returned itself.
    ///
    /// Confinement is relative to `start`, not to the driver's root: with
    /// [`Symlinks::Beneath`], a `..` above `start` or an absolute symlink
    /// target fails with [`EscapeBlocked`](ErrorKind::EscapeBlocked); with
    /// [`Symlinks::InRoot`], `..` stays at `start` and absolute targets
    /// restart from it.
    ///
    /// # Errors
    ///
    /// - [`EscapeBlocked`](ErrorKind::EscapeBlocked) as described above;
    /// - [`SymlinkLoop`](ErrorKind::SymlinkLoop) for a symlink that must be
    ///   followed under [`Symlinks::Deny`], or more than
    ///   [`MAX_SYMLINK_HOPS`] symlinks;
    /// - [`NotADirectory`](ErrorKind::NotADirectory) for a component other
    ///   than the last that is not a directory;
    /// - [`CrossesDevices`](ErrorKind::CrossesDevices) for a mount point
    ///   when [`Resolve::cross_mounts`] is `false`;
    /// - any error from the primitives, such as
    ///   [`NotFound`](ErrorKind::NotFound).
    fn resolve(
        &self,
        start: &Self::Node,
        path: &[u8],
        policy: Resolve,
        follow_final: bool,
        op: OpKind,
    ) -> Result<Self::Node, Error> {
        let error = |kind| Error::new(kind, op);
        // The directories entered so far, from `start`; `..` pops them.
        let mut dirs = alloc::vec![start.clone()];
        // Components still to resolve, the next one last.
        let mut pending: Vec<Vec<u8>> = Vec::new();
        push_components(&mut pending, path);
        let mut hops = 0;

        while let Some(component) = pending.pop() {
            match component.as_slice() {
                b"" | b"." => {},
                b".." => {
                    if dirs.len() > 1 {
                        dirs.pop();
                    } else if policy.symlinks != Symlinks::InRoot {
                        return Err(error(ErrorKind::EscapeBlocked));
                    }
                },
                name => {
                    let dir = dirs.last().expect("`start` is never popped");
                    let child = self.lookup(dir, name, op)?;
                    if !policy.cross_mounts && self.crosses_mount(dir, &child) {
                        return Err(error(ErrorKind::CrossesDevices));
                    }
                    match self.kind(&child) {
                        FileKind::Symlink if follow_final || !pending.is_empty() => {
                            if policy.symlinks == Symlinks::Deny {
                                return Err(error(ErrorKind::SymlinkLoop));
                            }
                            hops += 1;
                            if hops > MAX_SYMLINK_HOPS {
                                return Err(error(ErrorKind::SymlinkLoop));
                            }
                            let target = self.read_link(dir, name, op)?;
                            let target = target.as_bytes();
                            if target.starts_with(b"/") {
                                if policy.symlinks != Symlinks::InRoot {
                                    return Err(error(ErrorKind::EscapeBlocked));
                                }
                                dirs.truncate(1);
                            }
                            push_components(&mut pending, target);
                        },
                        FileKind::Dir => dirs.push(child),
                        _ if pending.is_empty() => return Ok(child),
                        _ => return Err(error(ErrorKind::NotADirectory)),
                    }
                },
            }
        }
        Ok(dirs.pop().expect("`start` is never popped"))
    }
}

/// Queues the `/`-separated components of `path` so that the first is
/// resolved next.
fn push_components(pending: &mut Vec<Vec<u8>>, path: &[u8]) {
    if path.is_empty() {
        return;
    }
    pending.extend(path.split(|&b| b == b'/').rev().map(<[u8]>::to_vec));
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{collections::BTreeMap, vec};

    /// A toy filesystem: nodes by index, each with a mount number.
    struct Toy {
        nodes: Vec<(ToyNode, u8)>,
    }

    enum ToyNode {
        Dir(BTreeMap<Vec<u8>, usize>),
        File,
        Symlink(Vec<u8>),
    }

    impl Toy {
        /// Builds a tree from `(path, node, mount)` triples; the root is 0.
        fn new(entries: &[(&str, ToyNode, u8)]) -> Self {
            let mut toy = Self {
                nodes: vec![(ToyNode::Dir(BTreeMap::new()), 0)],
            };
            for (path, node, mount) in entries {
                let (parent, name) = path.rsplit_once('/').unwrap_or(("", path));
                let parent = toy.find(parent);
                let ino = toy.nodes.len();
                let node = match node {
                    ToyNode::Dir(_) => ToyNode::Dir(BTreeMap::new()),
                    ToyNode::File => ToyNode::File,
                    ToyNode::Symlink(target) => ToyNode::Symlink(target.clone()),
                };
                toy.nodes.push((node, *mount));
                let ToyNode::Dir(entries) = &mut toy.nodes[parent].0 else {
                    panic!("{path}: parent is not a directory");
                };
                entries.insert(name.as_bytes().to_vec(), ino);
            }
            toy
        }

        /// Finds a directory by literal path, without symlinks.
        fn find(&self, path: &str) -> usize {
            let mut ino = 0;
            for name in path.split('/').filter(|n| !n.is_empty()) {
                let ToyNode::Dir(entries) = &self.nodes[ino].0 else {
                    panic!("{path}: not a directory");
                };
                ino = entries[name.as_bytes()];
            }
            ino
        }

        fn resolve_from(
            &self,
            start: &str,
            path: &str,
            symlinks: Symlinks,
        ) -> Result<usize, ErrorKind> {
            let policy = Resolve {
                symlinks,
                ..Resolve::default()
            };
            self.resolve(
                &self.find(start),
                path.as_bytes(),
                policy,
                true,
                OpKind::Open,
            )
            .map_err(|error| error.kind())
        }

        fn resolve_path(&self, path: &str, symlinks: Symlinks) -> Result<usize, ErrorKind> {
            self.resolve_from("", path, symlinks)
        }
    }

    impl Resolver for Toy {
        type Node = usize;

        fn lookup(&self, dir: &usize, name: &[u8], op: OpKind) -> Result<usize, Error> {
            match &self.nodes[*dir].0 {
                ToyNode::Dir(entries) => entries
                    .get(name)
                    .copied()
                    .ok_or_else(|| Error::new(ErrorKind::NotFound, op)),
                _ => Err(Error::new(ErrorKind::NotADirectory, op)),
            }
        }

        fn kind(&self, node: &usize) -> FileKind {
            match self.nodes[*node].0 {
                ToyNode::Dir(_) => FileKind::Dir,
                ToyNode::File => FileKind::File,
                ToyNode::Symlink(_) => FileKind::Symlink,
            }
        }

        fn read_link(&self, dir: &usize, name: &[u8], op: OpKind) -> Result<LinkTarget, Error> {
            let link = self.lookup(dir, name, op)?;
            match &self.nodes[link].0 {
                ToyNode::Symlink(target) => Ok(LinkTarget::from_bytes(target.clone())),
                _ => Err(Error::new(ErrorKind::NotASymlink, op)),
            }
        }

        fn crosses_mount(&self, dir: &usize, child: &usize) -> bool {
            self.nodes[*dir].1 != self.nodes[*child].1
        }
    }

    fn dir() -> ToyNode {
        ToyNode::Dir(BTreeMap::new())
    }

    fn link(target: &str) -> ToyNode {
        ToyNode::Symlink(target.as_bytes().to_vec())
    }

    /// A tree exercising every escape route:
    ///
    /// ```text
    /// /a/b/file        /a/up -> ../a/b      /a/abs -> /a/b/file
    /// /a/out -> ../..  /a/self -> self       /a/f2 -> b/file
    /// /a/mnt (mount 1) /a/b/back -> ../../a  /a/dangling -> nope
    /// ```
    fn fixture() -> Toy {
        Toy::new(&[
            ("a", dir(), 0),
            ("a/b", dir(), 0),
            ("a/b/file", ToyNode::File, 0),
            ("a/b/back", link("../../a"), 0),
            ("a/up", link("../a/b"), 0),
            ("a/abs", link("/a/b/file"), 0),
            ("a/out", link("../.."), 0),
            ("a/self", link("self"), 0),
            ("a/f2", link("b/file"), 0),
            ("a/dangling", link("nope"), 0),
            ("a/mnt", dir(), 1),
        ])
    }

    #[test]
    fn resolves_plain_paths() {
        let toy = fixture();
        let file = toy.find("a/b/file");
        assert_eq!(toy.resolve_path("a/b/file", Symlinks::Beneath), Ok(file));
        assert_eq!(toy.resolve_path("", Symlinks::Beneath), Ok(0));
        assert_eq!(toy.resolve_path("a/./b//file", Symlinks::Beneath), Ok(file));
        assert_eq!(
            toy.resolve_path("a/b/../b/file", Symlinks::Beneath),
            Ok(file)
        );
    }

    #[test]
    fn follows_symlinks_beneath() {
        let toy = fixture();
        let file = toy.find("a/b/file");
        assert_eq!(toy.resolve_path("a/up/file", Symlinks::Beneath), Ok(file));
        assert_eq!(toy.resolve_path("a/f2", Symlinks::Beneath), Ok(file));
        assert_eq!(
            toy.resolve_path("a/b/back/b/file", Symlinks::Beneath),
            Ok(file)
        );
    }

    #[test]
    fn blocks_escapes_beneath() {
        let toy = fixture();
        let blocked = Err(ErrorKind::EscapeBlocked);
        assert_eq!(toy.resolve_path("..", Symlinks::Beneath), blocked);
        assert_eq!(toy.resolve_path("a/../..", Symlinks::Beneath), blocked);
        assert_eq!(toy.resolve_path("a/abs", Symlinks::Beneath), blocked);
        assert_eq!(toy.resolve_path("a/out", Symlinks::Beneath), blocked);
    }

    #[test]
    fn confines_to_the_start_not_the_root() {
        let toy = fixture();
        let blocked = Err(ErrorKind::EscapeBlocked);
        // `a/up` points to `../a/b`, which leaves `a` before re-entering it.
        assert_eq!(toy.resolve_from("a", "up", Symlinks::Beneath), blocked);
        assert_eq!(toy.resolve_from("a/b", "back", Symlinks::Beneath), blocked);
        assert_eq!(
            toy.resolve_from("a", "f2", Symlinks::Beneath),
            Ok(toy.find("a/b/file"))
        );
    }

    #[test]
    fn clamps_escapes_in_root() {
        let toy = fixture();
        assert_eq!(toy.resolve_path("..", Symlinks::InRoot), Ok(0));
        assert_eq!(toy.resolve_path("a/out", Symlinks::InRoot), Ok(0));
        assert_eq!(
            toy.resolve_path("a/abs", Symlinks::InRoot),
            Ok(toy.find("a/b/file"))
        );
        // The root is the start: from `a/b`, `..` stays at `a/b`.
        let file = toy.find("a/b/file");
        assert_eq!(
            toy.resolve_from("a/b", "../../file", Symlinks::InRoot),
            Ok(file)
        );
        // From `a`, the target `/a/b/file` means `a/a/b/file`.
        let from_a = toy.resolve_from("a", "abs", Symlinks::InRoot);
        assert_eq!(from_a, Err(ErrorKind::NotFound));
    }

    #[test]
    fn denies_symlinks() {
        let toy = fixture();
        let denied = Err(ErrorKind::SymlinkLoop);
        assert_eq!(toy.resolve_path("a/f2", Symlinks::Deny), denied);
        assert_eq!(toy.resolve_path("a/up/file", Symlinks::Deny), denied);
        assert_eq!(
            toy.resolve_path("a/b/file", Symlinks::Deny),
            Ok(toy.find("a/b/file"))
        );
    }

    #[test]
    fn returns_a_final_symlink_when_not_following() {
        let toy = fixture();
        let start = toy.find("a");
        let link = toy.lookup(&start, b"f2", OpKind::Stat).unwrap();
        for symlinks in [Symlinks::Deny, Symlinks::Beneath, Symlinks::InRoot] {
            let policy = Resolve {
                symlinks,
                ..Resolve::default()
            };
            let resolved = toy.resolve(&start, b"f2", policy, false, OpKind::Stat);
            assert_eq!(resolved.unwrap(), link);
        }
    }

    #[test]
    fn stops_loops() {
        let toy = fixture();
        assert_eq!(
            toy.resolve_path("a/self", Symlinks::Beneath),
            Err(ErrorKind::SymlinkLoop)
        );
    }

    #[test]
    fn allows_exactly_the_hop_limit() {
        let mut entries = vec![("f", ToyNode::File, 0)];
        let names: Vec<alloc::string::String> = (0..=MAX_SYMLINK_HOPS)
            .map(|i| alloc::format!("l{i}"))
            .collect();
        for i in 0..=MAX_SYMLINK_HOPS {
            let target = if i == 0 { "f" } else { names[i - 1].as_str() };
            entries.push((names[i].as_str(), link(target), 0));
        }
        let toy = Toy::new(&entries);
        let last = MAX_SYMLINK_HOPS - 1;
        assert_eq!(toy.resolve_path(&names[last], Symlinks::Beneath), Ok(1));
        assert_eq!(
            toy.resolve_path(&names[MAX_SYMLINK_HOPS], Symlinks::Beneath),
            Err(ErrorKind::SymlinkLoop)
        );
    }

    #[test]
    fn reports_non_directories_and_dangling_links() {
        let toy = fixture();
        assert_eq!(
            toy.resolve_path("a/b/file/x", Symlinks::Beneath),
            Err(ErrorKind::NotADirectory)
        );
        assert_eq!(
            toy.resolve_path("a/f2/x", Symlinks::Beneath),
            Err(ErrorKind::NotADirectory)
        );
        assert_eq!(
            toy.resolve_path("a/b/file/.", Symlinks::Beneath),
            Err(ErrorKind::NotADirectory)
        );
        assert_eq!(
            toy.resolve_path("a/dangling", Symlinks::Beneath),
            Err(ErrorKind::NotFound)
        );
    }

    #[test]
    fn honors_mount_crossing() {
        let toy = fixture();
        let mut policy = Resolve::default();
        assert!(
            toy.resolve(&0, b"a/mnt", policy, true, OpKind::Open)
                .is_ok()
        );
        policy.cross_mounts = false;
        let error = toy
            .resolve(&0, b"a/mnt", policy, true, OpKind::Open)
            .unwrap_err();
        assert_eq!(
            (error.kind(), error.op()),
            (ErrorKind::CrossesDevices, OpKind::Open)
        );
    }

    #[test]
    fn resolves_non_utf8_targets() {
        let toy = Toy::new(&[
            ("d", dir(), 0),
            ("l", ToyNode::Symlink(b"d/\xFF".to_vec()), 0),
        ]);
        // Insert the non-UTF-8 entry directly.
        let mut toy = toy;
        let ino = toy.nodes.len();
        toy.nodes.push((ToyNode::File, 0));
        let ToyNode::Dir(entries) = &mut toy.nodes[1].0 else {
            unreachable!()
        };
        entries.insert(b"\xFF".to_vec(), ino);
        assert_eq!(toy.resolve_path("l", Symlinks::Beneath), Ok(ino));
    }
}

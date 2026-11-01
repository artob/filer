// This is free and unencumbered software released into the public domain.

use crate::{
    Error, FileKind, LinkTarget, OpKind, Resolve,
    walk::{Next, Walk},
};
#[cfg(doc)]
use crate::{ErrorKind, Resolver};

/// Confined path resolution in userspace, for drivers whose primitives
/// block: system calls, as on macOS, Windows, and older Linux, whose OS
/// cannot confine resolution itself, or memory accesses, as in the
/// in-memory driver and archive trees.
///
/// This is [`Resolver`] with synchronous primitives, which fit every
/// executor: a thread-pool driver runs the whole walk as one job. The
/// provided [`resolve`](Self::resolve) method walks the same steps under
/// the same policy, with the same errors; see [`Resolver`] for the details,
/// and for why to implement this trait on a private type. A driver with
/// kernel-confined resolution, such as Linux `openat2`, may override
/// `resolve` instead.
pub trait BlockingResolver {
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

    /// Resolves `path` beneath the directory `start` under `policy`, as
    /// [`Resolver::resolve`] does.
    ///
    /// # Errors
    ///
    /// As [`Resolver::resolve`].
    fn resolve(
        &self,
        start: &Self::Node,
        path: &[u8],
        policy: Resolve,
        follow_final: bool,
        op: OpKind,
    ) -> Result<Self::Node, Error> {
        let mut walk = Walk::new(start.clone(), path, policy, follow_final, op);
        loop {
            let name = match walk.next()? {
                Next::Lookup(name) => name,
                Next::Done(node) => return Ok(node),
            };
            let child = self.lookup(walk.dir(), &name, op)?;
            let crosses = !policy.cross_mounts && self.crosses_mount(walk.dir(), &child);
            let kind = self.kind(&child);
            if walk.found(child, kind, crosses)? {
                let target = self.read_link(walk.dir(), &name, op)?;
                walk.follow(target.as_bytes())?;
            }
        }
    }
}

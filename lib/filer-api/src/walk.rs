// This is free and unencumbered software released into the public domain.

#[cfg(doc)]
use crate::{BlockingResolver, Resolver};
use crate::{Error, ErrorKind, FileKind, MAX_SYMLINK_HOPS, OpKind, Resolve, Symlinks};
use alloc::vec::Vec;

/// One resolution in progress: the walk of [`Resolver::resolve`] and
/// [`BlockingResolver::resolve`], written once without I/O so that both
/// drive the same steps.
///
/// The caller loops: [`next`](Self::next) yields a name to look up in
/// [`dir`](Self::dir) or the result; [`found`](Self::found) takes what the
/// lookup found and says whether it is a symlink to read and pass to
/// [`follow`](Self::follow).
pub(crate) struct Walk<N> {
    /// The directory the walk started in, which `..` never leaves.
    start: N,
    /// The directories entered since, the current one last; `..` pops
    /// them.
    dirs: Vec<N>,
    /// Components still to resolve, the next one last.
    pending: Vec<Vec<u8>>,
    hops: usize,
    policy: Resolve,
    follow_final: bool,
    op: OpKind,
}

/// What a [`Walk`] needs next.
pub(crate) enum Next<N> {
    /// The node for this name in [`Walk::dir`].
    Lookup(Vec<u8>),

    /// Nothing: the path resolves to this node.
    Done(N),
}

impl<N: Clone> Walk<N> {
    pub(crate) fn new(
        start: N,
        path: &[u8],
        policy: Resolve,
        follow_final: bool,
        op: OpKind,
    ) -> Self {
        let mut pending = Vec::new();
        push_components(&mut pending, path);
        Self {
            start,
            dirs: Vec::new(),
            pending,
            hops: 0,
            policy,
            follow_final,
            op,
        }
    }

    /// Returns the directory the walk is in.
    pub(crate) fn dir(&self) -> &N {
        self.dirs.last().unwrap_or(&self.start)
    }

    fn error(&self, kind: ErrorKind) -> Error {
        Error::new(kind, self.op)
    }

    /// Steps over `.` and `..` components, and returns the next name to
    /// look up, or the node the path resolves to.
    pub(crate) fn next(&mut self) -> Result<Next<N>, Error> {
        while let Some(component) = self.pending.pop() {
            match component.as_slice() {
                b"" | b"." => {},
                b".." => {
                    if self.dirs.pop().is_none() && self.policy.symlinks != Symlinks::InRoot {
                        return Err(self.error(ErrorKind::EscapeBlocked));
                    }
                },
                _ => return Ok(Next::Lookup(component)),
            }
        }
        Ok(Next::Done(
            self.dirs.pop().unwrap_or_else(|| self.start.clone()),
        ))
    }

    /// Takes the node a lookup found, of `kind`, and whether reaching it
    /// crossed a mount point. Returns `true` if it is a symlink to follow:
    /// the caller then reads it and passes its target to
    /// [`follow`](Self::follow).
    pub(crate) fn found(&mut self, child: N, kind: FileKind, crosses: bool) -> Result<bool, Error> {
        if crosses {
            return Err(self.error(ErrorKind::CrossesDevices));
        }
        match kind {
            FileKind::Symlink if self.follow_final || !self.pending.is_empty() => {
                self.hops = self.hops.saturating_add(1);
                if self.policy.symlinks == Symlinks::Deny || self.hops > MAX_SYMLINK_HOPS {
                    return Err(self.error(ErrorKind::SymlinkLoop));
                }
                Ok(true)
            },
            FileKind::Dir => {
                self.dirs.push(child);
                Ok(false)
            },
            _ if self.pending.is_empty() => {
                // The walk ends here: `next` returns the last directory
                // entered, so make that this node.
                self.dirs.push(child);
                Ok(false)
            },
            _ => Err(self.error(ErrorKind::NotADirectory)),
        }
    }

    /// Continues the walk at a symlink's `target`, relative to the
    /// symlink's directory, or restarting from the start if absolute.
    pub(crate) fn follow(&mut self, target: &[u8]) -> Result<(), Error> {
        if target.starts_with(b"/") {
            if self.policy.symlinks != Symlinks::InRoot {
                return Err(self.error(ErrorKind::EscapeBlocked));
            }
            self.dirs.clear();
        }
        push_components(&mut self.pending, target);
        Ok(())
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

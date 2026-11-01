// This is free and unencumbered software released into the public domain.

use crate::{
    Completion, DirCookie, EntryName, ErasedBufMut, Error, ErrorKind, Handle, MetaMask, OpKind,
    OpenHow, Target, XattrOp, path::RelPathBuf,
};

/// One driver operation, as data.
///
/// Drivers implement a single entry point that takes an `Op` and returns a
/// [`Completion`](crate::Completion). Representing operations as values
/// makes layering cheap: middleware and routing drivers match the
/// operations they care about and forward the rest, including operations
/// added after they were written, since the enum is `#[non_exhaustive]`.
///
/// Every field is owned, so the futures that carry operations are
/// `'static`. Only read operations exist so far.
#[derive(Debug)]
#[non_exhaustive]
pub enum Op {
    /// Opens `path` beneath the directory `at`, completing with
    /// [`Output::Opened`](crate::Output::Opened). The empty path reopens
    /// `at` itself.
    Open {
        /// The directory to resolve `path` against.
        at: Handle,
        /// The path to open.
        path: RelPathBuf,
        /// What to open and how to resolve it.
        how: OpenHow,
    },

    /// Closes `handle` explicitly, completing with
    /// [`Output::Done`](crate::Output::Done), to surface errors that some
    /// filesystems only report at close. Other clones of the handle become
    /// invalid.
    Close {
        /// The handle to close.
        handle: Handle,
    },

    /// Reads from the file at offset `off` into the spare capacity of
    /// `buf`, completing with [`Output::Read`](crate::Output::Read) and
    /// returning `buf` in the completion. Reading zero bytes into a buffer
    /// with spare capacity means end of file.
    Read {
        /// The file to read.
        file: Handle,
        /// The buffer to append to.
        buf: ErasedBufMut,
        /// The file offset to read from.
        off: u64,
    },

    /// Reads metadata, completing with
    /// [`Output::Metadata`](crate::Output::Metadata).
    Stat {
        /// What to read metadata of.
        target: Target,
        /// The fields wanted; the driver may return more or fewer.
        mask: MetaMask,
    },

    /// Lists up to `max` entries of a directory, completing with
    /// [`Output::Entries`](crate::Output::Entries).
    ReadDir {
        /// The directory to list.
        dir: Handle,
        /// Where to continue, or `None` to start from the beginning.
        cookie: Option<DirCookie>,
        /// The maximum number of entries to return.
        max: usize,
    },

    /// Reads the target of the symlink `name` in the directory `at`,
    /// completing with [`Output::LinkTarget`](crate::Output::LinkTarget).
    ReadLink {
        /// The directory containing the symlink.
        at: Handle,
        /// The symlink's name.
        name: EntryName,
    },

    /// Reads extended attributes; see [`XattrOp`].
    Xattr(XattrOp),
}

impl Op {
    /// Returns the kind of this operation.
    #[must_use]
    pub fn kind(&self) -> OpKind {
        match self {
            Self::Open { .. } => OpKind::Open,
            Self::Close { .. } => OpKind::Close,
            Self::Read { .. } => OpKind::Read,
            Self::Stat { .. } => OpKind::Stat,
            Self::ReadDir { .. } => OpKind::ReadDir,
            Self::ReadLink { .. } => OpKind::ReadLink,
            Self::Xattr(op) => op.kind(),
        }
    }

    /// Takes the buffer this operation carries, if any.
    #[must_use]
    pub fn into_buf(self) -> Option<ErasedBufMut> {
        match self {
            Self::Read { buf, .. } | Self::Xattr(XattrOp::Get { buf, .. }) => Some(buf),
            _ => None,
        }
    }

    /// Fails this operation with [`Unsupported`](ErrorKind::Unsupported),
    /// returning the buffer it carries, as the frontend expects of every
    /// completion.
    ///
    /// Drivers answer each operation they do not implement with this,
    /// including operations added after they were written, rather than
    /// dropping the operation and its buffer.
    #[must_use]
    pub fn unsupported(self) -> Completion {
        let completion = Completion::new(Err(Error::new(ErrorKind::Unsupported, self.kind())));
        match self.into_buf() {
            Some(buf) => completion.with_buf(buf),
            None => completion,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DriverKey, HandleKind, name};
    use alloc::vec::Vec;

    #[test]
    fn returns_buffers_when_unsupported() {
        let key = DriverKey::new();
        let file = key.handle(HandleKind::File, ());
        let read = Op::Read {
            file: file.clone(),
            buf: ErasedBufMut::new(Vec::<u8>::new()),
            off: 0,
        };
        let get = Op::Xattr(XattrOp::Get {
            handle: file.clone(),
            name: "user.comment".try_into().unwrap(),
            buf: ErasedBufMut::new(Vec::<u8>::new()),
        });
        for op in [read, get] {
            let kind = op.kind();
            let completion = op.unsupported();
            let error = completion.result.unwrap_err();
            assert_eq!((error.kind(), error.op()), (ErrorKind::Unsupported, kind));
            assert!(completion.buf.is_some());
        }
        let completion = Op::Close { handle: file }.unsupported();
        assert!(completion.buf.is_none());
    }

    #[test]
    fn reports_its_kind() {
        let key = DriverKey::new();
        let dir = key.handle(HandleKind::Dir, ());
        let ops = [
            Op::Open {
                at: dir.clone(),
                path: RelPathBuf::new(),
                how: OpenHow::dir(),
            },
            Op::Close {
                handle: dir.clone(),
            },
            Op::Read {
                file: dir.clone(),
                buf: ErasedBufMut::new(Vec::new()),
                off: 0,
            },
            Op::Stat {
                target: Target::Handle(dir.clone()),
                mask: MetaMask::all(),
            },
            Op::ReadDir {
                dir: dir.clone(),
                cookie: None,
                max: 1,
            },
            Op::ReadLink {
                at: dir.clone(),
                name: EntryName::from(name!("link")),
            },
            Op::Xattr(XattrOp::Get {
                handle: dir.clone(),
                name: "user.comment".try_into().unwrap(),
                buf: ErasedBufMut::new(Vec::new()),
            }),
            Op::Xattr(XattrOp::List { handle: dir }),
        ];
        let kinds: Vec<OpKind> = ops.iter().map(Op::kind).collect();
        assert_eq!(
            kinds,
            [
                OpKind::Open,
                OpKind::Close,
                OpKind::Read,
                OpKind::Stat,
                OpKind::ReadDir,
                OpKind::ReadLink,
                OpKind::GetXattr,
                OpKind::ListXattrs,
            ]
        );
    }
}

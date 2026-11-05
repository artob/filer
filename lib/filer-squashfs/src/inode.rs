// This is free and unencumbered software released into the public domain.

//! Inodes, in the inode table: a common header, then what each type adds,
//! basic or extended, the extended ones with an index of extended
//! attributes and wider fields.

use crate::{
    SquashfsError,
    meta::{Cursor, Metadata},
    superblock::Superblock,
};
use alloc::vec::Vec;
use filer_api::{Error, FileKind, OpKind, ReadAt};

/// What marks a fragment index, or an index of extended attributes, as
/// absent.
const NONE: u32 = u32::MAX;

/// The longest symlink target, as Linux's paths are.
const MAX_TARGET: u32 = 4096;

/// An inode.
#[derive(Clone, Debug)]
pub(crate) struct Inode {
    /// Its number, unique in the image.
    pub(crate) number: u32,
    pub(crate) kind: FileKind,
    /// Its modification time, in seconds since the Unix epoch.
    pub(crate) mtime: u32,
    /// The index of its extended attributes, if it has any.
    pub(crate) xattr: Option<u32>,
    pub(crate) body: Body,
}

/// What an inode adds, by its kind.
#[derive(Clone, Debug)]
pub(crate) enum Body {
    /// A directory's listing: where it begins, in the directory table,
    /// and its length, which the inode gives as 3 more.
    Dir {
        listing: Cursor,
        len: u32,
    },
    File(Layout),
    Symlink(Vec<u8>),
    /// A device, FIFO, or socket.
    Special,
}

/// Where a regular file's contents are.
#[derive(Clone, Debug)]
pub(crate) struct Layout {
    /// Its size in bytes.
    pub(crate) size: u64,
    /// The position of its first block.
    pub(crate) start: u64,
    /// How many blocks it has, each of the image's block size but for
    /// the last, unless its tail is in a fragment.
    pub(crate) blocks: u64,
    /// Where the sizes of its blocks are listed, 4 bytes each.
    pub(crate) list: Cursor,
    /// The fragment its tail is in, and the tail's offset there.
    pub(crate) fragment: Option<(u32, u32)>,
}

impl Inode {
    /// Returns its size: a file's, a directory's as the inode gives it,
    /// or a symlink target's length.
    pub(crate) fn size(&self) -> u64 {
        match &self.body {
            Body::Dir { len, .. } => u64::from(*len).saturating_add(3),
            Body::File(layout) => layout.size,
            Body::Symlink(target) => u64::try_from(target.len()).unwrap_or(u64::MAX),
            Body::Special => 0,
        }
    }
}

/// Reads the inode that `reference` names.
pub(crate) async fn read<S: ReadAt>(
    meta: &Metadata<S>,
    sb: &Superblock,
    reference: u64,
    op: OpKind,
) -> Result<Inode, Error> {
    let bad = || SquashfsError::BadInode { reference }.into_error(op);
    let at = Cursor::of(sb.inode_table, reference)
        .filter(|at| at.block < sb.dir_table)
        .ok_or_else(bad)?;
    let mut at = meta.reader(at);
    let header = at.read(16, op).await?;
    let u16_at = |at| filer_fs::le::u16_at(&header, at).unwrap_or(0);
    let u32_at = |at| filer_fs::le::u32_at(&header, at).unwrap_or(0);
    let (kind_id, mtime, number) = (u16_at(0), u32_at(8), u32_at(12));
    let kind = kind(kind_id).ok_or_else(bad)?;
    let extended = kind_id >= 8;
    // Each kind's fields, after the link count where it has one.
    let (body, xattr) = match kind {
        FileKind::Dir if extended => {
            let fields = at.read(24, op).await?;
            let field = |at| filer_fs::le::u32_at(&fields, at).unwrap_or(0);
            let offset = filer_fs::le::u16_at(&fields, 18).unwrap_or(0);
            let listing = listing(sb, field(8), offset).ok_or_else(bad)?;
            let len = field(4).checked_sub(3).ok_or_else(bad)?;
            (Body::Dir { listing, len }, field(20))
        },
        FileKind::Dir => {
            let fields = at.read(16, op).await?;
            let size = filer_fs::le::u16_at(&fields, 8).unwrap_or(0);
            let offset = filer_fs::le::u16_at(&fields, 10).unwrap_or(0);
            let block = filer_fs::le::u32_at(&fields, 0).unwrap_or(0);
            let listing = listing(sb, block, offset).ok_or_else(bad)?;
            let len = u32::from(size).checked_sub(3).ok_or_else(bad)?;
            (Body::Dir { listing, len }, NONE)
        },
        FileKind::File if extended => {
            let fields = at.read(40, op).await?;
            let u32_at = |at| filer_fs::le::u32_at(&fields, at).unwrap_or(0);
            let u64_at = |at| filer_fs::le::u64_at(&fields, at).unwrap_or(0);
            let (start, size) = (u64_at(0), u64_at(8));
            let layout =
                layout(sb, start, size, u32_at(28), u32_at(32), at.cursor()).ok_or_else(bad)?;
            (Body::File(layout), u32_at(36))
        },
        FileKind::File => {
            let fields = at.read(16, op).await?;
            let u32_at = |at| filer_fs::le::u32_at(&fields, at).unwrap_or(0);
            let (start, size) = (u64::from(u32_at(0)), u64::from(u32_at(12)));
            let layout =
                layout(sb, start, size, u32_at(4), u32_at(8), at.cursor()).ok_or_else(bad)?;
            (Body::File(layout), NONE)
        },
        FileKind::Symlink => {
            let fields = at.read(8, op).await?;
            let len = filer_fs::le::u32_at(&fields, 4).unwrap_or(0);
            if len == 0 || len > MAX_TARGET {
                return Err(SquashfsError::BadSymlink { inode: number }.into_error(op));
            }
            let target = at.read(usize::try_from(len).unwrap_or(0), op).await?;
            let xattr = if extended { at.u32(op).await? } else { NONE };
            (Body::Symlink(target), xattr)
        },
        // Devices, after their numbers; FIFOs and sockets.
        FileKind::BlockDevice | FileKind::CharDevice => {
            let fields = at.read(if extended { 12 } else { 8 }, op).await?;
            let xattr = filer_fs::le::u32_at(&fields, 8).unwrap_or(NONE);
            (Body::Special, xattr)
        },
        _ => {
            let fields = at.read(if extended { 8 } else { 4 }, op).await?;
            let xattr = filer_fs::le::u32_at(&fields, 4).unwrap_or(NONE);
            (Body::Special, xattr)
        },
    };
    Ok(Inode {
        number,
        kind,
        mtime,
        xattr: (xattr != NONE).then_some(xattr),
        body,
    })
}

/// Returns the kind of inode of type `id`, basic (1 to 7) or extended (8
/// to 14), as directory entries give them too.
pub(crate) const fn kind(id: u16) -> Option<FileKind> {
    let kind = match id {
        1 | 8 => FileKind::Dir,
        2 | 9 => FileKind::File,
        3 | 10 => FileKind::Symlink,
        4 | 11 => FileKind::BlockDevice,
        5 | 12 => FileKind::CharDevice,
        6 | 13 => FileKind::Fifo,
        7 | 14 => FileKind::Socket,
        _ => return None,
    };
    Some(kind)
}

/// Returns where a directory's listing begins: `block` bytes into the
/// directory table, and `offset` bytes into that block.
fn listing(sb: &Superblock, block: u32, offset: u16) -> Option<Cursor> {
    let offset = usize::from(offset);
    let block = sb.dir_table.checked_add(u64::from(block))?;
    (offset < crate::meta::BLOCK && block < sb.bytes_used).then_some(Cursor { block, offset })
}

/// Returns a regular file's layout, of `size` bytes from `start`, its tail
/// in fragment `fragment` at `offset` unless that is `NONE`, its blocks
/// listed at `list`.
fn layout(
    sb: &Superblock,
    start: u64,
    size: u64,
    fragment: u32,
    offset: u32,
    list: Cursor,
) -> Option<Layout> {
    let block = u64::from(sb.block_size);
    let tail = size.checked_rem(block)?;
    let fragment = (fragment != NONE).then_some((fragment, offset));
    let blocks = match fragment {
        // A fragment holds a tail of less than a block, which fits.
        Some((index, offset)) => {
            let fits = u64::from(offset).checked_add(tail)? <= block;
            if index >= sb.fragments || tail == 0 || !fits {
                return None;
            }
            size.checked_div(block)?
        },
        None => size.div_ceil(block),
    };
    (blocks == 0 || start < sb.bytes_used).then_some(Layout {
        size,
        start,
        blocks,
        list,
        fragment,
    })
}

// This is free and unencumbered software released into the public domain.

//! Symlink targets: local ones in the inode, remote ones in data blocks.
//! With version 5, each extent of those starts with a header of its own,
//! self-describing (`crc::REMOTE`) over the whole extent, which names the
//! part of the target it holds after the header.

use crate::{
    XfsError, crc,
    file_map::FileMap,
    inode::{Format, Inode},
    superblock::Superblock,
};
use alloc::vec::Vec;
use filer_api::{Error, OpKind, ReadAt};
use filer_fs::{be, read_exact};

/// The longest target: Linux's `PATH_MAX` less its NUL.
const MAX_TARGET: u64 = 1024;

/// The magic number of a version 5 target's header, `XSLM`, and its size.
const MAGIC: u32 = 0x5853_4C4D;
const HEADER: usize = 56;

/// Reads the target of symlink `inode`, which must not be empty, contain
/// NUL, or be longer than 1024 bytes.
pub(crate) async fn read<S: ReadAt + ?Sized>(
    source: &S,
    sb: &Superblock,
    inode: &Inode,
    op: OpKind,
) -> Result<Vec<u8>, Error> {
    let fail = |error: XfsError| error.into_error(op);
    let bad = || {
        fail(XfsError::BadSymlink {
            inode: inode.number(),
        })
    };
    let size = inode.size();
    if size == 0 || size > MAX_TARGET {
        return Err(bad());
    }
    let len = usize::try_from(size).map_err(|_| bad())?;
    let target = if inode.data().format == Format::Local {
        inode.data().bytes.clone()
    } else {
        let map = FileMap::data(inode, sb).map_err(fail)?;
        if sb.is_v5() {
            remote_v5(source, sb, &map, inode.number(), len, op).await?
        } else {
            map.read(source, sb, size, 0, len, op).await?
        }
    };
    if target.len() != len || target.contains(&0) {
        return Err(bad());
    }
    Ok(target)
}

/// Reads a version 5 remote target of `len` bytes, of symlink `inode`
/// mapped by `map`: from as many blocks as hold it after a header in each
/// extent of them.
async fn remote_v5<S: ReadAt + ?Sized>(
    source: &S,
    sb: &Superblock,
    map: &FileMap,
    inode: u64,
    len: usize,
    op: OpKind,
) -> Result<Vec<u8>, Error> {
    let fail = |error: XfsError| error.into_error(op);
    let bad = || fail(XfsError::BadSymlink { inode });
    let block = usize::try_from(sb.block_size()).map_err(|_| bad())?;
    let room = block.checked_sub(HEADER).ok_or_else(bad)?;
    let blocks = u64::try_from(len.div_ceil(room)).map_err(|_| bad())?;
    let mut target = Vec::with_capacity(len);
    let mut logical = 0;
    while logical < blocks {
        let run = map
            .map(source, sb, logical, blocks.saturating_sub(logical), op)
            .await?;
        let physical = run.physical.ok_or_else(bad)?;
        let offset = sb.block_offset(physical).ok_or_else(bad)?;
        let run_len = usize::try_from(run.len)
            .ok()
            .and_then(|n| n.checked_mul(block))
            .ok_or_else(bad)?;
        let bytes = read_exact(source, offset, run_len, op).await?;
        crc::check(&bytes, crc::REMOTE, sb, offset, inode).map_err(fail)?;
        let part = len
            .saturating_sub(target.len())
            .min(run_len.saturating_sub(HEADER));
        let header = (
            be::u32_at(&bytes, 0),
            be::u32_at(&bytes, 4).and_then(|at| usize::try_from(at).ok()),
            be::u32_at(&bytes, 8).and_then(|len| usize::try_from(len).ok()),
        );
        if header != (Some(MAGIC), Some(target.len()), Some(part)) {
            return Err(bad());
        }
        target.extend_from_slice(
            bytes
                .get(HEADER..HEADER.saturating_add(part))
                .ok_or_else(bad)?,
        );
        logical = logical.saturating_add(run.len);
    }
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        file_map::tests::record,
        inode::tests::Raw,
        superblock::{Version, tests::Fields},
    };
    use alloc::vec;
    use filer_api::ErrorKind;
    use filer_spec::block_on;

    fn local(target: &[u8]) -> Inode {
        let raw = Raw::local(0o120_777, target);
        Inode::parse(&raw.bytes(256), 131, Version::V4).unwrap()
    }

    fn remote(size: u64, block: u64) -> Inode {
        let mut raw = Raw::file(size);
        raw.mode = 0o120_777;
        raw.extents = 1;
        raw.literal = record(0, block, 1, false).to_vec();
        Inode::parse(&raw.bytes(256), 131, Version::V4).unwrap()
    }

    #[test]
    fn reads_local_and_remote_targets() {
        let sb = Superblock::parse(&Fields::new().bytes()).unwrap();
        let mut image = vec![0; 4096 * 1024];
        let long = [b'x'; 1024];
        image[10 * 4096..10 * 4096 + 1024].copy_from_slice(&long);
        let read = |inode: &Inode| block_on(read(&image, &sb, inode, OpKind::ReadLink));
        assert_eq!(read(&local(b"target")).unwrap(), b"target");
        assert_eq!(read(&remote(1024, 10)).unwrap(), long);
        assert_eq!(read(&remote(200, 10)).unwrap(), &long[..200]);

        let corrupt = |inode: &Inode| {
            assert_eq!(read(inode).unwrap_err().kind(), ErrorKind::Corrupt);
        };
        corrupt(&local(b"a\0b"));
        corrupt(&remote(1025, 10));
        corrupt(&remote(0, 10));
        // Zeros past the target's bytes.
        corrupt(&remote(1100, 10));
    }
}

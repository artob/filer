// This is free and unencumbered software released into the public domain.

//! Symlink targets: fast ones in `i_block`, slow ones in data blocks.

use crate::{
    Ext4Error,
    block_map::BlockMap,
    inode::{I_BLOCK_SIZE, Inode},
    superblock::Superblock,
};
use alloc::vec::Vec;
use filer_api::{Error, OpKind, ReadAt};

/// Returns `true` if symlink `inode` keeps its target in `i_block`: a
/// target under 60 bytes is kept there unless the inode has data blocks,
/// which its sector count tells, less its extended attribute block's.
pub(crate) fn is_fast(inode: &Inode, sb: &Superblock) -> bool {
    let attributes = if inode.file_acl() == 0 {
        0
    } else {
        sb.block_size() / 512
    };
    inode.size() < u64::try_from(I_BLOCK_SIZE).unwrap_or(u64::MAX) && inode.sectors() == attributes
}

/// Reads the target of symlink `inode`, which must not be empty, contain
/// NUL, or be longer than a block.
pub(crate) async fn read<S: ReadAt + ?Sized>(
    source: &S,
    sb: &Superblock,
    inode: &Inode,
    op: OpKind,
) -> Result<Vec<u8>, Error> {
    let fail = |error: Ext4Error| error.into_error(op);
    let bad = || {
        fail(Ext4Error::BadSymlink {
            inode: inode.number(),
        })
    };
    let size = inode.size();
    if size == 0 || size >= u64::from(sb.block_size()) {
        return Err(bad());
    }
    let len = usize::try_from(size).map_err(|_| bad())?;
    let target = if is_fast(inode, sb) {
        inode.block().get(..len).ok_or_else(bad)?.to_vec()
    } else {
        let map = BlockMap::new(inode, sb).map_err(fail)?;
        map.read(source, sb, size, 0, len, op).await?
    };
    if target.len() != len || target.contains(&0) {
        return Err(bad());
    }
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{inode::tests::Raw, superblock::tests::Fields};
    use alloc::vec;
    use filer_spec::block_on;

    fn symlink(target: &[u8], sectors: u32, file_acl: u32) -> Inode {
        let mut raw = Raw::file(u64::try_from(target.len()).unwrap());
        raw.mode = 0o120_777;
        raw.sectors = sectors;
        raw.file_acl = file_acl;
        let n = target.len().min(I_BLOCK_SIZE);
        raw.block[..n].copy_from_slice(&target[..n]);
        Inode::parse(&raw.bytes(256), 12).unwrap()
    }

    #[test]
    fn reads_fast_and_slow_targets() {
        let sb = Superblock::parse(&Fields::new().bytes()).unwrap();
        let mut image = vec![0; 4096 * 1024];
        let read =
            |image: &Vec<u8>, inode: &Inode| block_on(read(image, &sb, inode, OpKind::ReadLink));
        let fast = symlink(b"a/b/c.txt", 0, 0);
        assert!(is_fast(&fast, &sb));
        assert_eq!(read(&image, &fast).unwrap(), b"a/b/c.txt");
        // With an attribute block, of two sectors.
        let fast = symlink(b"target", 2, 77);
        assert!(is_fast(&fast, &sb));
        // A long target, in block 100.
        let long = [b'x'; 100];
        let mut raw = Raw::file(100);
        raw.mode = 0o120_777;
        raw.sectors = 2;
        raw.block[..4].copy_from_slice(&100u32.to_le_bytes());
        let slow = Inode::parse(&raw.bytes(256), 13).unwrap();
        assert!(!is_fast(&slow, &sb));
        image[100 * 1024..100 * 1024 + 100].copy_from_slice(&long);
        assert_eq!(read(&image, &slow).unwrap(), long);
        // A short target in a block.
        let short = symlink(&[100, 0, 0, 0], 2, 0);
        assert!(!is_fast(&short, &sb));
        assert_eq!(read(&image, &short).unwrap(), b"xxxx");
    }

    #[test]
    fn refuses_bad_targets() {
        let sb = Superblock::parse(&Fields::new().bytes()).unwrap();
        let image = vec![0; 4096 * 1024];
        for inode in [symlink(b"", 0, 0), symlink(b"a\0b", 0, 0)] {
            let error = block_on(read(&image, &sb, &inode, OpKind::ReadLink)).unwrap_err();
            assert_eq!(error.kind(), filer_api::ErrorKind::Corrupt);
        }
        let mut raw = Raw::file(1024);
        raw.mode = 0o120_777;
        let inode = Inode::parse(&raw.bytes(256), 12).unwrap();
        let error = block_on(read(&image, &sb, &inode, OpKind::ReadLink)).unwrap_err();
        assert_eq!(error.kind(), filer_api::ErrorKind::Corrupt);
    }
}

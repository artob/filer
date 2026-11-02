// This is free and unencumbered software released into the public domain.

//! Symlink targets: local ones in the inode, remote ones in data blocks.

use crate::{
    XfsError,
    file_map::FileMap,
    inode::{Format, Inode},
    superblock::Superblock,
};
use alloc::vec::Vec;
use filer_api::{Error, OpKind, ReadAt};

/// The longest target: Linux's `PATH_MAX` less its NUL.
const MAX_TARGET: u64 = 1024;

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

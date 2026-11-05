// This is free and unencumbered software released into the public domain.

//! The volume descriptor set, from sector 16 to its terminator: the
//! primary volume descriptor, and supplementary ones, of Joliet's names
//! or of ISO 9660:1999's, each with a root directory of its own over the
//! same files.

use crate::CdfsError;
use filer_api::{Error, ErrorKind, OpKind, ReadAt};
use filer_fs::le;

/// The size of a logical sector, which holds a volume descriptor, and in
/// which a directory record ends.
pub(crate) const SECTOR: usize = 2048;

/// Where the volume descriptor set starts, in sectors.
const FIRST: u64 = 16;

/// How many sectors it may take, as no image's set has as many.
const MAX_SECTORS: u64 = 100;

/// How a tree's names are recorded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Names {
    /// The primary descriptor's: `NAME.EXT;1`, of few characters.
    Plain,
    /// Joliet's: UTF-16, big-endian, up to 64 characters.
    Joliet,
    /// ISO 9660:1999's: any bytes, up to 207.
    Enhanced,
}

/// A directory tree a volume descriptor roots.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Tree {
    pub(crate) names: Names,
    /// The logical block size.
    pub(crate) block: u32,
    /// The root directory's record.
    pub(crate) root: [u8; 34],
}

/// The trees an image's descriptors root, and an id for the image.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Descriptors {
    pub(crate) primary: Tree,
    pub(crate) joliet: Option<Tree>,
    pub(crate) enhanced: Option<Tree>,
    /// A hash of what identifies the image: its size, its root's place,
    /// and when it was made.
    pub(crate) id: u64,
}

/// Reads the volume descriptor set of the image in `source`.
pub(crate) async fn read<S: ReadAt>(source: &S, op: OpKind) -> Result<Descriptors, Error> {
    let (mut primary, mut joliet, mut enhanced, mut id) = (None, None::<(u8, Tree)>, None, 0);
    for index in 0..MAX_SECTORS {
        let at = FIRST
            .saturating_add(index)
            .saturating_mul(u64::try_from(SECTOR).unwrap_or(u64::MAX));
        let bytes = match filer_fs::read_exact(source, at, SECTOR, op).await {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == ErrorKind::UnexpectedEof && index > 0 => break,
            Err(error) if error.kind() == ErrorKind::UnexpectedEof => {
                return Err(CdfsError::NotIso9660.into_error(op));
            },
            Err(error) => return Err(error),
        };
        if bytes.get(1..6) != Some(b"CD001") {
            if index == 0 {
                return Err(CdfsError::NotIso9660.into_error(op));
            }
            break;
        }
        match (bytes.first(), bytes.get(6)) {
            // The terminator.
            (Some(255), _) => break,
            (Some(1), _) if primary.is_none() => {
                primary = Some(tree(&bytes, Names::Plain).map_err(|error| error.into_error(op))?);
                id = identity(&bytes);
            },
            // ISO 9660:1999's, of version 2.
            (Some(2), Some(2)) if enhanced.is_none() => {
                enhanced = tree(&bytes, Names::Enhanced).ok();
            },
            // Joliet's, named by its escape sequence, the highest level
            // first.
            (Some(2), _) => {
                let level = match bytes.get(88..91) {
                    Some([0x25, 0x2F, level @ (0x40 | 0x43 | 0x45)]) => *level,
                    _ => continue,
                };
                if joliet.is_none_or(|(known, _)| level > known)
                    && let Ok(found) = tree(&bytes, Names::Joliet)
                {
                    joliet = Some((level, found));
                }
            },
            _ => {},
        }
    }
    Ok(Descriptors {
        primary: primary.ok_or_else(|| CdfsError::NoPrimary.into_error(op))?,
        joliet: joliet.map(|(_, tree)| tree),
        enhanced,
        id,
    })
}

/// Returns the tree descriptor `bytes` roots.
fn tree(bytes: &[u8], names: Names) -> Result<Tree, CdfsError> {
    let block = le::u16_at(bytes, 128).ok_or(CdfsError::BadRoot)?;
    if !matches!(block, 512 | 1024 | 2048) {
        return Err(CdfsError::BadBlockSize(block));
    }
    let root = bytes
        .get(156..190)
        .and_then(|root| <[u8; 34]>::try_from(root).ok())
        .ok_or(CdfsError::BadRoot)?;
    Ok(Tree {
        names,
        block: u32::from(block),
        root,
    })
}

/// Returns the 64-bit FNV-1a hash of what identifies an image in its
/// primary descriptor `bytes`: its size in blocks, its root's record, and
/// when it was made.
fn identity(bytes: &[u8]) -> u64 {
    [80..84, 156..190, 813..830]
        .into_iter()
        .filter_map(|range| bytes.get(range))
        .flatten()
        .fold(0xCBF2_9CE4_8422_2325, |hash, &byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01B3)
        })
}

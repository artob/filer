// This is free and unencumbered software released into the public domain.

//! System Use entries, as the System Use Sharing Protocol lays them out in
//! a directory record after its name, and continues them in areas `CE`
//! entries point to; and the Rock Ridge entries among them, which give a
//! file its POSIX name, mode, symlink target, and times, and relocate
//! directories nested too deep. Each entry is a two-letter signature, its
//! length, its version, and its data.

use crate::{CdfsError, read, time};
use alloc::vec::Vec;
use filer_api::{Error, OpKind, ReadAt, Timestamp};
use filer_fs::le;

/// How many continuation areas one record's entries may span.
const MAX_CONTINUATIONS: u32 = 32;

/// The longest name, beyond any file system's.
const MAX_NAME: usize = 4096;

/// The longest symlink target, as Linux's paths are.
const MAX_TARGET: usize = 4096;

/// What a record's entries say, as far as Filer reports it.
#[derive(Clone, Debug, Default)]
pub(crate) struct Rock {
    /// Its POSIX name, from `NM` entries.
    pub(crate) name: Option<Vec<u8>>,
    /// Its POSIX mode, from a `PX` entry.
    pub(crate) mode: Option<u32>,
    /// Its file serial number, which hard links share, from a `PX` entry
    /// of Rock Ridge 1.12.
    pub(crate) serial: Option<u32>,
    /// Its symlink target, from `SL` entries.
    pub(crate) target: Option<Vec<u8>>,
    /// Where the directory this record stands for was relocated to, from a
    /// `CL` entry: the logical block of its extent.
    pub(crate) child: Option<u32>,
    /// Whether this is a relocated directory, from an `RE` entry, which is
    /// shown where its `CL` record stands, not here.
    pub(crate) relocated: bool,
    /// Its times, from a `TF` entry.
    pub(crate) times: Times,
    /// How zisofs compresses it, from a `ZF` entry.
    pub(crate) zisofs: Option<Zf>,
    /// Whether it is sparse, from an `SF` entry.
    pub(crate) sparse: bool,
}

/// A file's times, as a `TF` entry records them.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Times {
    pub(crate) created: Option<Timestamp>,
    pub(crate) modified: Option<Timestamp>,
    pub(crate) accessed: Option<Timestamp>,
    /// When its attributes changed.
    pub(crate) changed: Option<Timestamp>,
}

/// What a `ZF` entry says of a compressed file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Zf {
    /// The algorithm, `pz` for zisofs.
    pub(crate) algorithm: [u8; 2],
    /// The header's size in 4-byte units.
    pub(crate) header: u8,
    /// The base-2 logarithm of its block size.
    pub(crate) log: u8,
    /// Its size, uncompressed.
    pub(crate) size: u32,
}

/// Returns how many bytes to skip at the start of every other record's
/// System Use field if `field`, the root directory's first, begins with an
/// `SP` entry, which says entries are in use; or `None` if it does not.
pub(crate) fn sharing(field: &[u8]) -> Option<u8> {
    match field {
        [b'S', b'P', len, _, 0xBE, 0xEF, skip, ..] if *len >= 7 => Some(*skip),
        _ => None,
    }
}

/// Reads the entries of the record at `at`, whose System Use field is
/// `field`, after `skip` bytes, following continuation areas in the image
/// in `source`, of logical blocks of `block` bytes.
pub(crate) async fn read<S: ReadAt>(
    source: &S,
    block: u32,
    field: &[u8],
    skip: u8,
    at: u64,
    op: OpKind,
) -> Result<Rock, Error> {
    let bad = || CdfsError::BadSystemUse { at }.into_error(op);
    let mut rock = Rock::default();
    let mut link = Link::default();
    let mut area = field.get(usize::from(skip)..).unwrap_or_default().to_vec();
    let mut continuations = 0_u32;
    loop {
        let mut next = None;
        let mut rest = &area[..];
        while let Some(entry) = split(&mut rest).map_err(|()| bad())? {
            let ok = match entry.get(..2).unwrap_or_default() {
                b"CE" => continuation(entry, block).map(|found| next = Some(found)),
                b"ST" => break,
                b"NM" => name(&mut rock, entry),
                b"PX" => le::u32_at(entry, 4).map(|mode| {
                    rock.mode = Some(mode);
                    rock.serial = le::u32_at(entry, 36).filter(|_| entry.len() >= 44);
                }),
                b"SL" => {
                    let target = link.add(entry);
                    if target.is_none() || link.target.len() > MAX_TARGET {
                        return Err(CdfsError::BadSymlink { at }.into_error(op));
                    }
                    Some(())
                },
                b"CL" => le::u32_at(entry, 4).map(|child| rock.child = Some(child)),
                b"RE" => {
                    rock.relocated = true;
                    Some(())
                },
                b"TF" => times(&mut rock.times, entry),
                b"ZF" => zf(entry).map(|found| rock.zisofs = Some(found)),
                b"SF" => {
                    rock.sparse = true;
                    Some(())
                },
                // Padding, sharing, extensions' references, devices'
                // numbers, parent links, attributes, and what else other
                // extensions record: none is reported.
                _ => Some(()),
            };
            ok.ok_or_else(bad)?;
        }
        let Some((position, len)) = next else {
            break;
        };
        continuations = continuations.saturating_add(1);
        if continuations > MAX_CONTINUATIONS {
            return Err(bad());
        }
        area = read::read(source, position, len, op, bad).await?;
    }
    if rock.name.as_ref().is_some_and(|name| name.len() > MAX_NAME) {
        return Err(bad());
    }
    if link.any {
        rock.target = Some(link.target);
    }
    Ok(rock)
}

/// Splits the next entry off `rest`: `None` at the end, which a short
/// remainder, or an entry of length 0, as padding with zeros makes, marks;
/// or `Err` for an entry longer than what remains.
fn split<'a>(rest: &mut &'a [u8]) -> Result<Option<&'a [u8]>, ()> {
    let Some(&len) = rest.get(2).filter(|_| rest.len() >= 4) else {
        return Ok(None);
    };
    if len == 0 {
        return Ok(None);
    }
    let (entry, after) = rest.split_at_checked(usize::from(len)).ok_or(())?;
    if entry.len() < 4 {
        return Err(());
    }
    *rest = after;
    Ok(Some(entry))
}

/// Returns where a `CE` entry's continuation area is, and its length, at
/// most a block.
fn continuation(entry: &[u8], block: u32) -> Option<(u64, usize)> {
    let (extent, offset, len) = (
        le::u32_at(entry, 4)?,
        le::u32_at(entry, 12)?,
        le::u32_at(entry, 20)?,
    );
    if len > block {
        return None;
    }
    let position = u64::from(extent)
        .checked_mul(u64::from(block))?
        .checked_add(u64::from(offset))?;
    Some((position, usize::try_from(len).ok()?))
}

/// Adds an `NM` entry's part of the name, but for those naming `.` and
/// `..`, by their flags.
fn name(rock: &mut Rock, entry: &[u8]) -> Option<()> {
    let flags = *entry.get(4)?;
    if flags & 0x06 == 0 {
        let name = rock.name.get_or_insert_default();
        name.extend_from_slice(entry.get(5..)?);
    }
    Some(())
}

/// Reads a `TF` entry's times, each in the long or the short form, as its
/// flags say, in the order of their bits.
fn times(times: &mut Times, entry: &[u8]) -> Option<()> {
    let flags = *entry.get(4)?;
    let size = if flags & 0x80 != 0 { 17 } else { 7 };
    let mut at = 5_usize;
    for bit in 0..7_u8 {
        if flags & (1 << bit) == 0 {
            continue;
        }
        let end = at.checked_add(size)?;
        let bytes = entry.get(at..end)?;
        at = end;
        let time = if size == 17 {
            time::long(bytes)
        } else {
            time::short(bytes)
        };
        match bit {
            0 => times.created = time,
            1 => times.modified = time,
            2 => times.accessed = time,
            3 => times.changed = time,
            // Of backup, expiration, and effect.
            _ => {},
        }
    }
    Some(())
}

/// Reads a `ZF` entry.
fn zf(entry: &[u8]) -> Option<Zf> {
    Some(Zf {
        algorithm: [*entry.get(4)?, *entry.get(5)?],
        header: *entry.get(6)?,
        log: *entry.get(7)?,
        size: le::u32_at(entry, 8)?,
    })
}

/// A symlink's target, as `SL` entries record it: components, each with
/// flags, the last perhaps continued in the next.
#[derive(Default)]
struct Link {
    target: Vec<u8>,
    /// Whether the last component continues in the next.
    joined: bool,
    /// Whether any `SL` entry was found.
    any: bool,
}

impl Link {
    /// The component flag that it continues in the next.
    const CONTINUE: u8 = 0x01;
    /// The component flags of `.`, `..`, the root, and the volume's root.
    const CURRENT: u8 = 0x02;
    const PARENT: u8 = 0x04;
    const ROOT: u8 = 0x08 | 0x10;

    /// Adds the components of an `SL` entry, after its flags.
    fn add(&mut self, entry: &[u8]) -> Option<()> {
        self.any = true;
        let mut rest = entry.get(5..)?;
        while let [flags, len, tail @ ..] = rest {
            let (content, after) = tail.split_at_checked(usize::from(*len))?;
            rest = after;
            if !self.joined && !self.target.is_empty() && !self.target.ends_with(b"/") {
                self.target.push(b'/');
            }
            if flags & Self::ROOT != 0 {
                if !self.target.ends_with(b"/") {
                    self.target.push(b'/');
                }
            } else if flags & Self::CURRENT != 0 {
                self.target.push(b'.');
            } else if flags & Self::PARENT != 0 {
                self.target.extend_from_slice(b"..");
            } else {
                self.target.extend_from_slice(content);
            }
            self.joined = flags & Self::CONTINUE != 0;
        }
        // A byte left over is no component.
        rest.is_empty().then_some(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn entry(signature: &[u8; 2], data: &[u8]) -> Vec<u8> {
        let mut entry = vec![signature[0], signature[1], 4 + data.len() as u8, 1];
        entry.extend_from_slice(data);
        entry
    }

    fn rock(field: &[u8]) -> Result<Rock, Error> {
        filer_spec::block_on(read(&&[][..], 2048, field, 0, 0, OpKind::Open))
    }

    #[test]
    fn reads_rock_ridge_entries() {
        let mut field = entry(b"NM", b"\0long");
        field.extend(entry(b"NM", b"\0-name"));
        let mut px = 0o100644_u32.to_le_bytes().repeat(2);
        px.extend([0; 24]);
        px.extend(7_u32.to_le_bytes().repeat(2));
        field.extend(entry(b"PX", &px));
        field.extend(entry(
            b"TF",
            &[0x06, 124, 1, 2, 3, 4, 6, 0, 70, 1, 1, 0, 0, 0, 0],
        ));
        field.extend(entry(b"ZF", b"pz\x04\x0F\x10\0\0\0\0\0\x10\0"));
        let rock = rock(&field).unwrap();
        assert_eq!(rock.name.as_deref(), Some(&b"long-name"[..]));
        assert_eq!((rock.mode, rock.serial), (Some(0o100644), Some(7)));
        assert_eq!(rock.times.modified.unwrap().secs(), 1_704_164_646);
        assert_eq!(rock.times.accessed.unwrap().secs(), 0);
        assert_eq!(rock.times.created, None);
        let zf = rock.zisofs.unwrap();
        assert_eq!(
            (zf.algorithm, zf.header, zf.log, zf.size),
            (*b"pz", 4, 15, 16)
        );
    }

    #[test]
    fn reads_symlinks() {
        let target = |entries: &[&[u8]]| {
            let field: Vec<u8> = entries.iter().flat_map(|data| entry(b"SL", data)).collect();
            rock(&field).map(|rock| rock.target.unwrap())
        };
        assert_eq!(target(&[b"\0\0\x01a\0\x01b"]).unwrap(), b"a/b");
        assert_eq!(target(&[b"\0\x08\0\0\x03etc"]).unwrap(), b"/etc");
        assert_eq!(target(&[b"\0\x04\0\x02\0\0\x01x"]).unwrap(), b".././x");
        // A component continued in the next entry.
        assert_eq!(
            target(&[b"\x01\x01\x03lon", b"\0\0\x01g"]).unwrap(),
            b"long"
        );
        assert!(target(&[b"\0\0\x05ab"]).is_err());
    }

    #[test]
    fn ends_at_padding_and_refuses_overruns() {
        let mut field = entry(b"RE", &[]);
        field.extend([0; 6]);
        assert!(rock(&field).unwrap().relocated);
        let mut overrun = entry(b"NM", b"\0name");
        overrun[2] = 40;
        assert!(rock(&overrun).is_err());
        assert_eq!(sharing(&entry(b"SP", &[0xBE, 0xEF, 0])), Some(0));
        assert_eq!(sharing(&entry(b"SP", &[0xBE, 0xEE, 0])), None);
    }
}

// This is free and unencumbered software released into the public domain.

//! The manifest of a volume, one line per file or directory, in listing
//! order:
//!
//! ```text
//! path <TAB> kind (d, f, l; or p, s, c, b for FIFOs, sockets, and
//!      character and block devices) <TAB> size <TAB> modified <TAB> created
//!      <TAB> accessed <TAB> FNV-1a hash of the contents, or link target
//!      <TAB> flags <TAB> extended attributes
//! ```
//!
//! Directories have no size here. Times are whole seconds since the Unix
//! epoch. Flags are lowercase names, sorted and joined by `,`; extended
//! attributes are `name=hash` pairs, sorted and joined by `,`. `-` stands
//! for what does not apply, is unknown, or is empty, and `!error` for
//! contents that cannot be read. Shared by the `manifest` example, which
//! `tests/differential.sh` compares with what the Linux kernel's `ntfs3`
//! and ntfs-3g report, and by `tests/fixtures.rs`, which compares it with
//! what Windows reports.

use filer_api::{Dir, FileAttrs, FileKind, MetaMask, Metadata, ReadAt, XattrEntry};
use filer_ntfs::Ntfs;
use std::error::Error;

/// Returns the manifest of everything below `root`, one line per entry,
/// in listing order, each directory before its contents.
pub async fn manifest<S: ReadAt + 'static>(
    root: &Dir<Ntfs<S>>,
) -> Result<Vec<String>, Box<dyn Error>> {
    let mut lines = Vec::new();
    walk(root, "", &mut lines).await?;
    Ok(lines)
}

/// Adds the lines of everything below `dir`, recursively.
async fn walk<S: ReadAt + 'static>(
    root: &Dir<Ntfs<S>>,
    dir: &str,
    out: &mut Vec<String>,
) -> Result<(), Box<dyn Error>> {
    let handle = root.open_dir(dir).await?;
    let mut entries = handle.entries()?;
    while let Some(entry) = entries.next().await {
        let entry = entry?;
        let path = match dir {
            "" => entry.name.to_string(),
            dir => format!("{dir}/{}", entry.name),
        };
        let metadata = root
            .symlink_metadata(path.as_str(), MetaMask::all())
            .await?;
        let (kind, size, hash, xattrs) = match metadata.kind {
            Some(FileKind::Symlink) => {
                let target = root.read_link(path.as_str()).await?;
                let size = metadata
                    .len
                    .map_or_else(|| "-".to_owned(), |len| len.to_string());
                (
                    "l",
                    size,
                    String::from_utf8_lossy(target.as_bytes()).into_owned(),
                    "-".to_owned(),
                )
            },
            Some(FileKind::Dir) => {
                let size = metadata
                    .len
                    .map_or_else(|| "-".to_owned(), |len| len.to_string());
                let dir = root.open_dir(path.as_str()).await?;
                let names = dir.xattr_names().await?;
                let mut pairs = Vec::new();
                for name in names {
                    let value = dir.xattr(name.clone()).await?.unwrap_or_default();
                    pairs.push(pair(&name, &value));
                }
                ("d", size, "-".to_owned(), joined(pairs))
            },
            Some(
                kind @ (FileKind::Fifo
                | FileKind::Socket
                | FileKind::CharDevice
                | FileKind::BlockDevice),
            ) => {
                let letter = match kind {
                    FileKind::Fifo => "p",
                    FileKind::Socket => "s",
                    FileKind::CharDevice => "c",
                    _ => "b",
                };
                let size = metadata
                    .len
                    .map_or_else(|| "-".to_owned(), |len| len.to_string());
                (letter, size, "-".to_owned(), "-".to_owned())
            },
            _ => {
                let size = metadata
                    .len
                    .map_or_else(|| "-".to_owned(), |len| len.to_string());
                // A file whose contents cannot be read, such as a cloud
                // placeholder, is listed with `!error` for its hash.
                match read(root, &path).await {
                    Ok((contents, xattrs)) => {
                        ("f", size, format!("{:016x}", fnv1a(&contents)), xattrs)
                    },
                    Err(_) => ("f", size, "!error".to_owned(), "!error".to_owned()),
                }
            },
        };
        out.push(format!(
            "{path}\t{kind}\t{size}\t{}\t{hash}\t{}\t{xattrs}",
            times(&metadata),
            flags(&metadata),
        ));
        if kind == "d" {
            // Boxed, as an async function cannot recurse into itself
            // directly.
            Box::pin(walk(root, &path, out)).await?;
        }
    }
    Ok(())
}

/// Reads a file's contents and its extended attributes, as listed.
async fn read<S: ReadAt + 'static>(
    root: &Dir<Ntfs<S>>,
    path: &str,
) -> filer_api::Result<(Vec<u8>, String)> {
    let file = root.open_file(path).await?;
    let mut pairs = Vec::new();
    for name in file.xattr_names().await? {
        let value = file.xattr(name.clone()).await?.unwrap_or_default();
        pairs.push(pair(&name, &value));
    }
    Ok((file.read_to_end().await?, joined(pairs)))
}

/// Returns an attribute's `name=hash` pair.
fn pair(name: &XattrEntry, value: &[u8]) -> String {
    format!("{name}={:016x}", fnv1a(value))
}

/// Returns `items` sorted and joined by `,`, or `-` if there are none.
fn joined(mut items: Vec<String>) -> String {
    if items.is_empty() {
        return "-".to_owned();
    }
    items.sort();
    items.join(",")
}

/// Returns the names of the file's flags.
fn flags(metadata: &Metadata) -> String {
    let names = metadata
        .attrs
        .unwrap_or(FileAttrs::empty())
        .iter_names()
        .map(|(name, _)| name.to_lowercase())
        .collect();
    joined(names)
}

fn times(metadata: &Metadata) -> String {
    let secs = |time: Option<filer_api::Timestamp>| {
        time.map_or_else(|| "-".to_owned(), |time| time.secs().to_string())
    };
    format!(
        "{}\t{}\t{}",
        secs(metadata.modified),
        secs(metadata.created),
        secs(metadata.accessed)
    )
}

/// The 64-bit FNV-1a hash.
pub fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, &byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

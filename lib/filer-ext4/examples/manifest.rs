// This is free and unencumbered software released into the public domain.

//! Prints the tree of an ext2, ext3, or ext4 image, one line per entry, in
//! directory order:
//!
//! ```text
//! path <TAB> kind (d, f, l; or p, s, c, b for FIFOs, sockets, and
//!      character and block devices) <TAB> size <TAB> modified <TAB> changed
//!      <TAB> accessed <TAB> created <TAB> FNV-1a hash of the contents, or
//!      link target <TAB> flags <TAB> extended attributes
//! ```
//!
//! Times are seconds since the Unix epoch, with nine digits of
//! nanoseconds. Flags are lowercase names, sorted and joined by `,`;
//! extended attributes are `name=hash` pairs, sorted and joined by `,`.
//! `-` stands for what does not apply, is unknown, or is empty. A name
//! that is not UTF-8 is printed with each byte not part of a character as
//! `\xHH`; paths are UTF-8, so such an entry is only stated, not read.
//! `tests/differential.sh` compares this with what the Linux kernel
//! reports for the same image.
//!
//! The image is opened as a file of the native driver, and `Ext4` reads it
//! through that file: one driver layered over another.
//!
//! Usage: `cargo run -p filer-ext4 --example manifest -- IMAGE`.

use filer_ext4::Ext4;
use filer_system::{
    Dir, Driver, EntryName, ErrorKind, FileAttrs, FileKind, MetaMask, Metadata, Op, OpKind,
    Output, ReadAt, Resolve, Target, Timestamp, XattrEntry, ambient_authority,
};
use std::{
    error::Error,
    future::Future,
    io::{StdoutLock, Write},
    path::Path,
    process::ExitCode,
};

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let [image] = &args[..] else {
        eprintln!("usage: manifest IMAGE");
        return ExitCode::FAILURE;
    };
    match block_on(run(Path::new(image))) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("manifest: {error}");
            ExitCode::FAILURE
        },
    }
}

async fn run(image: &Path) -> Result<(), Box<dyn Error>> {
    let parent = image.parent().filter(|p| !p.as_os_str().is_empty());
    let parent = parent.unwrap_or(Path::new("."));
    let name = image
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    let dir = filer_system::open_ambient(parent, ambient_authority())?;
    let file = dir.open_file(name).await?;
    let root = Ext4::open(file).await?;
    walk(&root, "", &mut std::io::stdout().lock()).await
}

/// Prints everything below `dir`, recursively, each directory before its
/// contents.
async fn walk<S: ReadAt + 'static>(
    root: &Dir<Ext4<S>>,
    dir: &str,
    out: &mut StdoutLock<'_>,
) -> Result<(), Box<dyn Error>> {
    let handle = root.open_dir(dir).await?;
    let mut entries = handle.entries()?;
    while let Some(entry) = entries.next().await {
        let entry = entry?;
        let path = match dir {
            "" => entry.name.to_string(),
            dir => format!("{dir}/{}", entry.name),
        };
        if entry.name.is_foreign() {
            // Paths are UTF-8, so only the entry itself can be reached.
            let metadata = foreign_metadata(&handle, entry.name.clone()).await?;
            writeln!(
                out,
                "{path}\t{}\t{}\t{}\t-\t{}\t-",
                letter(metadata.kind),
                size(&metadata),
                times(&metadata),
                flags(&metadata),
            )?;
            continue;
        }
        let metadata = root
            .symlink_metadata(path.as_str(), MetaMask::all())
            .await?;
        let kind = letter(metadata.kind);
        let (hash, xattrs) = match kind {
            "l" => {
                let target = root.read_link(path.as_str()).await?;
                let target = String::from_utf8_lossy(target.as_bytes()).into_owned();
                (target, "-".to_owned())
            },
            "d" => {
                let child = root.open_dir(path.as_str()).await?;
                ("-".to_owned(), attributes(&child).await?)
            },
            "f" => {
                let file = root.open_file(path.as_str()).await?;
                let contents = file.read_to_end().await?;
                let xattrs = attributes(&file).await?;
                (format!("{:016x}", fnv1a(&contents)), xattrs)
            },
            _ => ("-".to_owned(), "-".to_owned()),
        };
        writeln!(
            out,
            "{path}\t{kind}\t{}\t{}\t{hash}\t{}\t{xattrs}",
            size(&metadata),
            times(&metadata),
            flags(&metadata),
        )?;
        if kind == "d" {
            // Boxed, as an async function cannot recurse into itself
            // directly.
            Box::pin(walk(root, &path, out)).await?;
        }
    }
    Ok(())
}

/// Reads the metadata of the entry `name` of `dir`, which has no path.
async fn foreign_metadata<S: ReadAt + 'static>(
    dir: &Dir<Ext4<S>>,
    name: EntryName,
) -> filer_system::Result<Metadata> {
    let op = Op::Stat {
        target: Target::Entry {
            at: dir.handle().clone(),
            name,
            follow: false,
            resolve: Resolve::default(),
        },
        mask: MetaMask::all(),
    };
    match dir.driver().submit(op).await.result? {
        Output::Metadata(metadata) => Ok(metadata),
        _ => Err(filer_system::Error::new(ErrorKind::Other, OpKind::Stat)),
    }
}

/// Returns the letter of a kind.
fn letter(kind: Option<FileKind>) -> &'static str {
    match kind {
        Some(FileKind::Dir) => "d",
        Some(FileKind::Symlink) => "l",
        Some(FileKind::Fifo) => "p",
        Some(FileKind::Socket) => "s",
        Some(FileKind::CharDevice) => "c",
        Some(FileKind::BlockDevice) => "b",
        _ => "f",
    }
}

fn size(metadata: &Metadata) -> String {
    metadata
        .len
        .map_or_else(|| "-".to_owned(), |len| len.to_string())
}

/// Something with extended attributes: a file or a directory.
trait Attributed {
    fn names(&self) -> impl Future<Output = filer_system::Result<Vec<XattrEntry>>>;
    fn value(&self, name: XattrEntry) -> impl Future<Output = filer_system::Result<Vec<u8>>>;
}

impl<S: ReadAt + 'static> Attributed for Dir<Ext4<S>> {
    fn names(&self) -> impl Future<Output = filer_system::Result<Vec<XattrEntry>>> {
        self.xattr_names()
    }

    async fn value(&self, name: XattrEntry) -> filer_system::Result<Vec<u8>> {
        Ok(self.xattr(name).await?.unwrap_or_default())
    }
}

impl<S: ReadAt + 'static> Attributed for filer_system::File<filer_system::ReadOnly, Ext4<S>> {
    fn names(&self) -> impl Future<Output = filer_system::Result<Vec<XattrEntry>>> {
        self.xattr_names()
    }

    async fn value(&self, name: XattrEntry) -> filer_system::Result<Vec<u8>> {
        Ok(self.xattr(name).await?.unwrap_or_default())
    }
}

/// Returns the extended attributes of `item` as sorted `name=hash` pairs,
/// or `-` if it has none, or the driver does not read them.
async fn attributes(item: &impl Attributed) -> filer_system::Result<String> {
    let names = match item.names().await {
        Ok(names) => names,
        Err(error) if error.kind() == ErrorKind::Unsupported => return Ok("-".to_owned()),
        Err(error) => return Err(error),
    };
    let mut pairs = Vec::new();
    for name in names {
        let value = item.value(name.clone()).await?;
        pairs.push(format!("{name}={:016x}", fnv1a(&value)));
    }
    Ok(joined(pairs))
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

/// Returns the modification, change, access, and creation times.
fn times(metadata: &Metadata) -> String {
    let time = |time: Option<Timestamp>| {
        time.map_or_else(
            || "-".to_owned(),
            |time| format!("{}.{:09}", time.secs(), time.nanos()),
        )
    };
    format!(
        "{}\t{}\t{}\t{}",
        time(metadata.modified),
        time(metadata.changed),
        time(metadata.accessed),
        time(metadata.created)
    )
}

/// The 64-bit FNV-1a hash.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, &byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// Polls `future` to completion on this thread: the native driver used
/// here blocks, so there is never anything to wait for.
fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    loop {
        if let std::task::Poll::Ready(output) = future.as_mut().poll(&mut cx) {
            return output;
        }
    }
}

// This is free and unencumbered software released into the public domain.

//! Prints the tree of a SquashFS image, one line per entry, in directory
//! order:
//!
//! ```text
//! path <TAB> kind (d, f, l; or p, s, c, b for FIFOs, sockets, and
//!      character and block devices) <TAB> size <TAB> modified <TAB>
//!      FNV-1a hash of the contents, or link target <TAB> extended
//!      attributes
//! ```
//!
//! The time is seconds since the Unix epoch, with nine digits of
//! nanoseconds, the only time SquashFS records. Extended attributes are
//! `name=hash` pairs, sorted and joined by `,`. `-` stands for what does
//! not apply or is empty. A name that is not UTF-8 is printed with each
//! byte not part of a character as `\xHH`; paths are UTF-8, so such an
//! entry is only stated, not read. `tests/differential.sh` compares this
//! with what the Linux kernel reports for the same image.
//!
//! The image is opened as a file of the native driver, and `Squashfs`
//! reads it through that file: one driver layered over another.
//!
//! Usage: `cargo run -p filer-squashfs --example squashfs-manifest --
//! IMAGE`.

use filer_squashfs::Squashfs;
use filer_system::{
    Dir, Driver, EntryName, ErrorKind, FileKind, MetaMask, Metadata, Op, OpKind, Output, ReadAt,
    Resolve, Target, XattrEntry, ambient_authority,
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
        eprintln!("usage: squashfs-manifest IMAGE");
        return ExitCode::FAILURE;
    };
    match block_on(run(Path::new(image))) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("squashfs-manifest: {error}");
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
    let root = Squashfs::open(file).await?;
    walk(&root, "", &mut std::io::stdout().lock()).await
}

/// Prints everything below `dir`, recursively, each directory before its
/// contents.
async fn walk<S: ReadAt + 'static>(
    root: &Dir<Squashfs<S>>,
    dir: &str,
    out: &mut StdoutLock<'_>,
) -> Result<(), Box<dyn Error>> {
    let handle = root.open_dir(dir).await?;
    for name in handle.names().await? {
        let path = match dir {
            "" => name.to_string(),
            dir => format!("{dir}/{}", name),
        };
        if name.is_foreign() {
            // Paths are UTF-8, so only the entry itself can be reached.
            let metadata = foreign_metadata(&handle, name.clone()).await?;
            writeln!(
                out,
                "{path}\t{}\t{}\t{}\t-\t-",
                letter(metadata.kind),
                size(&metadata),
                time(&metadata),
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
            "{path}\t{kind}\t{}\t{}\t{hash}\t{xattrs}",
            size(&metadata),
            time(&metadata),
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
    dir: &Dir<Squashfs<S>>,
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

impl<S: ReadAt + 'static> Attributed for Dir<Squashfs<S>> {
    fn names(&self) -> impl Future<Output = filer_system::Result<Vec<XattrEntry>>> {
        self.xattr_names()
    }

    async fn value(&self, name: XattrEntry) -> filer_system::Result<Vec<u8>> {
        Ok(self.xattr(name).await?.unwrap_or_default())
    }
}

impl<S: ReadAt + 'static> Attributed for filer_system::File<filer_system::ReadOnly, Squashfs<S>> {
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

/// Returns the modification time.
fn time(metadata: &Metadata) -> String {
    metadata.modified.map_or_else(
        || "-".to_owned(),
        |time| format!("{}.{:09}", time.secs(), time.nanos()),
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

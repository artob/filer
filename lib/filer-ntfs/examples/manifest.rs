// This is free and unencumbered software released into the public domain.

//! Prints the tree of an NTFS image, one line per file or directory, in
//! directory order:
//!
//! ```text
//! path <TAB> kind (d, f, or l) <TAB> size <TAB> modified <TAB> created
//!      <TAB> accessed <TAB> FNV-1a hash of the contents, or link target
//! ```
//!
//! Directories have no size here. Times are whole seconds
//! since the Unix epoch; `-` stands for what does not apply or is
//! unknown. `tests/differential.sh` compares this with
//! what the Linux kernel's `ntfs3` driver reports for the same image.
//!
//! The image is opened as a file of the native driver, and `Ntfs` reads it
//! through that file: one driver layered over another.
//!
//! Usage: `cargo run -p filer-ntfs --example manifest -- [--allow-dirty]
//! IMAGE`, where `--allow-dirty` opens a volume marked dirty.

use filer_ntfs::{Ntfs, Options};
use filer_system::{Dir, FileKind, MetaMask, Metadata, ReadAt, ambient_authority};
use std::{
    error::Error,
    future::Future,
    io::{StdoutLock, Write},
    path::Path,
    process::ExitCode,
};

fn main() -> ExitCode {
    let mut args: Vec<_> = std::env::args_os().skip(1).collect();
    let mut options = Options::new();
    if args.first().is_some_and(|arg| arg == "--allow-dirty") {
        options = options.allow_dirty(true);
        args.remove(0);
    }
    let [image] = &args[..] else {
        eprintln!("usage: manifest [--allow-dirty] IMAGE");
        return ExitCode::FAILURE;
    };
    match block_on(run(Path::new(image), options)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("manifest: {error}");
            ExitCode::FAILURE
        },
    }
}

async fn run(image: &Path, options: Options) -> Result<(), Box<dyn Error>> {
    let parent = image.parent().filter(|p| !p.as_os_str().is_empty());
    let parent = parent.unwrap_or(Path::new("."));
    let name = image
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    let dir = filer_system::open_ambient(parent, ambient_authority())?;
    let file = dir.open_file(name).await?;
    let root = Ntfs::open_with(file, options).await?;
    walk(&root, "", &mut std::io::stdout().lock()).await
}

/// Prints everything below `dir`, recursively, each directory before its
/// contents.
async fn walk<S: ReadAt + 'static>(
    root: &Dir<Ntfs<S>>,
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
        let metadata = root
            .symlink_metadata(path.as_str(), MetaMask::all())
            .await?;
        let (kind, size, hash) = match metadata.kind {
            Some(FileKind::Symlink) => {
                let target = root.read_link(path.as_str()).await?;
                let size = metadata
                    .len
                    .map_or_else(|| "-".to_owned(), |len| len.to_string());
                (
                    "l",
                    size,
                    String::from_utf8_lossy(target.as_bytes()).into_owned(),
                )
            },
            Some(FileKind::Dir) => {
                let size = metadata
                    .len
                    .map_or_else(|| "-".to_owned(), |len| len.to_string());
                ("d", size, "-".to_owned())
            },
            _ => {
                let size = metadata
                    .len
                    .map_or_else(|| "-".to_owned(), |len| len.to_string());
                // A file whose contents cannot be read, such as a cloud
                // placeholder, is listed with `!error` for its hash.
                match read(root, &path).await {
                    Ok(contents) => ("f", size, format!("{:016x}", fnv1a(&contents))),
                    Err(_) => ("f", size, "!error".to_owned()),
                }
            },
        };
        writeln!(out, "{path}\t{kind}\t{size}\t{}\t{hash}", times(&metadata))?;
        if kind == "d" {
            // Boxed, as an async function cannot recurse into itself
            // directly.
            Box::pin(walk(root, &path, out)).await?;
        }
    }
    Ok(())
}

async fn read<S: ReadAt + 'static>(
    root: &Dir<Ntfs<S>>,
    path: &str,
) -> filer_system::Result<Vec<u8>> {
    root.open_file(path).await?.read_to_end().await
}

fn times(metadata: &Metadata) -> String {
    let secs = |time: Option<filer_system::Timestamp>| {
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

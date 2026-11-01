// This is free and unencumbered software released into the public domain.

//! Prints the tree of a FAT image, one line per file or directory, in
//! directory order:
//!
//! ```text
//! path <TAB> kind (d or f) <TAB> size <TAB> modified <TAB> created
//!      <TAB> accessed <TAB> FNV-1a hash of the contents
//! ```
//!
//! Times are whole seconds since the Unix epoch; `-` stands for what does
//! not apply or is unknown. `tests/differential.sh` compares this with
//! what the Linux kernel's FAT driver reports for the same image.
//!
//! The image is opened as a file of the native driver, and `Fat` reads it
//! through that file: one driver layered over another.
//!
//! Usage: `cargo run -p filer-fat --example manifest -- IMAGE`

use filer_fat::Fat;
use filer_system::{Dir, FileKind, MetaMask, Metadata, ReadAt, ambient_authority};
use std::{
    error::Error,
    future::Future,
    io::{StdoutLock, Write},
    path::Path,
    process::ExitCode,
};

fn main() -> ExitCode {
    let Some(image) = std::env::args_os().nth(1) else {
        eprintln!("usage: manifest IMAGE");
        return ExitCode::FAILURE;
    };
    match block_on(run(Path::new(&image))) {
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
    let root = Fat::open(file).await?;
    walk(&root, "", &mut std::io::stdout().lock()).await
}

/// Prints everything below `dir`, recursively, each directory before its
/// contents.
async fn walk<S: ReadAt + 'static>(
    root: &Dir<Fat<S>>,
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
        let metadata = root.metadata(path.as_str(), MetaMask::all()).await?;
        let (kind, size, hash) = match metadata.kind {
            Some(FileKind::Dir) => ("d", "-".to_owned(), "-".to_owned()),
            _ => {
                let contents = root.open_file(path.as_str()).await?.read_to_end().await?;
                let size = contents.len().to_string();
                ("f", size, format!("{:016x}", fnv1a(&contents)))
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

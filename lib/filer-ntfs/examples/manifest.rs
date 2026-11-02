// This is free and unencumbered software released into the public domain.

//! Prints the manifest of an NTFS image, one line per file or directory,
//! in directory order, as `tests/common/manifest.rs` describes.
//! `tests/differential.sh` compares it with what the Linux kernel's
//! `ntfs3` driver and ntfs-3g report for the same image.
//!
//! The image is opened as a file of the native driver, and `Ntfs` reads it
//! through that file: one driver layered over another.
//!
//! Usage: `cargo run -p filer-ntfs --example manifest -- [--allow-dirty]
//! IMAGE`, where `--allow-dirty` opens a volume marked dirty.

#[path = "../tests/common/manifest.rs"]
mod manifest;

use filer_ntfs::{Ntfs, Options};
use filer_system::ambient_authority;
use std::{error::Error, future::Future, io::Write, path::Path, process::ExitCode};

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
    let mut out = std::io::stdout().lock();
    for line in manifest::manifest(&root).await? {
        writeln!(out, "{line}")?;
    }
    Ok(())
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

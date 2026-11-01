// This is free and unencumbered software released into the public domain.

//! The `filer-api` frontend over a heap.

use core::{
    future::Future,
    task::{Context, Poll, Waker},
};
use filer_api::{ErrorKind, FileKind, MetaMask, Rights, Symlinks, rel};
use filer_heap::Heap;

fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = core::pin::pin!(future);
    let mut cx = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(output) = future.as_mut().poll(&mut cx) {
            return output;
        }
    }
}

fn heap() -> Heap {
    Heap::builder()
        .file("config/app.toml", "debug = true")
        .symlink("config/current", "app.toml")
        .symlink("config/root", "/config/app.toml")
        .dir("empty")
        .build()
}

#[test]
fn opens_nested_directories_and_files() {
    let root = heap().root_dir();
    let config = block_on(root.open_dir("config")).unwrap();
    let file = block_on(config.open_file("app.toml")).unwrap();
    assert_eq!(block_on(file.read_to_end()).unwrap(), b"debug = true");
    let again = block_on(root.open_file(rel!("config/current"))).unwrap();
    assert_eq!(block_on(again.read_to_end()).unwrap(), b"debug = true");
}

#[test]
fn stats_with_and_without_following() {
    let root = heap().root_dir();
    let link = block_on(root.symlink_metadata("config/current", MetaMask::all())).unwrap();
    assert_eq!(link.kind, Some(FileKind::Symlink));
    let file = block_on(root.metadata("config/current", MetaMask::all())).unwrap();
    assert_eq!((file.kind, file.len), (Some(FileKind::File), Some(12)));
    let dir = block_on(root.metadata("empty", MetaMask::KIND)).unwrap();
    assert_eq!(dir.kind, Some(FileKind::Dir));
}

#[test]
fn follows_with_the_directory_policy() {
    let root = heap().root_dir();
    let denying = root.clone().with_resolve(Symlinks::Deny);
    // `Deny` applies to stats of single entries too, not just to opens.
    let error = block_on(denying.metadata("config/current", MetaMask::KIND)).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::SymlinkLoop);
    let error = block_on(denying.open_file("config/current")).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::SymlinkLoop);

    let escape = block_on(root.open_file("config/root")).unwrap_err();
    assert_eq!(escape.kind(), ErrorKind::EscapeBlocked);
    let chrooted = root.clone().with_resolve(Symlinks::InRoot);
    let file = block_on(chrooted.open_file("config/root")).unwrap();
    assert_eq!(block_on(file.read_to_end()).unwrap(), b"debug = true");
}

#[test]
fn reads_links() {
    let root = heap().root_dir();
    let target = block_on(root.read_link("config/root")).unwrap();
    assert_eq!(target.as_str(), Some("/config/app.toml"));
    let error = block_on(root.read_link("config/app.toml")).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::NotASymlink);
    let error = block_on(root.read_link("")).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::InvalidName);
}

#[test]
fn inherits_rights_and_labels() {
    let root = heap().root_dir().with_label("<heap>");
    let blind = root.restrict(Rights::all() - Rights::READ);
    let config = block_on(blind.open_dir("config")).unwrap();
    assert_eq!(config.rights(), blind.rights());
    let error = block_on(config.open_file("app.toml")).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::RightsDenied);
    assert_eq!(error.label(), Some("<heap>"));
    let error = block_on(root.open_file("config/nope")).unwrap_err();
    assert_eq!(
        error.to_string(),
        "open \"config/nope\" in <heap>: not found"
    );
}

#[test]
fn erases_the_driver_type() {
    let root = heap().root_dir().into_dyn();
    let file = block_on(root.open_file("config/app.toml")).unwrap();
    assert_eq!(block_on(file.read_to_end()).unwrap(), b"debug = true");
}

#[test]
fn lists_directories_across_batches() {
    let mut builder = Heap::builder();
    for i in 0..600 {
        builder = builder.file(format!("many/f{i:03}"), "");
    }
    let root = builder.build().root_dir();
    let many = block_on(root.open_dir("many")).unwrap();
    let mut entries = many.entries().unwrap();
    let mut names = Vec::new();
    while let Some(entry) = block_on(entries.next()) {
        let entry = entry.unwrap();
        assert_eq!(entry.kind, Some(FileKind::File));
        names.push(entry.name.to_string());
    }
    let expected: Vec<String> = (0..600).map(|i| format!("f{i:03}")).collect();
    assert_eq!(names, expected);
    assert!(block_on(entries.next()).is_none());
}

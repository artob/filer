// This is free and unencumbered software released into the public domain.

//! Hostile, malformed, and unusual archives.

mod common;

use common::{Entry, FILE, Writer};
use filer_api::{Dir, Error, ErrorKind, FileKind, MetaMask};
use filer_spec::block_on;
use filer_zip::{Limits, Zip};
use std::time::Duration;

type Root = Dir<Zip<Vec<u8>>>;

fn open(bytes: Vec<u8>) -> Result<Root, Error> {
    block_on(Zip::open(bytes))
}

fn read(root: &Root, path: &str) -> Vec<u8> {
    let file = block_on(root.open_file(path)).unwrap();
    block_on(file.read_to_end()).unwrap()
}

fn kind(result: Result<Root, Error>) -> ErrorKind {
    result.unwrap_err().kind()
}

fn reason(result: Result<Root, Error>) -> String {
    let error = result.unwrap_err();
    core::error::Error::source(&error).unwrap().to_string()
}

#[test]
fn reads_an_info_zip_archive() {
    let archive = include_bytes!("data/hello.zip").to_vec();
    let root = open(archive).unwrap();
    assert_eq!(read(&root, "hello/world.txt"), b"Hello, world!\n");
    assert_eq!(read(&root, "hello/link"), b"Hello, world!\n");
    let link = block_on(root.read_link("hello/link")).unwrap();
    assert_eq!(link.as_bytes(), b"world.txt");
    // The extended timestamp, not the DOS fields.
    let modified = block_on(root.metadata("hello/world.txt", MetaMask::MTIME))
        .unwrap()
        .modified
        .unwrap();
    assert_eq!(modified.secs(), 1_699_968_140);
    assert_eq!(modified.granularity(), Some(Duration::from_secs(1)));
}

#[test]
fn refuses_names_that_could_escape() {
    for name in [
        &b"../evil"[..],
        b"a/../../evil",
        b"/etc/passwd",
        b"a\0b",
        b"..\\evil",
        b"C:/Windows/evil",
        b"c:evil",
    ] {
        let archive = Writer::default().file(name, b"x").finish();
        assert_eq!(kind(open(archive)), ErrorKind::InvalidName, "{name:?}");
    }
}

#[test]
fn confines_archive_symlinks() {
    let archive = Writer::default()
        .file(b"inside", b"inside")
        .symlink(b"up", b"../../etc/passwd")
        .symlink(b"abs", b"/etc/passwd")
        .symlink(b"ok", b"inside")
        .finish();
    let root = open(archive).unwrap();
    assert_eq!(read(&root, "ok"), b"inside");
    for path in ["up", "abs"] {
        let error = block_on(root.open_file(path)).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::EscapeBlocked, "{path}");
    }
}

#[test]
fn refuses_disagreeing_headers() {
    let mut writer = Writer::default();
    writer.file(b"benign", b"x").last().local_name = Some(b"evil".to_vec());
    assert_eq!(
        reason(open(writer.finish())),
        "zip header at offset 0: local and central headers disagree"
    );

    let mut writer = Writer::default();
    writer.file(b"a", b"x").last().local_crc = Some(0);
    assert_eq!(kind(open(writer.finish())), ErrorKind::Corrupt);

    // Data descriptors defer the local sizes and CRC, which may be zero.
    let mut writer = Writer::default();
    writer.file(b"a", b"described").last().data_descriptor = true;
    assert_eq!(read(&open(writer.finish()).unwrap(), "a"), b"described");
}

#[test]
fn refuses_malformed_archives() {
    assert_eq!(kind(open(Vec::new())), ErrorKind::Corrupt);
    assert_eq!(
        kind(open(b"not a zip archive".to_vec())),
        ErrorKind::Corrupt
    );

    let archive = Writer::default().file(b"a", b"x").finish();
    // Cut into the central directory, and the end record no longer fits.
    let mut short = archive.clone();
    short.drain(40..50);
    assert_eq!(kind(open(short)), ErrorKind::Corrupt);
    // A central directory that claims more members than it holds.
    let mut more = archive.clone();
    let n = more.len();
    more[n - 12] = 2;
    more[n - 14] = 2;
    assert_eq!(kind(open(more)), ErrorKind::Corrupt);
    // A local header offset past the central directory.
    let mut far = archive.clone();
    let cd = far.len() - 22 - 47;
    far[cd + 42] = 0x40;
    assert_eq!(kind(open(far)), ErrorKind::Corrupt);

    // An end record hidden in the comment of another is ambiguous.
    let inner = Writer::default().file(b"hidden", b"x").finish();
    let outer = Writer::default()
        .file(b"shown", b"x")
        .comment(&inner[inner.len() - 22..])
        .finish();
    assert_eq!(kind(open(outer)), ErrorKind::Corrupt);
}

#[test]
fn refuses_what_it_cannot_read() {
    let mut writer = Writer::default();
    writer.file(b"deflated", b"x").last().method = 8;
    assert_eq!(kind(open(writer.finish())), ErrorKind::Unsupported);

    let mut writer = Writer::default();
    writer.file(b"secret", b"x").last().flags = 1;
    assert_eq!(kind(open(writer.finish())), ErrorKind::Unsupported);

    let mut spanned = Writer::default().file(b"a", b"x").finish();
    let n = spanned.len();
    spanned[n - 18] = 1;
    assert_eq!(kind(open(spanned)), ErrorKind::Unsupported);
}

#[test]
fn enforces_limits() {
    let three = Writer::default()
        .file(b"a", b"")
        .file(b"b", b"")
        .file(b"c", b"")
        .finish();
    let limits = Limits::default().with_max_entries(2);
    let error = block_on(Zip::open_with(three, limits)).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::LimitExceeded);

    let long = Writer::default().file(&[b'a'; 5000], b"").finish();
    assert_eq!(kind(open(long)), ErrorKind::LimitExceeded);

    let target = Writer::default().symlink(b"l", &[b'a'; 5000]).finish();
    assert_eq!(kind(open(target)), ErrorKind::LimitExceeded);
}

#[test]
fn lets_later_duplicates_win() {
    let archive = Writer::default()
        .file(b"a", b"first")
        .file(b"a", b"second")
        .file(b"d", b"a file")
        .file(b"d/f", b"now a directory")
        .dir(b"e/")
        .file(b"e", b"now a file")
        .finish();
    let root = open(archive).unwrap();
    assert_eq!(read(&root, "a"), b"second");
    assert_eq!(read(&root, "d/f"), b"now a directory");
    assert_eq!(read(&root, "e"), b"now a file");
}

#[test]
fn reads_dos_hosts_and_times() {
    let archive = Writer::default()
        .entry(Entry::new(b"dir/", None, b""))
        .entry(Entry::new(b"implied/file", None, b"dos"))
        .finish();
    let root = open(archive).unwrap();
    let kind = |path| block_on(root.metadata(path, MetaMask::KIND)).unwrap().kind;
    assert_eq!(kind("dir"), Some(FileKind::Dir));
    assert_eq!(kind("implied"), Some(FileKind::Dir));
    assert_eq!(read(&root, "implied/file"), b"dos");
    let modified = block_on(root.metadata("implied/file", MetaMask::MTIME))
        .unwrap()
        .modified
        .unwrap();
    assert_eq!(modified.secs(), 1_704_110_400);
    assert_eq!(modified.granularity(), Some(Duration::from_secs(2)));
}

#[test]
fn accepts_comments_and_zip64() {
    let mut writer = Writer::default();
    writer.file(b"a", b"x").comment(b"a comment").zip64();
    assert_eq!(read(&open(writer.finish()).unwrap(), "a"), b"x");
}

#[test]
fn reads_members_in_pieces_without_overrunning_them() {
    let archive = Writer::default()
        .file(b"a", b"0123456789")
        .file(b"b", b"other")
        .finish();
    let root = open(archive).unwrap();
    let file = block_on(root.open_file("a")).unwrap();
    let filer_api::BufResult(n, buf) = block_on(file.read_at(Vec::with_capacity(100), 6));
    assert_eq!((n.unwrap(), buf.as_slice()), (4, &b"6789"[..]));
    let filer_api::BufResult(n, _) = block_on(file.read_at(Vec::with_capacity(100), 10));
    assert_eq!(n.unwrap(), 0);
}

#[test]
fn reads_large_central_directories() {
    // More than one chunk of central directory.
    let mut writer = Writer::default();
    let names: Vec<String> = (0..3000).map(|i| format!("dir/{i:05}")).collect();
    for name in &names {
        writer.entry(Entry::new(name.as_bytes(), Some(FILE), name.as_bytes()));
    }
    let root = open(writer.finish()).unwrap();
    assert_eq!(read(&root, "dir/02999"), b"dir/02999");
}

#[test]
fn reads_archives_stored_in_another_driver() {
    let archive = Writer::default().file(b"inner.txt", b"nested").finish();
    let heap = filer_heap::Heap::builder()
        .file("archives/inner.zip", archive)
        .build();
    let file = block_on(heap.root_dir().open_file("archives/inner.zip")).unwrap();
    // The `File` of one driver is the source of another.
    let root = block_on(Zip::open(file)).unwrap();
    let inner = block_on(root.open_file("inner.txt")).unwrap();
    assert_eq!(block_on(inner.read_to_end()).unwrap(), b"nested");
}

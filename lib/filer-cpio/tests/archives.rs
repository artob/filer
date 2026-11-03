// This is free and unencumbered software released into the public domain.

//! Archives that real tools wrote, and hostile, malformed, and unusual
//! ones.
//!
//! `tests/data` holds what `make.sh` there wrote with GNU cpio in each of
//! its formats (`gnu-*.cpio`), and an initramfs image of two archives
//! concatenated; `bsd-*.cpio` come from macOS's `bsdcpio`, from the same
//! tree.

mod common;

use common::{FILE, FORMATS, Format, Writer};
use filer_api::{Dir, EntryName, Error, ErrorKind, FileKind, MetaMask};
use filer_cpio::{Cpio, Limits};
use filer_spec::block_on;

type Root = Dir<Cpio<Vec<u8>>>;

fn open(bytes: Vec<u8>) -> Result<Root, Error> {
    block_on(Cpio::open(bytes))
}

fn read(root: &Root, path: &str) -> Result<Vec<u8>, Error> {
    let file = block_on(root.open_file(path))?;
    block_on(file.read_to_end())
}

fn list(root: &Root, path: &str) -> Vec<EntryName> {
    let dir = block_on(root.open_dir(path)).unwrap();
    let mut names = block_on(dir.names()).unwrap();
    names.sort();
    names
}

fn stat(root: &Root, path: &str) -> filer_api::Metadata {
    block_on(root.symlink_metadata(path, MetaMask::all())).unwrap()
}

fn error(result: Result<Root, Error>) -> (ErrorKind, String) {
    let error = result.unwrap_err();
    let source = core::error::Error::source(&error).map(ToString::to_string);
    (error.kind(), source.unwrap_or_default())
}

fn kind(result: Result<Root, Error>) -> ErrorKind {
    error(result).0
}

fn at(offset: u64, reason: &str) -> String {
    format!("cpio header at offset {offset}: {reason}")
}

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!("{}/tests/data/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

#[test]
fn reads_what_cpio_tools_write() {
    for archive in [
        "gnu-newc.cpio",
        "gnu-crc.cpio",
        "gnu-odc.cpio",
        "gnu-bin.cpio",
        "bsd-newc.cpio",
        "bsd-odc.cpio",
    ] {
        let root = open(fixture(archive)).unwrap();
        assert_eq!(
            list(&root, ""),
            ["dir", "empty", "fifo", "hard1", "link"],
            "{archive}"
        );
        assert_eq!(list(&root, "dir"), ["hard2", "hello.txt"], "{archive}");
        assert!(list(&root, "empty").is_empty());
        assert_eq!(read(&root, "dir/hello.txt").unwrap(), b"Hello, world!\n");
        assert_eq!(read(&root, "link").unwrap(), b"Hello, world!\n");
        assert_eq!(stat(&root, "link").kind, Some(FileKind::Symlink));
        assert_eq!(stat(&root, "fifo").kind, Some(FileKind::Fifo));
        // Whichever link holds the data, both read it, as one file.
        assert_eq!(read(&root, "hard1").unwrap(), b"shared\n", "{archive}");
        assert_eq!(read(&root, "dir/hard2").unwrap(), b"shared\n", "{archive}");
        assert_eq!(stat(&root, "hard1").id, stat(&root, "dir/hard2").id);
        assert_ne!(stat(&root, "hard1").id, stat(&root, "dir/hello.txt").id);
        if archive.starts_with("gnu") {
            let modified = stat(&root, "dir/hello.txt").modified.unwrap();
            assert_eq!(modified.secs(), 1_700_000_000, "{archive}");
        }
    }
}

#[test]
fn reads_initramfs_images() {
    let root = open(fixture("initramfs.img")).unwrap();
    assert_eq!(
        list(&root, ""),
        ["dir", "empty", "fifo", "hard1", "kernel", "link"]
    );
    assert_eq!(
        read(&root, "kernel/x86/microcode/GenuineIntel.bin").unwrap(),
        b"not really microcode\n"
    );
    assert_eq!(read(&root, "hard1").unwrap(), b"shared\n");
}

#[test]
fn reads_concatenated_archives() {
    // The same inode in two archives is two files.
    let first = Writer::new(Format::Newc)
        .entry(b"a", FILE, 7, 2, b"")
        .entry(b"b", FILE, 7, 2, b"first")
        .finish();
    let mut second = Writer::new(Format::Odc)
        .entry(b"c", FILE, 7, 2, b"second")
        .entry(b"d", FILE, 7, 2, b"second")
        .finish();
    // Padding between archives may be any number of zeros.
    second.splice(0..0, [0; 3]);
    let root = open([first.clone(), second].concat()).unwrap();
    assert_eq!(read(&root, "a").unwrap(), b"first");
    assert_eq!(read(&root, "c").unwrap(), b"second");
    assert_eq!(stat(&root, "a").id, stat(&root, "b").id);
    assert_eq!(stat(&root, "c").id, stat(&root, "d").id);
    assert_ne!(stat(&root, "a").id, stat(&root, "c").id);

    // As Linux distributions build them: an archive, then a compressed one.
    let zstd = [&first[..], &[0x28, 0xB5, 0x2F, 0xFD, 0x84, 0]].concat();
    let offset = first.len() as u64;
    assert_eq!(
        error(open(zstd)),
        (ErrorKind::Unsupported, at(offset, "compressed archive"))
    );
    let gzip = vec![0x1F, 0x8B, 8, 0];
    assert_eq!(kind(open(gzip)), ErrorKind::Unsupported);
    let garbage = [&first[..], b"garbage"].concat();
    assert_eq!(
        error(open(garbage)),
        (ErrorKind::Corrupt, at(offset, "not a cpio archive"))
    );
}

#[test]
fn shares_nodes_between_hardlinks() {
    for format in FORMATS {
        let archive = Writer::new(format)
            .hardlinks(&[b"one", b"sub/two", b"three"], b"linked")
            .finish();
        let root = open(archive).unwrap();
        for path in ["one", "sub/two", "three"] {
            assert_eq!(read(&root, path).unwrap(), b"linked", "{format:?} {path}");
        }
        assert_eq!(stat(&root, "one").id, stat(&root, "three").id);
    }
    // A link holds even if a later member takes its first name.
    let archive = Writer::new(Format::Newc)
        .entry(b"first", FILE, 9, 2, b"")
        .file(b"first", b"replaced")
        .entry(b"second", FILE, 9, 2, b"linked")
        .finish();
    let root = open(archive).unwrap();
    assert_eq!(read(&root, "first").unwrap(), b"replaced");
    assert_eq!(read(&root, "second").unwrap(), b"linked");
    // Without a link count, the same inode is no link.
    let archive = Writer::new(Format::Newc)
        .entry(b"a", FILE, 9, 1, b"a")
        .entry(b"b", FILE, 9, 1, b"b")
        .finish();
    let root = open(archive).unwrap();
    assert_eq!(read(&root, "a").unwrap(), b"a");
    assert_eq!(read(&root, "b").unwrap(), b"b");
}

#[test]
fn checks_checksums() {
    let mut archive = Writer::new(Format::Crc)
        .file(b"file", b"0123456789")
        .symlink(b"link", b"file")
        .finish();
    let good = open(archive.clone()).unwrap();
    assert_eq!(read(&good, "file").unwrap(), b"0123456789");
    // The data starts after the 110-byte header and the padded name.
    archive[116] = b'X';
    let root = open(archive.clone()).unwrap();
    assert_eq!(read(&root, "file").unwrap_err().kind(), ErrorKind::Corrupt);
    // A read that stops short of the end is not checked.
    let file = block_on(root.open_file("file")).unwrap();
    let filer_api::BufResult(n, buf) = block_on(file.read_at(Vec::with_capacity(3), 0));
    assert_eq!((n.unwrap(), buf.as_slice()), (3, &b"X12"[..]));
    // Reading through a symlink checks the file; a symlink's own sum is
    // not checked, as GNU cpio writes 0 for it.
    assert_eq!(read(&root, "link").unwrap_err().kind(), ErrorKind::Corrupt);
    let gnu = open(fixture("gnu-crc.cpio")).unwrap();
    assert_eq!(read(&gnu, "link").unwrap(), b"Hello, world!\n");
}

#[test]
fn refuses_names_that_could_escape() {
    for name in [&b"../evil"[..], b"a/../../evil", b"/etc/passwd"] {
        let archive = Writer::new(Format::Newc).file(name, b"x").finish();
        assert_eq!(kind(open(archive)), ErrorKind::InvalidName, "{name:?}");
    }
    let archive = Writer::new(Format::Odc)
        .file(b"inside", b"inside")
        .symlink(b"up", b"../../etc/passwd")
        .symlink(b"abs", b"/etc/passwd")
        .symlink(b"ok", b"inside")
        .finish();
    let root = open(archive).unwrap();
    assert_eq!(read(&root, "ok").unwrap(), b"inside");
    for path in ["up", "abs"] {
        let error = block_on(root.open_file(path)).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::EscapeBlocked, "{path}");
    }
}

#[test]
fn refuses_malformed_archives() {
    let one = Writer::new(Format::Newc).file(b"a", b"x").finish();
    assert_eq!(
        error(open(Vec::new())),
        (ErrorKind::Corrupt, at(0, "not a cpio archive"))
    );
    assert_eq!(
        error(open(one[..60].to_vec())),
        (ErrorKind::Corrupt, at(0, "truncated header"))
    );
    let mut bad = one.clone();
    bad[10] = b'G';
    assert_eq!(error(open(bad)), (ErrorKind::Corrupt, at(0, "bad header")));
    // The second header, after the first member's 116 bytes.
    let mut bad = one.clone();
    bad[116] = b'1';
    assert_eq!(
        error(open(bad)),
        (ErrorKind::Corrupt, at(116, "bad magic number"))
    );
    // Its name, without the NUL that must end it.
    let mut bad = one.clone();
    bad[111] = b'b';
    assert_eq!(error(open(bad)), (ErrorKind::Corrupt, at(0, "bad name")));
    let mut bad = one.clone();
    bad[94..102].copy_from_slice(b"00000000");
    assert_eq!(error(open(bad)), (ErrorKind::Corrupt, at(0, "bad name")));
    assert_eq!(
        error(open(one[..111].to_vec())),
        (ErrorKind::Corrupt, at(0, "truncated name"))
    );
    let mut bad = one.clone();
    bad[54..62].copy_from_slice(b"00010000");
    assert_eq!(
        error(open(bad)),
        (ErrorKind::Corrupt, at(0, "member extends past the end"))
    );
    let archive = Writer::new(Format::Newc)
        .entry(b"odd", 0o170_644, 1, 1, b"")
        .finish();
    assert_eq!(
        error(open(archive)),
        (ErrorKind::Corrupt, at(0, "unknown file type"))
    );
    // An archive may end without its trailer.
    let root = open(one[..116].to_vec()).unwrap();
    assert_eq!(read(&root, "a").unwrap(), b"x");
}

#[test]
fn enforces_limits() {
    let three = Writer::new(Format::Odc)
        .file(b"a", b"")
        .file(b"b", b"")
        .file(b"c", b"")
        .finish();
    let limits = Limits::default().with_max_entries(3);
    let error = block_on(Cpio::open_with(three, limits)).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::LimitExceeded);

    let long = Writer::new(Format::Newc).file(&[b'a'; 5000], b"").finish();
    assert_eq!(kind(open(long)), ErrorKind::LimitExceeded);
    let target = Writer::new(Format::Newc)
        .symlink(b"link", &[b'a'; 5000])
        .finish();
    assert_eq!(kind(open(target)), ErrorKind::LimitExceeded);
}

#[test]
fn lets_later_duplicates_win() {
    let archive = Writer::new(Format::Newc)
        .file(b"a", b"first")
        .file(b"a", b"second")
        .file(b"d", b"a file")
        .file(b"d/f", b"now a directory")
        .dir(b"e")
        .file(b"e", b"now a file")
        .finish();
    let root = open(archive).unwrap();
    assert_eq!(read(&root, "a").unwrap(), b"second");
    assert_eq!(read(&root, "d/f").unwrap(), b"now a directory");
    assert_eq!(read(&root, "e").unwrap(), b"now a file");
}

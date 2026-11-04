// This is free and unencumbered software released into the public domain.

//! Hostile, malformed, and unusual archives.

mod common;

use common::{Writer, header, seal};
use filer_api::{Dir, Error, ErrorKind, MetaMask};
use filer_spec::block_on;
use filer_tar::{Compression, Limits, Tar};

type Root = Dir<Tar<Vec<u8>>>;

fn open(bytes: Vec<u8>) -> Result<Root, Error> {
    block_on(Tar::open(bytes))
}

fn read(root: &Root, path: &str) -> Vec<u8> {
    let file = block_on(root.open_file(path)).unwrap();
    block_on(file.read_to_end()).unwrap()
}

fn kind(result: Result<Root, Error>) -> ErrorKind {
    result.unwrap_err().kind()
}

#[test]
fn refuses_names_that_could_escape() {
    for name in [&b"../evil"[..], b"a/../../evil", b"/etc/passwd"] {
        let archive = Writer::default().file(name, b"x").finish();
        assert_eq!(kind(open(archive)), ErrorKind::InvalidName, "{name:?}");
    }
    let archive = Writer::default()
        .pax(&[("path", b"a\0b")])
        .file(b"ignored", b"x")
        .finish();
    assert_eq!(kind(open(archive)), ErrorKind::InvalidName);
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
fn refuses_malformed_archives() {
    let mut archive = Writer::default().file(b"a", b"x").finish();
    archive[0] = b'b'; // The checksum no longer matches.
    let error = open(archive).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Corrupt);
    let source = core::error::Error::source(&error).unwrap().to_string();
    assert_eq!(source, "tar header at offset 0: bad checksum");

    let archive = Writer::default().file(b"a", b"x").finish();
    assert_eq!(kind(open(archive[..300].to_vec())), ErrorKind::Corrupt);

    let mut big = header(b"big", b'0', b"", 1 << 20);
    seal(&mut big);
    assert_eq!(kind(open(big.to_vec())), ErrorKind::Corrupt);
}

#[test]
fn enforces_limits() {
    let three = Writer::default()
        .file(b"a", b"")
        .file(b"b", b"")
        .file(b"c", b"")
        .finish();
    let limits = Limits::default().with_max_entries(2);
    let error = block_on(Tar::open_with(three, limits)).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::LimitExceeded);

    let long = Writer::default()
        .pax(&[("path", "a".repeat(5000).as_bytes())])
        .file(b"x", b"")
        .finish();
    assert_eq!(kind(open(long)), ErrorKind::LimitExceeded);

    let huge = Writer::default()
        .pax(&[("comment", vec![b'x'; 70 * 1024].as_slice())])
        .file(b"x", b"")
        .finish();
    assert_eq!(kind(open(huge)), ErrorKind::LimitExceeded);
}

#[test]
fn refuses_sparse_members() {
    let archive = Writer::default().member(b"sparse", b'S', b"", b"").finish();
    assert_eq!(kind(open(archive)), ErrorKind::Unsupported);
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
fn shares_nodes_between_hardlinks() {
    let archive = Writer::default()
        .file(b"original", b"shared")
        .member(b"link", b'1', b"original", b"")
        .finish();
    let root = open(archive).unwrap();
    assert_eq!(read(&root, "link"), b"shared");
    let id = |path| block_on(root.metadata(path, MetaMask::ID)).unwrap().id;
    assert_eq!(id("link"), id("original"));

    for target in [&b"missing"[..], b"dir"] {
        let archive = Writer::default()
            .dir(b"dir/")
            .member(b"link", b'1', target, b"")
            .finish();
        assert_eq!(kind(open(archive)), ErrorKind::Corrupt, "{target:?}");
    }
}

#[test]
fn reads_extended_names_and_metadata() {
    let long = format!("{}/file", "d".repeat(200));
    let archive = Writer::default()
        .pax(&[("path", long.as_bytes()), ("mtime", b"1700000000.5")])
        .file(b"short", b"pax")
        .member(b"././@LongLink", b'L', b"", b"gnu/long/name\0")
        .file(b"short", b"gnu")
        .member(b"unknown", b'Z', b"", b"read as a file")
        .finish();
    let root = open(archive).unwrap();
    assert_eq!(read(&root, &long), b"pax");
    let modified = block_on(root.metadata(long.as_str(), MetaMask::MTIME))
        .unwrap()
        .modified
        .unwrap();
    assert_eq!(
        (modified.secs(), modified.nanos()),
        (1_700_000_000, 500_000_000)
    );
    assert_eq!(read(&root, "gnu/long/name"), b"gnu");
    assert_eq!(read(&root, "unknown"), b"read as a file");
    // Neither override leaked into a later member's name.
    assert!(block_on(root.open_file("short")).is_err());
}

#[test]
fn joins_the_ustar_prefix() {
    let mut member = header(b"name", b'0', b"", 4);
    member[345..351].copy_from_slice(b"prefix");
    seal(&mut member);
    let mut archive = member.to_vec();
    archive.extend_from_slice(b"data");
    archive.resize(1024, 0);
    let root = open(archive).unwrap();
    assert_eq!(read(&root, "prefix/name"), b"data");
}

#[test]
fn accepts_archives_without_end_blocks() {
    let mut archive = Writer::default().file(b"a", b"x").finish();
    archive.truncate(1024);
    assert_eq!(read(&open(archive).unwrap(), "a"), b"x");
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
fn reads_archives_stored_in_another_driver() {
    let archive = Writer::default().file(b"inner.txt", b"nested").finish();
    let heap = filer_heap::Heap::builder()
        .file("archives/inner.tar", archive)
        .build();
    let file = block_on(heap.root_dir().open_file("archives/inner.tar")).unwrap();
    // The `File` of one driver is the source of another.
    let root = block_on(Tar::open(file)).unwrap();
    let inner = block_on(root.open_file("inner.txt")).unwrap();
    assert_eq!(block_on(inner.read_to_end()).unwrap(), b"nested");
}

#[test]
fn reads_the_headers_of_small_members_together() {
    let mut writer = Writer::default();
    for i in 0..1000 {
        writer.file(format!("m{i:04}").as_bytes(), b"x");
    }
    let source = filer_spec::Counted::new(writer.finish());
    let reads = source.reads();
    let root = block_on(Tar::open(source)).unwrap();
    let reads = reads.load(std::sync::atomic::Ordering::Relaxed);
    assert!(reads < 100, "{reads} reads to open 1,000 members");
    let file = block_on(root.open_file("m0999")).unwrap();
    assert_eq!(block_on(file.read_to_end()).unwrap(), b"x");
}

/// `hello.tar`, compressed by gzip 479 (Apple's), zstd 1.5.7, xz 5.8.4,
/// and bzip2 1.0.8.
const COMPRESSED: [(&str, Compression); 4] = [
    ("hello.tar.gz", Compression::Gzip),
    ("hello.tar.zst", Compression::Zstd),
    ("hello.tar.xz", Compression::Xz),
    ("hello.tar.bz2", Compression::Bzip2),
];

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!("{}/tests/data/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

#[test]
fn reads_compressed_archives() {
    for (name, compression) in COMPRESSED {
        let opened = open(fixture(name));
        if !compression.is_enabled() {
            assert_eq!(kind(opened), ErrorKind::Unsupported, "{name}");
            continue;
        }
        let root = opened.unwrap();
        assert_eq!(root.driver().compression(), Some(compression), "{name}");
        assert_eq!(read(&root, "hello/world.txt"), b"Hello, world!\n");
        // Reading back, before what was read last, decodes again.
        assert_eq!(read(&root, "hello/world.txt"), b"Hello, world!\n");
    }
    assert_eq!(
        open(fixture("hello.tar")).unwrap().driver().compression(),
        None
    );
}

#[test]
fn refuses_compressed_archives_that_fail_their_checks() {
    for (name, compression) in COMPRESSED.iter().filter(|(_, c)| c.is_enabled()) {
        // A bit of the next to last byte, which each format checks: gzip's
        // length, zstd's checksum, xz's footer, and bzip2's CRC.
        let mut archive = fixture(name);
        let at = archive.len() - 2;
        archive[at] ^= 1;
        let error = open(archive).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Corrupt, "{name}");
        let source = core::error::Error::source(&error).unwrap().to_string();
        assert_eq!(source, "tar header at offset 0: bad compressed stream");
        // Decoded, it passes the total size the limits allow.
        let limits = Limits::default().with_max_total_size(2559);
        let error = block_on(Tar::open_with(fixture(name), limits)).unwrap_err();
        assert_eq!(
            error.kind(),
            ErrorKind::LimitExceeded,
            "{name} ({compression:?})"
        );
    }
}

#[test]
fn reads_plain_archives_whose_names_begin_as_compressed_streams() {
    for name in [&b"BZh91AY&SY"[..], b"\x1F\x8B\x08name"] {
        let archive = Writer::default().file(name, b"plain").finish();
        let root = open(archive).unwrap();
        assert_eq!(root.driver().compression(), None);
    }
}

#[cfg(feature = "gzip")]
#[test]
fn reads_back_from_checkpoints() {
    // Twelve members of 1 MiB, deflated as stored blocks, so packed as
    // long: reading the one before the last, after the last, reads on
    // from the checkpoint before it rather than from the start.
    let member = |i: u8| -> Vec<u8> { (0..1 << 20).map(|n: u32| (n % 251) as u8 ^ i).collect() };
    let mut writer = Writer::default();
    for i in 0..12 {
        writer.file(format!("m{i:02}").as_bytes(), &member(i));
    }
    let source = filer_spec::Counted::new(common::gzip(&writer.finish(), 0));
    let reads = source.reads();
    let root = block_on(Tar::open(source)).unwrap();
    let read = |path: &str| {
        let file = block_on(root.open_file(path)).unwrap();
        block_on(file.read_to_end()).unwrap()
    };
    assert_eq!(read("m11"), member(11));
    let before = reads.load(std::sync::atomic::Ordering::Relaxed);
    assert_eq!(read("m10"), member(10));
    let after = reads.load(std::sync::atomic::Ordering::Relaxed);
    // At most 4 MiB back, and the member: 80 reads of 64 KiB, not 176.
    assert!(after - before <= 82, "{} reads", after - before);
}

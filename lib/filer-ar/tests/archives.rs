// This is free and unencumbered software released into the public domain.

//! Archives that real tools wrote, and hostile, malformed, and unusual
//! ones.
//!
//! `tests/data` holds what `make.sh` there wrote with GNU `ar`, LLVM's
//! `llvm-ar` (BSD and Windows formats), and `dpkg-deb`, and `darwin.a`,
//! which macOS's `ar` wrote from the same sources.

mod common;

use common::{DIALECTS, Dialect, Writer, header, member};
use filer_api::{Dir, Error, ErrorKind, MetaMask};
use filer_ar::{Ar, Limits};
use filer_spec::block_on;
use filer_tar::Tar;

type Root = Dir<Ar<Vec<u8>>>;

fn open(bytes: Vec<u8>) -> Result<Root, Error> {
    block_on(Ar::open(bytes))
}

fn read(root: &Root, path: &str) -> Vec<u8> {
    let file = block_on(root.open_file(path)).unwrap();
    block_on(file.read_to_end()).unwrap()
}

/// Lists `path` in `root`, sorted.
fn list<D: filer_api::Driver>(root: &Dir<D>, path: &str) -> Vec<String> {
    let dir = block_on(root.open_dir(path)).unwrap();
    let mut entries = dir.entries().unwrap();
    let mut names = Vec::new();
    while let Some(entry) = block_on(entries.next()) {
        names.push(entry.unwrap().name.to_string());
    }
    names.sort();
    names
}

fn error(result: Result<Root, Error>) -> (ErrorKind, String) {
    let error = result.unwrap_err();
    let source = core::error::Error::source(&error).map(ToString::to_string);
    (error.kind(), source.unwrap_or_default())
}

fn kind(result: Result<Root, Error>) -> ErrorKind {
    error(result).0
}

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!("{}/tests/data/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

#[test]
fn reads_what_ar_tools_write() {
    let objects = ["a_rather_long_object_name.o", "hello.o"];
    for (archive, extra, magic) in [
        ("gnu.a", &["notes.txt"][..], &b"\x7fELF"[..]),
        ("bsd.a", &["notes.txt", "with space.txt"], b"\x7fELF"),
        ("coff.lib", &["notes.txt"], b"\x7fELF"),
        ("darwin.a", &[], b"\xCF\xFA\xED\xFE"),
    ] {
        let root = open(fixture(archive)).unwrap();
        // The symbol tables and name tables are hidden.
        let mut expected: Vec<&str> = objects.iter().chain(extra).copied().collect();
        expected.sort_unstable();
        assert_eq!(list(&root, ""), expected, "{archive}");
        assert!(read(&root, "hello.o").starts_with(magic), "{archive}");
        if !extra.is_empty() {
            assert_eq!(read(&root, "notes.txt"), b"Hello, world!\n", "{archive}");
            let metadata = block_on(root.metadata("notes.txt", MetaMask::all())).unwrap();
            assert_eq!(metadata.len, Some(14));
            assert_eq!(metadata.modified.unwrap().secs(), 1_700_000_000);
        }
    }
    let bsd = open(fixture("bsd.a")).unwrap();
    assert_eq!(read(&bsd, "with space.txt"), b"spaced\n");
    // macOS's `ar` pads each object so that its data stays aligned.
    let darwin = open(fixture("darwin.a")).unwrap();
    let hello = block_on(darwin.metadata("hello.o", MetaMask::all())).unwrap();
    assert_eq!(hello.len, Some(512));
}

#[test]
fn reads_debian_packages() {
    let package = block_on(Ar::open(fixture("hello.deb"))).unwrap();
    assert_eq!(
        list(&package, ""),
        ["control.tar", "data.tar", "debian-binary"]
    );
    let control = block_on(package.open_file("control.tar")).unwrap();
    let control = block_on(Tar::open(control)).unwrap();
    let file = block_on(control.open_file("control")).unwrap();
    let text = String::from_utf8(block_on(file.read_to_end()).unwrap()).unwrap();
    assert!(text.starts_with("Package: hello\n"), "{text}");
    let data = block_on(package.open_file("data.tar")).unwrap();
    let data = block_on(Tar::open(data)).unwrap();
    assert_eq!(list(&data, "usr/share/doc/hello"), ["README"]);
}

#[test]
fn reads_each_dialect() {
    let long = "a name longer than sixteen bytes.txt";
    for dialect in [Dialect::Gnu, Dialect::Bsd] {
        let archive = Writer::new(dialect)
            .file(b"odd", b"odd")
            .file(long.as_bytes(), b"long")
            .file(b"dir/nested.o", b"nested")
            .file(b"last", b"x")
            .finish();
        let root = open(archive).unwrap();
        assert_eq!(list(&root, ""), [long, "dir", "last", "odd"], "{dialect:?}");
        assert_eq!(read(&root, "odd"), b"odd");
        assert_eq!(read(&root, long), b"long");
        assert_eq!(read(&root, "dir/nested.o"), b"nested");
        assert_eq!(read(&root, "last"), b"x");
    }
    // Windows ends long names with NUL rather than `/\n`.
    let mut archive = b"!<arch>\n".to_vec();
    member(
        &mut archive,
        b"//",
        b"first long name.obj\0second long name.obj\0",
    );
    member(&mut archive, b"/20", b"second");
    member(&mut archive, b"/0", b"first");
    let root = open(archive).unwrap();
    assert_eq!(read(&root, "first long name.obj"), b"first");
    assert_eq!(read(&root, "second long name.obj"), b"second");
    // The last member's padding may be missing, and an archive empty.
    let mut archive = Writer::new(Dialect::SysV).file(b"odd", b"odd").finish();
    archive.pop();
    assert_eq!(read(&open(archive).unwrap(), "odd"), b"odd");
    assert!(list(&open(b"!<arch>\n".to_vec()).unwrap(), "").is_empty());
}

#[test]
fn hides_symbol_tables() {
    let mut archive = b"!<arch>\n".to_vec();
    for name in [&b"/"[..], b"/SYM64/", b"/<ECSYMBOLS>/", b"__.SYMDEF"] {
        member(&mut archive, name, b"symbols");
    }
    member(&mut archive, b"#1/20", b"__.SYMDEF SORTED\0\0\0\0symbols");
    member(&mut archive, b"file/", b"file");
    let root = open(archive).unwrap();
    assert_eq!(list(&root, ""), ["file"]);
}

#[test]
fn refuses_names_that_could_escape() {
    for dialect in [Dialect::Gnu, Dialect::Bsd] {
        let name = "a/../../evil name longer than sixteen";
        for name in [
            &b"../evil"[..],
            name.as_bytes(),
            b"/etc/passwd in a long name",
        ] {
            let archive = Writer::new(dialect).file(name, b"x").finish();
            assert_eq!(kind(open(archive)), ErrorKind::InvalidName, "{name:?}");
        }
    }
    let archive = Writer::new(Dialect::Bsd)
        .file(b"nul\0in a long name", b"x")
        .finish();
    assert_eq!(kind(open(archive)), ErrorKind::InvalidName);
}

#[test]
fn refuses_malformed_archives() {
    let at = |offset: u64, reason: &str| format!("ar header at offset {offset}: {reason}");
    let one = Writer::new(Dialect::SysV).file(b"a", b"x").finish();
    assert_eq!(
        error(open(b"!<arch\n".to_vec())),
        (ErrorKind::Corrupt, at(0, "not an ar archive"))
    );
    assert_eq!(
        error(open(b"!<thin>\n".to_vec())),
        (ErrorKind::Unsupported, at(0, "thin archives"))
    );
    let mut bad = one.clone();
    bad[66] = b'!';
    assert_eq!(error(open(bad)), (ErrorKind::Corrupt, at(8, "bad header")));
    let mut bad = one.clone();
    bad[56] = b'x';
    assert_eq!(error(open(bad)), (ErrorKind::Corrupt, at(8, "bad size")));
    let mut bad = one.clone();
    bad[24] = b'x';
    assert_eq!(error(open(bad)), (ErrorKind::Corrupt, at(8, "bad time")));
    assert_eq!(
        error(open(one[..40].to_vec())),
        (ErrorKind::Corrupt, at(8, "truncated header"))
    );
    let mut past = b"!<arch>\n".to_vec();
    past.extend(header(b"a", 2));
    past.push(b'x');
    assert_eq!(
        error(open(past)),
        (ErrorKind::Corrupt, at(8, "member extends past the end"))
    );

    let mut archive = b"!<arch>\n".to_vec();
    member(&mut archive, b"/0", b"no table");
    assert_eq!(
        error(open(archive)),
        (ErrorKind::Corrupt, at(8, "long name before the name table"))
    );
    let mut archive = b"!<arch>\n".to_vec();
    member(&mut archive, b"//", b"name/\n");
    member(&mut archive, b"/7", b"past the table");
    assert_eq!(
        error(open(archive)),
        (ErrorKind::Corrupt, at(74, "bad long name"))
    );
    let mut archive = b"!<arch>\n".to_vec();
    member(&mut archive, b"//", b"name/\n");
    member(&mut archive, b"//", b"name/\n");
    assert_eq!(
        error(open(archive)),
        (ErrorKind::Corrupt, at(74, "second name table"))
    );
    for name in [&b"#1/9"[..], b"#1/x"] {
        let mut archive = b"!<arch>\n".to_vec();
        member(&mut archive, name, b"short");
        assert_eq!(kind(open(archive)), ErrorKind::Corrupt, "{name:?}");
    }
}

#[test]
fn enforces_limits() {
    let three = Writer::new(Dialect::SysV)
        .file(b"a", b"")
        .file(b"b", b"")
        .file(b"c", b"")
        .finish();
    let limits = Limits::default().with_max_entries(2);
    let error = block_on(Ar::open_with(three, limits)).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::LimitExceeded);

    for dialect in [Dialect::Gnu, Dialect::Bsd] {
        let long = Writer::new(dialect).file(&[b'a'; 5000], b"").finish();
        assert_eq!(kind(open(long)), ErrorKind::LimitExceeded, "{dialect:?}");
    }
    let mut huge = b"!<arch>\n".to_vec();
    member(&mut huge, b"//", &vec![b'x'; 70 * 1024]);
    assert_eq!(kind(open(huge)), ErrorKind::LimitExceeded);
}

#[test]
fn lets_later_duplicates_win() {
    for dialect in DIALECTS {
        let archive = Writer::new(dialect)
            .file(b"a.o", b"first")
            .file(b"a.o", b"second")
            .finish();
        assert_eq!(read(&open(archive).unwrap(), "a.o"), b"second");
    }
}

#[test]
fn reads_members_in_pieces_without_overrunning_them() {
    let archive = Writer::new(Dialect::Bsd)
        .file(b"a long name for the first member", b"0123456789")
        .file(b"b", b"other")
        .finish();
    let root = open(archive).unwrap();
    let file = block_on(root.open_file("a long name for the first member")).unwrap();
    let filer_api::BufResult(n, buf) = block_on(file.read_at(Vec::with_capacity(100), 6));
    assert_eq!((n.unwrap(), buf.as_slice()), (4, &b"6789"[..]));
    let filer_api::BufResult(n, _) = block_on(file.read_at(Vec::with_capacity(100), 10));
    assert_eq!(n.unwrap(), 0);
}

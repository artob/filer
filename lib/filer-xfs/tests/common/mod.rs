// This is free and unencumbered software released into the public domain.

//! Shared by the tests: reading fixtures, and what they hold.

// Each test binary compiles this module and uses only part of it.
#![allow(dead_code)]

use filer_api::{BufResult, FileKind, IoBufMut, ReadAt};
use std::{collections::BTreeMap, future::Future};

pub mod exercise;

const SECTOR: usize = 512;

/// The modification time `tests/fixtures/make.sh` gives files:
/// 2024-01-02T03:04:06.123456789Z.
pub const MTIME: i64 = 1_704_164_646;
pub const MTIME_NANOS: u32 = 123_456_789;

/// A volume image as `tests/fixtures/make.sh` stores it: runs of identical
/// nonzero sectors, all others zero. Read as it is, as the volumes are of
/// hundreds of megabytes, mostly zeros.
pub struct Sparse {
    len: u64,
    /// Runs of sectors, ascending: the first, the count, and the bytes.
    runs: Vec<(u64, u64, [u8; SECTOR])>,
}

impl Sparse {
    pub fn new(sparse: &[u8]) -> Self {
        let rest = sparse
            .strip_prefix(b"FILER-SPARSE-2\n")
            .expect("sparse magic");
        let (len, mut rest) = rest.split_at(8);
        let len = u64::from_le_bytes(len.try_into().unwrap());
        let mut runs = Vec::new();
        while !rest.is_empty() {
            let first = u32::from_le_bytes(rest[0..4].try_into().unwrap());
            let count = u32::from_le_bytes(rest[4..8].try_into().unwrap());
            let sector = rest[8..8 + SECTOR].try_into().unwrap();
            runs.push((u64::from(first), u64::from(count), sector));
            rest = &rest[8 + SECTOR..];
        }
        Self { len, runs }
    }

    pub const fn len(&self) -> u64 {
        self.len
    }

    /// Returns sector `index`, if it is not zero.
    fn sector(&self, index: u64) -> Option<&[u8; SECTOR]> {
        let after = self.runs.partition_point(|&(first, _, _)| first <= index);
        let (first, count, sector) = self.runs.get(after.checked_sub(1)?)?;
        (index < first + count).then_some(sector)
    }

    /// Returns the byte at `at`.
    pub fn byte(&self, at: u64) -> u8 {
        self.sector(at / SECTOR as u64)
            .map_or(0, |sector| sector[(at % SECTOR as u64) as usize])
    }

    /// Fills `out` from `at`, which must be within the image.
    pub fn copy(&self, at: u64, out: &mut [u8]) {
        let mut done = 0;
        while done < out.len() {
            let pos = at + done as u64;
            let within = (pos % SECTOR as u64) as usize;
            let n = (SECTOR - within).min(out.len() - done);
            match self.sector(pos / SECTOR as u64) {
                Some(sector) => out[done..done + n].copy_from_slice(&sector[within..within + n]),
                None => out[done..done + n].fill(0),
            }
            done += n;
        }
    }

    /// Returns the offsets of the nonzero sectors.
    pub fn sectors(&self) -> impl Iterator<Item = u64> + '_ {
        self.runs
            .iter()
            .flat_map(|&(first, count, _)| (first..first + count).map(|i| i * SECTOR as u64))
    }
}

impl ReadAt for Sparse {
    fn read_at<B: IoBufMut>(
        &self,
        mut buf: B,
        off: u64,
    ) -> impl Future<Output = BufResult<usize, B>> + Send {
        let result = buf.fill(&mut |spare| {
            let n = (spare.len() as u64).min(self.len.saturating_sub(off)) as usize;
            self.copy(off, &mut spare[..n]);
            Ok(n)
        });
        core::future::ready(BufResult(result, buf))
    }

    fn size(&self) -> impl Future<Output = filer_api::Result<u64>> + Send {
        core::future::ready(Ok(self.len))
    }
}

/// What a path holds.
#[derive(Debug, PartialEq, Eq)]
pub enum Expected {
    Dir,
    File(Vec<u8>),
    Link(Vec<u8>),
    Special(FileKind),
}

/// The pattern `make.sh` writes.
pub fn pattern(size: usize, seed: usize) -> Vec<u8> {
    (0..size).map(|i| ((i * 7 + seed) % 251) as u8).collect()
}

/// The tree `make.sh`'s `fill` writes, after its deletions, but for the
/// file whose name is not UTF-8.
pub fn expected() -> BTreeMap<String, Expected> {
    let mut tree = BTreeMap::new();
    let mut file = |path: &str, contents: Vec<u8>| {
        tree.insert(path.to_owned(), Expected::File(contents));
    };
    file("README.TXT", pattern(9, 1));
    file("notes.txt", pattern(10, 2));
    file("A long file name with spaces.txt", pattern(100, 3));
    file("Ünïcödé ✓.txt", pattern(7, 4));
    file("emoji 😀.bin", pattern(5, 5));
    file("empty", Vec::new());
    file("big.bin", pattern(100_000, 7));
    file("Sub/nested.bin", pattern(5000, 8));
    file("Sub/Deeper/leaf.txt", pattern(5, 9));
    for i in 0..150 {
        file(&format!("many/f{i:03}"), pattern(i, 100 + i));
    }
    for (name, seed) in [("frag.a", 20), ("frag.b", 40)] {
        file(
            name,
            (0..12).flat_map(|i| pattern(4096, seed + i)).collect(),
        );
    }
    let mut sparse = vec![0; 1_000_000];
    sparse.extend(b"end");
    file("sparse.bin", sparse);
    let mut deep = vec![0; 70_000_003];
    deep[..5].copy_from_slice(b"start");
    deep[300_000..300_006].copy_from_slice(b"middle");
    deep[70_000_000..].copy_from_slice(b"end");
    file("deep.bin", deep);
    file("small", pattern(4, 10));
    file("Sub/hardlink", pattern(4, 10));
    file("Case.txt", pattern(3, 11));
    file("case.txt", pattern(4, 12));
    file(&"n".repeat(255), pattern(6, 13));
    for (link, target) in [
        ("link", "README.TXT".to_owned()),
        ("deeplink", "Sub/Deeper/leaf.txt".to_owned()),
        ("dirlink", "Sub".to_owned()),
        ("abs", "/notes.txt".to_owned()),
        ("dangling", "nope".to_owned()),
        ("long", format!("Sub/{}", "x".repeat(100))),
    ] {
        tree.insert(link.to_owned(), Expected::Link(target.into_bytes()));
    }
    for (name, kind) in [
        ("fifo", FileKind::Fifo),
        ("sock", FileKind::Socket),
        ("chr", FileKind::CharDevice),
        ("blk", FileKind::BlockDevice),
    ] {
        tree.insert(format!("special/{name}"), Expected::Special(kind));
    }
    for dir in ["Sub", "Sub/Deeper", "many", "special"] {
        tree.insert(dir.to_owned(), Expected::Dir);
    }
    tree
}

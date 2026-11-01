// This is free and unencumbered software released into the public domain.

//! A minimal FAT image writer for tests: it lays out a tree of files and
//! directories on a fresh FAT12, FAT16, or FAT32 volume, giving long names
//! and short aliases as Windows would, and records where everything went
//! so that tests can then corrupt it.

// Each test binary compiles this module and uses only part of it.
#![allow(dead_code)]

use std::collections::BTreeMap;

const SECTOR: usize = 512;

/// A file or directory to write.
pub enum Node {
    File(String, Vec<u8>),
    Dir(String, Vec<Node>),
}

pub fn file(name: impl Into<String>, data: impl Into<Vec<u8>>) -> Node {
    Node::File(name.into(), data.into())
}

pub fn dir(name: impl Into<String>, children: Vec<Node>) -> Node {
    Node::Dir(name.into(), children)
}

impl Node {
    fn name(&self) -> &str {
        match self {
            Self::File(name, _) | Self::Dir(name, _) => name,
        }
    }

    /// Adds a file (with `data`) or directory (without) at the
    /// `/`-separated `path` below this directory, whose parents must have
    /// been added already.
    pub fn add(&mut self, path: &str, data: Option<Vec<u8>>) {
        let Self::Dir(_, children) = self else {
            panic!("{path}: parent is not a directory");
        };
        match path.split_once('/') {
            Some((first, rest)) => children
                .iter_mut()
                .find(|child| child.name() == first)
                .unwrap_or_else(|| panic!("{path}: no parent"))
                .add(rest, data),
            None => children.push(match data {
                Some(data) => file(path, data),
                None => dir(path, Vec::new()),
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Fat12,
    Fat16,
    Fat32,
}

pub const KINDS: [Kind; 3] = [Kind::Fat12, Kind::Fat16, Kind::Fat32];

/// The modification time every entry gets: 2024-01-02T03:04:06, local.
pub const MTIME: i64 = 1_704_164_646;

/// A written volume.
pub struct Image {
    pub kind: Kind,
    pub bytes: Vec<u8>,
    cluster_sectors: usize,
    fat: usize,
    fat_sectors: usize,
    data: usize,
    /// The first cluster of every file and directory with one, by path.
    pub clusters: BTreeMap<String, u32>,
    /// The offset of each entry's short slot, by path.
    pub slots: BTreeMap<String, usize>,
    next: u32,
    fragmented: bool,
}

impl Image {
    /// Writes `root` on a new volume of `kind`.
    pub fn new(kind: Kind, root: Vec<Node>) -> Self {
        Self::build(kind, root, false)
    }

    /// Writes `root` with every file's clusters spread out, one free
    /// cluster between each, so that no two are contiguous.
    pub fn fragmented(kind: Kind, root: Vec<Node>) -> Self {
        Self::build(kind, root, true)
    }

    fn build(kind: Kind, root: Vec<Node>, fragmented: bool) -> Self {
        // FAT12: a 1.44 MB floppy. FAT16 and FAT32: 64 MiB.
        let (cluster_sectors, reserved, root_entries, total, fat_sectors) = match kind {
            Kind::Fat12 => (1, 1, 224, 2880, 9),
            Kind::Fat16 => (4, 1, 512, 131_072, 128),
            Kind::Fat32 => (1, 32, 0, 131_072, 1016),
        };
        let root_sectors = (root_entries * 32usize).div_ceil(SECTOR);
        let fat = reserved;
        let data = reserved + 2 * fat_sectors + root_sectors;
        let mut image = Self {
            kind,
            bytes: vec![0; total * SECTOR],
            cluster_sectors,
            fat: fat * SECTOR,
            fat_sectors,
            data: data * SECTOR,
            clusters: BTreeMap::new(),
            slots: BTreeMap::new(),
            next: 2,
            fragmented,
        };
        image.boot_sector(cluster_sectors, reserved, root_entries, total, fat_sectors);
        let media = 0xF8;
        image.set_fat(0, 0x0FFF_FF00 | media);
        image.set_fat(1, 0x0FFF_FFFF);
        match kind {
            Kind::Fat32 => {
                let slots = slots_of(&root, false);
                let clusters = image.alloc_dir(slots);
                image.write_dir("", &clusters, root, None);
            },
            Kind::Fat12 | Kind::Fat16 => {
                assert!(slots_of(&root, false) <= root_entries, "root too full");
                let offset = (reserved + 2 * fat_sectors) * SECTOR;
                let len = root_entries * 32;
                image.write_entries("", Region::Fixed(offset, len), root, None);
            },
        }
        image
    }

    pub fn cluster_size(&self) -> usize {
        self.cluster_sectors * SECTOR
    }

    fn boot_sector(
        &mut self,
        cluster_sectors: usize,
        reserved: usize,
        root_entries: usize,
        total: usize,
        fat_sectors: usize,
    ) {
        let b = &mut self.bytes;
        b[..3].copy_from_slice(&[0xEB, 0x58, 0x90]);
        b[3..11].copy_from_slice(b"FILER.RS");
        b[11..13].copy_from_slice(&(SECTOR as u16).to_le_bytes());
        b[13] = cluster_sectors as u8;
        b[14..16].copy_from_slice(&(reserved as u16).to_le_bytes());
        b[16] = 2;
        b[17..19].copy_from_slice(&(root_entries as u16).to_le_bytes());
        if total < 0x1_0000 {
            b[19..21].copy_from_slice(&(total as u16).to_le_bytes());
        } else {
            b[32..36].copy_from_slice(&(total as u32).to_le_bytes());
        }
        b[21] = 0xF8;
        b[24..26].copy_from_slice(&63u16.to_le_bytes());
        b[26..28].copy_from_slice(&255u16.to_le_bytes());
        let extended = match self.kind {
            Kind::Fat12 | Kind::Fat16 => {
                b[22..24].copy_from_slice(&(fat_sectors as u16).to_le_bytes());
                36
            },
            Kind::Fat32 => {
                b[36..40].copy_from_slice(&(fat_sectors as u32).to_le_bytes());
                b[44..48].copy_from_slice(&2u32.to_le_bytes()); // Root cluster.
                b[48..50].copy_from_slice(&1u16.to_le_bytes()); // FSInfo.
                b[50..52].copy_from_slice(&6u16.to_le_bytes()); // Backup.
                let fs_info = &mut b[SECTOR..2 * SECTOR];
                fs_info[..4].copy_from_slice(&0x4161_5252u32.to_le_bytes());
                fs_info[484..488].copy_from_slice(&0x6141_7272u32.to_le_bytes());
                fs_info[488..496].fill(0xFF);
                fs_info[508..512].copy_from_slice(&0xAA55_0000u32.to_le_bytes());
                64
            },
        };
        b[extended] = 0x80; // Drive number.
        b[extended + 2] = 0x29; // Extended boot signature.
        b[extended + 3..extended + 7].copy_from_slice(&0x1234_5678u32.to_le_bytes());
        b[extended + 7..extended + 18].copy_from_slice(b"FILER      ");
        let fs_type: &[u8; 8] = match self.kind {
            Kind::Fat12 => b"FAT12   ",
            Kind::Fat16 => b"FAT16   ",
            Kind::Fat32 => b"FAT32   ",
        };
        b[extended + 18..extended + 26].copy_from_slice(fs_type);
        b[510] = 0x55;
        b[511] = 0xAA;
        if self.kind == Kind::Fat32 {
            let (boot, rest) = b.split_at_mut(6 * SECTOR);
            rest[..2 * SECTOR].copy_from_slice(&boot[..2 * SECTOR]);
        }
    }

    /// Sets the FAT entry of cluster `n` in both FATs, truncated to the
    /// entry width.
    pub fn set_fat(&mut self, n: u32, raw: u32) {
        let n = n as usize;
        for copy in 0..2 {
            let table = self.fat + copy * self.fat_sectors * SECTOR;
            let b = &mut self.bytes;
            match self.kind {
                Kind::Fat12 => {
                    let at = table + n * 3 / 2;
                    let pair = u16::from_le_bytes([b[at], b[at + 1]]);
                    let raw = (raw & 0x0FFF) as u16;
                    let pair = if n.is_multiple_of(2) {
                        (pair & 0xF000) | raw
                    } else {
                        (pair & 0x000F) | (raw << 4)
                    };
                    b[at..at + 2].copy_from_slice(&pair.to_le_bytes());
                },
                Kind::Fat16 => {
                    let at = table + n * 2;
                    b[at..at + 2].copy_from_slice(&(raw as u16).to_le_bytes());
                },
                Kind::Fat32 => {
                    let at = table + n * 4;
                    b[at..at + 4].copy_from_slice(&raw.to_le_bytes());
                },
            }
        }
    }

    /// Returns the offset of cluster `n`.
    pub fn cluster_offset(&self, n: u32) -> usize {
        self.data + (n as usize - 2) * self.cluster_size()
    }

    /// Allocates and links `count` clusters, spread out if fragmented.
    fn alloc(&mut self, count: usize, spread: bool) -> Vec<u32> {
        let step = if spread { 2 } else { 1 };
        let clusters: Vec<u32> = (0..count as u32).map(|i| self.next + i * step).collect();
        self.next = clusters.last().map_or(self.next, |&last| last + step);
        for pair in clusters.windows(2) {
            self.set_fat(pair[0], pair[1]);
        }
        if let Some(&last) = clusters.last() {
            self.set_fat(last, 0x0FFF_FFFF);
        }
        clusters
    }

    fn alloc_dir(&mut self, slots: usize) -> Vec<u32> {
        let count = (slots * 32).div_ceil(self.cluster_size()).max(1);
        self.alloc(count, false)
    }

    fn write_dir(
        &mut self,
        path: &str,
        clusters: &[u32],
        children: Vec<Node>,
        parent: Option<u32>,
    ) {
        self.write_entries(path, Region::Clusters(clusters.to_vec()), children, parent);
    }

    /// Writes the entries of a directory at `path` into `region`, and
    /// everything below it.
    fn write_entries(
        &mut self,
        path: &str,
        region: Region,
        children: Vec<Node>,
        dots: Option<u32>,
    ) {
        let mut slots: Vec<[u8; 32]> = Vec::new();
        if let (Some(parent), Region::Clusters(clusters)) = (dots, &region) {
            slots.push(short(*b".          ", 0x10, 0, clusters[0], 0));
            slots.push(short(*b"..         ", 0x10, 0, parent, 0));
        }
        // The `..` entries of the root's subdirectories hold cluster 0.
        let this = match &region {
            Region::Clusters(clusters) if !path.is_empty() => clusters[0],
            Region::Fixed(..) | Region::Clusters(_) => 0,
        };
        let mut aliases = Vec::new();
        for node in children {
            let (name, is_dir) = match &node {
                Node::File(name, _) => (name.clone(), false),
                Node::Dir(name, _) => (name.clone(), true),
            };
            let name = name.as_str();
            let child = if path.is_empty() {
                name.to_owned()
            } else {
                format!("{path}/{name}")
            };
            let (first, size) = match node {
                Node::File(_, data) => {
                    let count = data.len().div_ceil(self.cluster_size());
                    let clusters = self.alloc(count, self.fragmented);
                    for (i, &cluster) in clusters.iter().enumerate() {
                        let chunk = &data[i * self.cluster_size()..];
                        let chunk = &chunk[..chunk.len().min(self.cluster_size())];
                        let at = self.cluster_offset(cluster);
                        self.bytes[at..at + chunk.len()].copy_from_slice(chunk);
                    }
                    (clusters.first().copied().unwrap_or(0), data.len() as u32)
                },
                Node::Dir(_, grandchildren) => {
                    let clusters = self.alloc_dir(slots_of(&grandchildren, true));
                    self.write_dir(&child, &clusters, grandchildren, Some(this));
                    (clusters[0], 0)
                },
            };
            if first != 0 {
                self.clusters.insert(child.clone(), first);
            }
            let (short_name, case, long) = alias(name, &mut aliases);
            if let Some(long) = long {
                slots.extend(long_slots(&long, &short_name));
            }
            let attrs = if is_dir { 0x10 } else { 0x20 };
            self.slots.insert(child, slots.len());
            slots.push(short(short_name, attrs, case, first, size));
        }
        let at = |image: &Self, i: usize| -> usize {
            match &region {
                Region::Fixed(offset, len) => {
                    assert!(i * 32 < *len);
                    offset + i * 32
                },
                Region::Clusters(clusters) => {
                    let per = image.cluster_size() / 32;
                    image.cluster_offset(clusters[i / per]) + (i % per) * 32
                },
            }
        };
        for (i, slot) in slots.iter().enumerate() {
            let at = at(self, i);
            self.bytes[at..at + 32].copy_from_slice(slot);
        }
        // `slots` held indexes for this directory's entries: make them
        // offsets.
        let entries: Vec<(String, usize)> = self
            .slots
            .iter()
            .filter(|(p, _)| parent_of(p) == path)
            .map(|(p, &i)| (p.clone(), i))
            .collect();
        for (p, i) in entries {
            self.slots.insert(p, at(self, i));
        }
    }

    /// Returns the bytes of the short slot of the entry at `path`.
    pub fn slot(&mut self, path: &str) -> &mut [u8] {
        let at = self.slots[path];
        &mut self.bytes[at..at + 32]
    }
}

enum Region {
    Fixed(usize, usize),
    Clusters(Vec<u32>),
}

fn parent_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(parent, _)| parent)
}

/// Returns the number of slots a directory's entries take.
fn slots_of(children: &[Node], dots: bool) -> usize {
    let mut aliases = Vec::new();
    let entries: usize = children
        .iter()
        .map(|node| {
            let (_, _, long) = alias(node.name(), &mut aliases);
            1 + long.map_or(0, |long| long.encode_utf16().count().div_ceil(13))
        })
        .sum();
    entries + if dots { 2 } else { 0 }
}

/// Returns the short name and case flags for `name`, and the long name to
/// write too, if one is needed.
fn alias(name: &str, aliases: &mut Vec<[u8; 11]>) -> ([u8; 11], u8, Option<String>) {
    let (base, ext) = name.rsplit_once('.').unwrap_or((name, ""));
    let fits = |part: &str, max: usize| {
        !part.is_empty() && part.len() <= max && part.bytes().all(|b| b.is_ascii_alphanumeric())
    };
    let case = |part: &str| {
        if part.bytes().all(|b| !b.is_ascii_lowercase()) {
            Some(false)
        } else if part.bytes().all(|b| !b.is_ascii_uppercase()) {
            Some(true)
        } else {
            None
        }
    };
    let mut short = [b' '; 11];
    if fits(base, 8)
        && (ext.is_empty() || fits(ext, 3))
        && let (Some(lower_base), Some(lower_ext)) = (case(base), case(ext))
    {
        short[..base.len()].copy_from_slice(base.to_ascii_uppercase().as_bytes());
        short[8..8 + ext.len()].copy_from_slice(ext.to_ascii_uppercase().as_bytes());
        let flags = if lower_base { 0x08 } else { 0 } | if lower_ext { 0x10 } else { 0 };
        return (short, flags, None);
    }
    let clean = |part: &str, max: usize| -> Vec<u8> {
        part.bytes()
            .filter(u8::is_ascii_alphanumeric)
            .map(|b| b.to_ascii_uppercase())
            .take(max)
            .collect()
    };
    // Numbered per stem, from 1, as Windows does.
    let ext = clean(ext, 3);
    for n in 1.. {
        let suffix = format!("~{n}");
        let mut stem = clean(base, 8 - suffix.len());
        if stem.is_empty() {
            stem.push(b'X');
        }
        stem.extend_from_slice(suffix.as_bytes());
        let mut candidate = [b' '; 11];
        candidate[..stem.len()].copy_from_slice(&stem);
        candidate[8..8 + ext.len()].copy_from_slice(&ext);
        if !aliases.contains(&candidate) {
            aliases.push(candidate);
            short = candidate;
            break;
        }
    }
    (short, 0, Some(name.to_owned()))
}

/// Returns the checksum of a short name.
pub fn checksum(short: &[u8; 11]) -> u8 {
    short
        .iter()
        .fold(0u8, |sum, &b| sum.rotate_right(1).wrapping_add(b))
}

fn long_slots(name: &str, short: &[u8; 11]) -> Vec<[u8; 32]> {
    let mut units: Vec<u16> = name.encode_utf16().collect();
    let fragments = units.len().div_ceil(13);
    if units.len() < fragments * 13 {
        units.push(0);
    }
    units.resize(fragments * 13, 0xFFFF);
    let offsets = [1, 3, 5, 7, 9, 14, 16, 18, 20, 22, 24, 28, 30];
    (1..=fragments)
        .rev()
        .map(|order| {
            let mut slot = [0; 32];
            slot[0] = order as u8 | if order == fragments { 0x40 } else { 0 };
            slot[11] = 0x0F;
            slot[13] = checksum(short);
            for (i, at) in offsets.into_iter().enumerate() {
                let unit = units[(order - 1) * 13 + i];
                slot[at..at + 2].copy_from_slice(&unit.to_le_bytes());
            }
            slot
        })
        .collect()
}

/// Returns a short slot, with the modification, creation, and access
/// times of [`MTIME`].
fn short(name: [u8; 11], attrs: u8, case: u8, cluster: u32, size: u32) -> [u8; 32] {
    let date: u16 = ((2024 - 1980) << 9) | (1 << 5) | 2;
    let time: u16 = (3 << 11) | (4 << 5) | (6 / 2);
    let mut slot = [0; 32];
    slot[..11].copy_from_slice(&name);
    slot[11] = attrs;
    slot[12] = case;
    slot[14..16].copy_from_slice(&time.to_le_bytes());
    slot[16..18].copy_from_slice(&date.to_le_bytes());
    slot[18..20].copy_from_slice(&date.to_le_bytes());
    slot[20..22].copy_from_slice(&((cluster >> 16) as u16).to_le_bytes());
    slot[22..24].copy_from_slice(&time.to_le_bytes());
    slot[24..26].copy_from_slice(&date.to_le_bytes());
    slot[26..28].copy_from_slice(&(cluster as u16).to_le_bytes());
    slot[28..32].copy_from_slice(&size.to_le_bytes());
    slot
}

/// Expands a sparse image, as `tests/fixtures/make.sh` stores them.
pub fn unsparse(sparse: &[u8]) -> Vec<u8> {
    let rest = sparse
        .strip_prefix(b"FILER-SPARSE-1\n")
        .expect("sparse magic");
    let (size, mut rest) = rest.split_at(8);
    let mut image = vec![0; u64::from_le_bytes(size.try_into().unwrap()) as usize];
    while !rest.is_empty() {
        let (index, sector) = rest.split_at(4);
        let at = u32::from_le_bytes(index.try_into().unwrap()) as usize * SECTOR;
        image[at..at + SECTOR].copy_from_slice(&sector[..SECTOR]);
        rest = &sector[SECTOR..];
    }
    image
}

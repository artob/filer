// This is free and unencumbered software released into the public domain.

//! A minimal ZIP writer for tests: stored and Deflate members, built by
//! hand so that tests can also write malformed and hostile archives.

// Each test binary compiles this module and uses only part of it.
#![allow(dead_code)]

pub mod exercise;

/// Unix file modes, as Info-ZIP stores them in external attributes.
pub const FILE: u32 = 0o100_644;
pub const DIR: u32 = 0o040_755;
pub const SYMLINK: u32 = 0o120_777;
pub const FIFO: u32 = 0o010_644;

/// A member to write, with fields tests may tamper with.
#[derive(Clone)]
pub struct Entry {
    pub name: Vec<u8>,
    pub data: Vec<u8>,
    /// The Unix mode, or `None` for an MS-DOS host.
    pub mode: Option<u32>,
    pub flags: u16,
    /// Stored (0) or Deflate (8), which `finish` compresses; any other
    /// method stores the data as is.
    pub method: u16,
    /// The packed bytes, if they should not be `data` encoded by `method`.
    pub packed: Option<Vec<u8>>,
    /// 2024-01-01 12:00:00.
    pub date: u16,
    pub time: u16,
    pub central_extra: Vec<u8>,
    /// The local header's name, if it should differ from the central one.
    pub local_name: Option<Vec<u8>>,
    /// The CRC in both headers, if it should not be `data`'s.
    pub crc: Option<u32>,
    /// The local header's CRC, if it should differ.
    pub local_crc: Option<u32>,
    /// Writes zero sizes and CRC in the local header and a data descriptor
    /// after the data.
    pub data_descriptor: bool,
}

impl Entry {
    pub fn new(name: &[u8], mode: Option<u32>, data: &[u8]) -> Self {
        Self {
            name: name.to_vec(),
            data: data.to_vec(),
            mode,
            flags: 0,
            method: 0,
            packed: None,
            date: (44 << 9) | (1 << 5) | 1,
            time: 12 << 11,
            central_extra: Vec::new(),
            local_name: None,
            crc: None,
            local_crc: None,
            data_descriptor: false,
        }
    }
}

/// An archive being written.
#[derive(Default)]
pub struct Writer {
    pub entries: Vec<Entry>,
    /// Writes ZIP64 records and extra fields throughout.
    pub zip64: bool,
    /// Deflates every file and symlink added from now on.
    pub deflate: bool,
    pub comment: Vec<u8>,
}

impl Writer {
    pub fn entry(&mut self, entry: Entry) -> &mut Self {
        self.entries.push(entry);
        self
    }

    pub fn file(&mut self, name: &[u8], data: &[u8]) -> &mut Self {
        let method = if self.deflate { 8 } else { 0 };
        self.entry(Entry::new(name, Some(FILE), data)).last().method = method;
        self
    }

    pub fn dir(&mut self, name: &[u8]) -> &mut Self {
        self.entry(Entry::new(name, Some(DIR), b""))
    }

    pub fn symlink(&mut self, name: &[u8], target: &[u8]) -> &mut Self {
        let method = if self.deflate { 8 } else { 0 };
        self.entry(Entry::new(name, Some(SYMLINK), target))
            .last()
            .method = method;
        self
    }

    pub fn fifo(&mut self, name: &[u8]) -> &mut Self {
        self.entry(Entry::new(name, Some(FIFO), b""))
    }

    /// Returns the last member added, to tamper with.
    pub fn last(&mut self) -> &mut Entry {
        self.entries.last_mut().unwrap()
    }

    pub fn zip64(&mut self) -> &mut Self {
        self.zip64 = true;
        self
    }

    pub fn comment(&mut self, comment: &[u8]) -> &mut Self {
        self.comment = comment.to_vec();
        self
    }

    /// Writes the archive and returns its bytes.
    pub fn finish(&mut self) -> Vec<u8> {
        let mut out = Vec::new();
        let mut offsets = Vec::new();
        let packed: Vec<Vec<u8>> = self.entries.iter().map(packed).collect();
        for (entry, packed) in self.entries.iter().zip(&packed) {
            offsets.push(out.len() as u64);
            let crc = entry.crc.unwrap_or_else(|| crc32(&entry.data));
            let size = entry.data.len() as u64;
            let compressed = packed.len() as u64;
            let name = entry.local_name.as_ref().unwrap_or(&entry.name);
            let (local_crc, local_size, local_compressed) = if entry.data_descriptor {
                (0, 0, 0)
            } else {
                (entry.local_crc.unwrap_or(crc), size, compressed)
            };
            let mut extra = Vec::new();
            if self.zip64 {
                extra.extend(u16s(&[0x0001, 16]));
                extra.extend(local_size.to_le_bytes());
                extra.extend(local_compressed.to_le_bytes());
            }
            out.extend(0x0403_4b50_u32.to_le_bytes());
            out.extend(u16s(&[20, self.flags(entry), entry.method]));
            out.extend(u16s(&[entry.time, entry.date]));
            out.extend(local_crc.to_le_bytes());
            out.extend(self.narrow(local_compressed).to_le_bytes());
            out.extend(self.narrow(local_size).to_le_bytes());
            out.extend(u16s(&[name.len() as u16, extra.len() as u16]));
            out.extend_from_slice(name);
            out.extend(extra);
            out.extend_from_slice(packed);
            if entry.data_descriptor {
                out.extend(0x0807_4b50_u32.to_le_bytes());
                out.extend(crc.to_le_bytes());
                out.extend((compressed as u32).to_le_bytes());
                out.extend((size as u32).to_le_bytes());
            }
        }

        let cd_offset = out.len() as u64;
        for ((entry, &offset), packed) in self.entries.iter().zip(&offsets).zip(&packed) {
            let size = entry.data.len() as u64;
            let compressed = packed.len() as u64;
            let mut extra = Vec::new();
            if self.zip64 {
                extra.extend(u16s(&[0x0001, 24]));
                extra.extend(size.to_le_bytes());
                extra.extend(compressed.to_le_bytes());
                extra.extend(offset.to_le_bytes());
            }
            extra.extend_from_slice(&entry.central_extra);
            let (made_by, external) = match entry.mode {
                Some(mode) => {
                    let dos = if mode & 0o170_000 == DIR & 0o170_000 {
                        0x10
                    } else {
                        0
                    };
                    ((3 << 8) | 20, (mode << 16) | dos)
                },
                None => (20, if entry.name.ends_with(b"/") { 0x10 } else { 0 }),
            };
            out.extend(0x0201_4b50_u32.to_le_bytes());
            out.extend(u16s(&[made_by, 20, self.flags(entry), entry.method]));
            out.extend(u16s(&[entry.time, entry.date]));
            out.extend(
                entry
                    .crc
                    .unwrap_or_else(|| crc32(&entry.data))
                    .to_le_bytes(),
            );
            out.extend(self.narrow(compressed).to_le_bytes());
            out.extend(self.narrow(size).to_le_bytes());
            out.extend(u16s(&[
                entry.name.len() as u16,
                extra.len() as u16,
                0,
                0,
                0,
            ]));
            out.extend(external.to_le_bytes());
            out.extend(self.narrow(offset).to_le_bytes());
            out.extend_from_slice(&entry.name);
            out.extend(extra);
        }

        let cd_size = out.len() as u64 - cd_offset;
        let entries = self.entries.len() as u64;
        if self.zip64 {
            let eocd64 = out.len() as u64;
            out.extend(0x0606_4b50_u32.to_le_bytes());
            out.extend(44_u64.to_le_bytes());
            out.extend(u16s(&[45, 45]));
            out.extend([0; 8]);
            for n in [entries, entries, cd_size, cd_offset] {
                out.extend(n.to_le_bytes());
            }
            out.extend(0x0706_4b50_u32.to_le_bytes());
            out.extend(0_u32.to_le_bytes());
            out.extend(eocd64.to_le_bytes());
            out.extend(1_u32.to_le_bytes());
        }
        let count = if self.zip64 { 0xFFFF } else { entries as u16 };
        out.extend(0x0605_4b50_u32.to_le_bytes());
        out.extend(u16s(&[0, 0, count, count]));
        out.extend(self.narrow(cd_size).to_le_bytes());
        out.extend(self.narrow(cd_offset).to_le_bytes());
        out.extend((self.comment.len() as u16).to_le_bytes());
        out.extend_from_slice(&self.comment);
        out
    }

    fn flags(&self, entry: &Entry) -> u16 {
        entry.flags | if entry.data_descriptor { 1 << 3 } else { 0 }
    }

    /// Returns a 32-bit field, or the ZIP64 sentinel.
    fn narrow(&self, n: u64) -> u32 {
        if self.zip64 { u32::MAX } else { n as u32 }
    }
}

/// Returns an entry's packed bytes.
fn packed(entry: &Entry) -> Vec<u8> {
    match (&entry.packed, entry.method) {
        (Some(packed), _) => packed.clone(),
        (None, 8) => miniz_oxide::deflate::compress_to_vec(&entry.data, 6),
        (None, _) => entry.data.clone(),
    }
}

fn u16s(values: &[u16]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

/// Returns the CRC-32 (ISO-HDLC) of `data`.
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0_u32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xEDB8_8320 & (crc & 1).wrapping_neg());
        }
    }
    !crc
}

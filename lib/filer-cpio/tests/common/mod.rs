// This is free and unencumbered software released into the public domain.

//! A minimal cpio writer for tests, in each format, built by hand so that
//! tests can also write malformed and hostile archives.

// Each test binary compiles this module and uses only part of it.
#![allow(dead_code)]

/// The formats an archive can be written in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Newc,
    /// `newc` with checksums.
    Crc,
    Odc,
    Binary {
        big: bool,
    },
}

pub const FORMATS: [Format; 5] = [
    Format::Newc,
    Format::Crc,
    Format::Odc,
    Format::Binary { big: false },
    Format::Binary { big: true },
];

pub const DIR: u32 = 0o040_755;
pub const FILE: u32 = 0o100_644;
pub const SYMLINK: u32 = 0o120_777;
pub const FIFO: u32 = 0o010_644;

/// An archive being written.
pub struct Writer {
    pub format: Format,
    pub bytes: Vec<u8>,
    ino: u32,
}

impl Writer {
    pub fn new(format: Format) -> Self {
        Self {
            format,
            bytes: Vec::new(),
            ino: 0,
        }
    }

    pub fn dir(&mut self, name: &[u8]) -> &mut Self {
        let ino = self.next_ino();
        self.entry(name, DIR, ino, 2, b"")
    }

    pub fn file(&mut self, name: &[u8], data: &[u8]) -> &mut Self {
        let ino = self.next_ino();
        self.entry(name, FILE, ino, 1, data)
    }

    pub fn symlink(&mut self, name: &[u8], target: &[u8]) -> &mut Self {
        let ino = self.next_ino();
        self.entry(name, SYMLINK, ino, 1, target)
    }

    pub fn fifo(&mut self, name: &[u8]) -> &mut Self {
        let ino = self.next_ino();
        self.entry(name, FIFO, ino, 1, b"")
    }

    /// Appends a file with several names: in `newc`, with its data only
    /// in the last, as GNU cpio writes it; otherwise in each.
    pub fn hardlinks(&mut self, names: &[&[u8]], data: &[u8]) -> &mut Self {
        let ino = self.next_ino();
        let nlink = names.len() as u32;
        for (i, name) in names.iter().enumerate() {
            let last = i + 1 == names.len();
            let newc = matches!(self.format, Format::Newc | Format::Crc);
            let data = if newc && !last { &b""[..] } else { data };
            self.entry(name, FILE, ino, nlink, data);
        }
        self
    }

    fn next_ino(&mut self) -> u32 {
        self.ino += 1;
        self.ino
    }

    /// Appends a member with these fields, its name, and its data.
    pub fn entry(
        &mut self,
        name: &[u8],
        mode: u32,
        ino: u32,
        nlink: u32,
        data: &[u8],
    ) -> &mut Self {
        let mut name = name.to_vec();
        name.push(0);
        let (len, size) = (name.len() as u32, data.len() as u32);
        let check: u32 = data.iter().map(|&b| u32::from(b)).sum();
        match self.format {
            Format::Newc | Format::Crc => {
                let magic = if self.format == Format::Crc {
                    "070702"
                } else {
                    "070701"
                };
                let check = if self.format == Format::Crc { check } else { 0 };
                let fields = [
                    ino,
                    mode,
                    0,
                    0,
                    nlink,
                    1_700_000_000,
                    size,
                    8,
                    1,
                    0,
                    0,
                    len,
                    check,
                ];
                let header: String = fields.iter().map(|n| format!("{n:08X}")).collect();
                self.bytes.extend(format!("{magic}{header}").as_bytes());
                self.bytes.extend(&name);
                self.pad(4);
                self.bytes.extend(data);
                self.pad(4);
            },
            Format::Odc => {
                let header = format!(
                    "070707{:06o}{ino:06o}{mode:06o}{:06o}{:06o}{nlink:06o}{:06o}{:011o}{len:06o}{size:011o}",
                    8, 0, 0, 0, 1_700_000_000
                );
                self.bytes.extend(header.as_bytes());
                self.bytes.extend(&name);
                self.bytes.extend(data);
            },
            Format::Binary { big } => {
                let mtime = 1_700_000_000_u32;
                let words = [
                    0o070707,
                    8,
                    ino as u16,
                    mode as u16,
                    0,
                    0,
                    nlink as u16,
                    0,
                    (mtime >> 16) as u16,
                    mtime as u16,
                    len as u16,
                    (size >> 16) as u16,
                    size as u16,
                ];
                for word in words {
                    let bytes = if big {
                        word.to_be_bytes()
                    } else {
                        word.to_le_bytes()
                    };
                    self.bytes.extend(bytes);
                }
                self.bytes.extend(&name);
                self.pad(2);
                self.bytes.extend(data);
                self.pad(2);
            },
        }
        self
    }

    fn pad(&mut self, alignment: usize) {
        let len = self.bytes.len().next_multiple_of(alignment);
        self.bytes.resize(len, 0);
    }

    /// Ends the archive with its trailer, padded to 512 bytes, and returns
    /// its bytes.
    pub fn finish(&mut self) -> Vec<u8> {
        self.entry(b"TRAILER!!!", 0, 0, 1, b"");
        self.pad(512);
        std::mem::take(&mut self.bytes)
    }
}

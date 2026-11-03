// This is free and unencumbered software released into the public domain.

//! A minimal ar writer for tests, in each dialect of names, built by hand
//! so that tests can also write malformed and hostile archives.

// Each test binary compiles this module and uses only part of it.
#![allow(dead_code)]

/// How an archive stores names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dialect {
    /// Short names only, padded with spaces, as Debian writes them.
    SysV,
    /// Short names ending with `/`, longer ones in a `//` table.
    Gnu,
    /// Short names, and `#1/` names stored before the data.
    Bsd,
}

pub const DIALECTS: [Dialect; 3] = [Dialect::SysV, Dialect::Gnu, Dialect::Bsd];

/// An archive being written: its members, as raw name fields and data.
pub struct Writer {
    dialect: Dialect,
    table: Vec<u8>,
    members: Vec<([u8; 16], Vec<u8>)>,
}

impl Writer {
    pub fn new(dialect: Dialect) -> Self {
        Self {
            dialect,
            table: Vec::new(),
            members: Vec::new(),
        }
    }

    /// Appends the file `name` with `data`, naming it as the dialect does.
    pub fn file(&mut self, name: &[u8], data: &[u8]) -> &mut Self {
        let short = name.len() < 16 && !name.ends_with(b" ") && !name.starts_with(b"#1/");
        match self.dialect {
            Dialect::SysV => {
                assert!(short && !name.ends_with(b"/"), "{name:?} needs a long name");
                self.raw(name, data)
            },
            Dialect::Gnu if short => self.raw(&[name, b"/"].concat(), data),
            Dialect::Gnu => {
                let field = format!("/{}", self.table.len());
                self.table.extend_from_slice(name);
                self.table.extend_from_slice(b"/\n");
                self.raw(field.as_bytes(), data)
            },
            Dialect::Bsd if short && !name.contains(&b' ') => self.raw(name, data),
            Dialect::Bsd => {
                // Padded with NULs to a multiple of 4, as BSD does.
                let mut stored = name.to_vec();
                stored.resize(name.len().next_multiple_of(4), 0);
                let field = format!("#1/{}", stored.len());
                stored.extend_from_slice(data);
                self.raw(field.as_bytes(), &stored)
            },
        }
    }

    /// Appends a member whose name field is `field`, padded with spaces.
    pub fn raw(&mut self, field: &[u8], data: &[u8]) -> &mut Self {
        let mut name = [b' '; 16];
        name[..field.len()].copy_from_slice(field);
        self.members.push((name, data.to_vec()));
        self
    }

    /// Returns the archive's bytes: the magic number, the GNU name table
    /// if any, then the members.
    pub fn finish(&mut self) -> Vec<u8> {
        let mut bytes = b"!<arch>\n".to_vec();
        if !self.table.is_empty() {
            member(&mut bytes, b"//", &self.table);
        }
        for (name, data) in &self.members {
            member(&mut bytes, name, data);
        }
        bytes
    }
}

/// Appends a member with the name field `name` and `data`.
pub fn member(bytes: &mut Vec<u8>, name: &[u8], data: &[u8]) {
    bytes.extend(header(name, data.len() as u64));
    bytes.extend_from_slice(data);
    if data.len() % 2 == 1 {
        bytes.push(b'\n');
    }
}

/// Returns a header for a member named `name`, of `size` bytes.
pub fn header(name: &[u8], size: u64) -> [u8; 60] {
    let mut header = [b' '; 60];
    header[..name.len()].copy_from_slice(name);
    header[16..26].copy_from_slice(b"1700000000");
    header[28..29].copy_from_slice(b"0");
    header[34..35].copy_from_slice(b"0");
    header[40..46].copy_from_slice(b"100644");
    let size = size.to_string();
    header[48..48 + size.len()].copy_from_slice(size.as_bytes());
    header[58..].copy_from_slice(b"`\n");
    header
}

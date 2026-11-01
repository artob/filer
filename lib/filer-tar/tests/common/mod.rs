// This is free and unencumbered software released into the public domain.

//! A minimal TAR writer for tests: POSIX ustar headers, built by hand so
//! that tests can also write malformed and hostile archives.

// Each test binary compiles this module and uses only part of it.
#![allow(dead_code)]

/// An archive being written.
#[derive(Default)]
pub struct Writer {
    pub bytes: Vec<u8>,
}

impl Writer {
    /// Appends a member with `name`, `typeflag`, `linkname`, and `data`.
    pub fn member(&mut self, name: &[u8], typeflag: u8, linkname: &[u8], data: &[u8]) -> &mut Self {
        let mut header = header(name, typeflag, linkname, data.len() as u64);
        seal(&mut header);
        self.bytes.extend_from_slice(&header);
        self.bytes.extend_from_slice(data);
        let padding = data.len().div_ceil(512) * 512 - data.len();
        self.bytes.extend(std::iter::repeat_n(0, padding));
        self
    }

    pub fn dir(&mut self, name: &[u8]) -> &mut Self {
        self.member(name, b'5', b"", b"")
    }

    pub fn file(&mut self, name: &[u8], data: &[u8]) -> &mut Self {
        self.member(name, b'0', b"", data)
    }

    pub fn symlink(&mut self, name: &[u8], target: &[u8]) -> &mut Self {
        self.member(name, b'2', target, b"")
    }

    /// Appends a PAX extended header applying `records` to the next member.
    pub fn pax(&mut self, records: &[(&str, &[u8])]) -> &mut Self {
        let mut data = Vec::new();
        for (key, value) in records {
            // The length counts itself, so find the fixed point.
            let body = 1 + key.len() + 1 + value.len() + 1;
            let mut len = body + 1;
            while len != body + len.to_string().len() {
                len = body + len.to_string().len();
            }
            data.extend_from_slice(format!("{len} {key}=").as_bytes());
            data.extend_from_slice(value);
            data.push(b'\n');
        }
        self.member(b"PaxHeader", b'x', b"", &data)
    }

    /// Ends the archive with two zero blocks and returns its bytes.
    pub fn finish(&mut self) -> Vec<u8> {
        self.bytes.extend([0; 1024]);
        std::mem::take(&mut self.bytes)
    }
}

/// Returns an unsealed header (checksum not yet computed).
pub fn header(name: &[u8], typeflag: u8, linkname: &[u8], size: u64) -> [u8; 512] {
    let mut header = [0_u8; 512];
    header[..name.len()].copy_from_slice(name);
    header[100..108].copy_from_slice(b"0000644\0");
    header[108..116].copy_from_slice(b"0000000\0");
    header[116..124].copy_from_slice(b"0000000\0");
    header[124..136].copy_from_slice(format!("{size:011o}\0").as_bytes());
    header[136..148].copy_from_slice(b"14707761400\0");
    header[156] = typeflag;
    header[157..157 + linkname.len()].copy_from_slice(linkname);
    header[257..263].copy_from_slice(b"ustar\0");
    header[263..265].copy_from_slice(b"00");
    header
}

/// Computes and stores the header checksum.
pub fn seal(header: &mut [u8; 512]) {
    header[148..156].copy_from_slice(b"        ");
    let sum: u32 = header.iter().map(|&b| u32::from(b)).sum();
    header[148..156].copy_from_slice(format!("{sum:06o}\0 ").as_bytes());
}

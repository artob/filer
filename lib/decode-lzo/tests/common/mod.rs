// This is free and unencumbered software released into the public domain.

//! What the tests and fuzzers share: a check that the decoder decodes the
//! same in pieces as whole; another decoder of blocks to check it
//! against; and lzop files built by hand.

// Each test binary compiles this module and uses only part of it.
#![allow(dead_code)]

use decode_lzo::{Decoder, Error};

/// The most a check decodes of any input.
pub const CAP: usize = 32 << 20;

/// Decodes `input`, giving the decoder at most `piece` bytes of input and
/// room for as many of output at a time, up to `CAP` in all, and then
/// [`finish`](Decoder::finish)es it.
pub fn decode(input: &[u8], piece: usize) -> Result<Vec<u8>, Error> {
    let mut decoder = Decoder::new();
    let mut output = Vec::new();
    let mut buf = vec![0; piece];
    let mut at = 0;
    loop {
        let end = input.len().min(at + piece);
        let room = piece.min(CAP - output.len());
        let (consumed, produced) = decoder.decode(&input[at..end], &mut buf[..room])?;
        assert!(
            consumed <= end - at && produced <= room,
            "broke its contract"
        );
        at += consumed;
        output.extend_from_slice(&buf[..produced]);
        if consumed == 0 && produced == 0 {
            break;
        }
    }
    decoder.finish()?;
    Ok(output)
}

/// Decodes `input` whole into room for exactly `len` bytes, what it
/// decodes to, and then [`finish`](Decoder::finish)es it: a stream must end
/// though its last byte fills the room, and what follows, such as a check
/// or a count of none more, is still to be read.
pub fn exactly(input: &[u8], len: usize) -> Result<Vec<u8>, Error> {
    let (mut decoder, mut output) = (Decoder::new(), vec![0; len]);
    let (mut at, mut written) = (0, 0);
    loop {
        let (consumed, produced) = decoder.decode(&input[at..], &mut output[written..])?;
        (at, written) = (at + consumed, written + produced);
        if (consumed, produced) == (0, 0) {
            break;
        }
    }
    decoder.finish()?;
    output.truncate(written);
    Ok(output)
}

/// Decodes `input` whole and in pieces of `piece` bytes, checks that both
/// agree, and returns what it decoded.
///
/// # Panics
///
/// If they do not agree.
pub fn agree(input: &[u8], piece: usize) -> Result<Vec<u8>, Error> {
    let whole = decode(input, input.len().max(1 << 20));
    let pieces = decode(input, piece);
    match (&whole, &pieces) {
        (Ok(whole), Ok(pieces)) => {
            assert!(whole == pieces, "decoded otherwise in pieces of {piece}")
        },
        (Err(whole), Err(pieces)) => assert_eq!(whole, pieces, "in pieces of {piece}"),
        _ => panic!("decoded whole: {whole:?}; in pieces of {piece}: {pieces:?}"),
    }
    if let Ok(data) = &whole {
        let exact = exactly(input, data.len());
        assert!(
            exact.as_ref() == whole.as_ref(),
            "decoded otherwise into room for exactly {} bytes: {:?}",
            data.len(),
            exact.as_ref().map(|_| ()),
        );
    }
    whole
}

/// Whether `block` names its version, as `lzokay-native`'s never do, and
/// it does not read: its first byte 17, and five bytes or more.
pub fn names_a_version(block: &[u8]) -> bool {
    block.len() >= 5 && block.first() == Some(&17)
}

/// Decodes `block` through `lzokay-native`, if it decodes to at most
/// `max` bytes, and reads it to its end: bytes after the end mark, which
/// it ignores, are refused, as the Linux kernel refuses them. It panics on
/// some malformed blocks, which are refused too.
pub fn lzokay(block: &[u8], max: usize) -> Option<Vec<u8>> {
    quiet();
    let decode = || {
        let mut reader = std::io::Cursor::new(block);
        let output = lzokay_native::decompress(&mut reader, None).ok()?;
        let whole = reader.position() == block.len() as u64;
        (whole && output.len() <= max).then_some(output)
    };
    std::panic::catch_unwind(decode).ok().flatten()
}

/// Silences the panics of `lzokay-native`'s, which [`lzokay`] catches,
/// and only those.
fn quiet() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let report = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let theirs = info
                .location()
                .is_some_and(|at| at.file().contains("lzokay-native"));
            if !theirs {
                report(info);
            }
        }));
    });
}

/// The header flags of a file's checksums: of what blocks decode to,
/// and of what compressed ones hold, Adler-32s and CRC-32s; of the
/// header, a CRC-32 rather than an Adler-32.
pub const ADLER32_D: u32 = 0x0001;
pub const ADLER32_C: u32 = 0x0002;
pub const CRC32_D: u32 = 0x0100;
pub const CRC32_C: u32 = 0x0200;
pub const HEADER_CRC32: u32 = 0x1000;

/// An lzop file's header, as lzop 1.04 writes it, but for what a test
/// changes.
#[derive(Clone)]
pub struct Header {
    pub version: u16,
    pub needed: u16,
    pub method: u8,
    pub flags: u32,
    pub name: Vec<u8>,
    pub extra: Option<Vec<u8>>,
}

impl Default for Header {
    fn default() -> Self {
        Self {
            version: 0x1040,
            needed: 0x0940,
            method: 1,
            flags: 0x0300_0000 | ADLER32_D,
            name: b"name".to_vec(),
            extra: None,
        }
    }
}

impl Header {
    /// Its bytes, checked by an Adler-32 or a CRC-32, as its flags say.
    pub fn bytes(&self) -> Vec<u8> {
        let newer = self.version >= 0x0940;
        let mut fields = self.version.to_be_bytes().to_vec();
        fields.extend_from_slice(&0x20A0_u16.to_be_bytes());
        if newer {
            fields.extend_from_slice(&self.needed.to_be_bytes());
        }
        fields.push(self.method);
        if newer {
            fields.push(5);
        }
        let flags = self.flags | if self.extra.is_some() { 0x40 } else { 0 };
        fields.extend_from_slice(&flags.to_be_bytes());
        if flags & 0x800 != 0 {
            fields.extend_from_slice(&1_u32.to_be_bytes());
        }
        // The mode, and the time.
        fields.extend_from_slice(&0o100_644_u32.to_be_bytes());
        fields.extend_from_slice(&0x6AEC_2A40_u32.to_be_bytes());
        if newer {
            fields.extend_from_slice(&0_u32.to_be_bytes());
        }
        fields.push(self.name.len() as u8);
        fields.extend_from_slice(&self.name);
        let sum = |bytes: &[u8]| {
            if flags & HEADER_CRC32 != 0 {
                compute_crc::crc32(bytes)
            } else {
                compute_adler32::adler32(bytes)
            }
        };
        let mut header = b"\x89LZO\0\r\n\x1A\n".to_vec();
        header.extend_from_slice(&fields);
        header.extend_from_slice(&sum(&fields).to_be_bytes());
        // The extra field's checksum covers its length and what it holds,
        // afresh: lzop 1.04 reads such a file, and refuses one whose
        // checksum carries on from the header's, or covers less.
        if let Some(extra) = &self.extra {
            let mut field = (extra.len() as u32).to_be_bytes().to_vec();
            field.extend_from_slice(extra);
            header.extend_from_slice(&field);
            header.extend_from_slice(&sum(&field).to_be_bytes());
        }
        header
    }
}

/// Returns a block of `data`, compressed by `lzokay-native` if `compress`
/// and that makes it smaller, and otherwise stored, as lzop does, with
/// the checksums that `flags` ask for.
pub fn block(flags: u32, data: &[u8], compress: bool) -> Vec<u8> {
    let mut packed = data.to_vec();
    if compress {
        let compressed = lzokay_native::compress(data).unwrap();
        if compressed.len() < data.len() {
            packed = compressed;
        }
    }
    let mut block = (data.len() as u32).to_be_bytes().to_vec();
    block.extend_from_slice(&(packed.len() as u32).to_be_bytes());
    let mut sums = vec![(ADLER32_D, data), (CRC32_D, data)];
    if packed.len() < data.len() {
        sums.extend([(ADLER32_C, &packed[..]), (CRC32_C, &packed[..])]);
    }
    for (kind, bytes) in sums {
        if flags & kind != 0 {
            let sum = if kind & (ADLER32_D | ADLER32_C) != 0 {
                compute_adler32::adler32(bytes)
            } else {
                compute_crc::crc32(bytes)
            };
            block.extend_from_slice(&sum.to_be_bytes());
        }
    }
    block.extend_from_slice(&packed);
    block
}

/// Returns a file of `header` and blocks of `blocks`, each compressed if
/// it says, and the end mark.
pub fn file(header: &Header, blocks: &[(bool, &[u8])]) -> Vec<u8> {
    let mut file = header.bytes();
    for &(compress, data) in blocks {
        file.extend_from_slice(&block(header.flags, data, compress));
    }
    file.extend_from_slice(&[0; 4]);
    file
}

/// `seq 1 3000`.
pub fn numbers() -> Vec<u8> {
    (1..=3000)
        .flat_map(|n| format!("{n}\n").into_bytes())
        .collect()
}

// This is free and unencumbered software released into the public domain.

//! Every decoder the harness measures, ours and the counterparts, driven
//! through one interface: a whole stream into a buffer, or a stream given
//! and taken in pieces of a size. Counterparts are run, never read.

use std::io::Read;

/// Why a decoder failed, as text, as each crate's errors differ.
pub type Failure = String;

/// Decodes `input` into `output`, given and taken in pieces of at most
/// the size it is given, or whole for `usize::MAX`; returns how many bytes
/// it wrote.
pub type Decode = fn(&[u8], &mut [u8], usize) -> Result<usize, Failure>;

/// A decoder, ours or a counterpart.
pub struct Decoder {
    /// Its name in tables.
    pub name: &'static str,
    /// The crate it is, as `Cargo.toml` names it, and its language.
    pub krate: &'static str,
    pub language: &'static str,
    pub decode: Decode,
}

/// A codec, and its decoders, ours first, then the reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Codec {
    Bzip2,
    Zstd,
}

impl Codec {
    pub const ALL: [Self; 2] = [Self::Bzip2, Self::Zstd];

    pub fn name(self) -> &'static str {
        match self {
            Self::Bzip2 => "bzip2",
            Self::Zstd => "zstd",
        }
    }

    pub fn decoders(self) -> &'static [Decoder] {
        match self {
            Self::Bzip2 => &BZIP2,
            Self::Zstd => &ZSTD,
        }
    }
}

const BZIP2: [Decoder; 4] = [
    Decoder {
        name: "ours",
        krate: "decode-bzip2",
        language: "Rust",
        decode: ours_bzip2,
    },
    Decoder {
        name: "libbzip2",
        krate: "bzip2-sys",
        language: "C",
        decode: libbzip2,
    },
    Decoder {
        name: "libbz2-rs-sys",
        krate: "libbz2-rs-sys",
        language: "Rust",
        decode: libbz2_rs,
    },
    Decoder {
        name: "bzip2-rs",
        krate: "bzip2-rs",
        language: "Rust",
        decode: bzip2_rs,
    },
];

const ZSTD: [Decoder; 3] = [
    Decoder {
        name: "ours",
        krate: "decode-zstd",
        language: "Rust",
        decode: ours_zstd,
    },
    Decoder {
        name: "libzstd",
        krate: "zstd",
        language: "C",
        decode: libzstd,
    },
    Decoder {
        name: "ruzstd",
        krate: "ruzstd",
        language: "Rust",
        decode: ruzstd,
    },
];

/// Drives one of our sans-I/O decoders, `step`, over `input` into
/// `output`, in pieces of at most `piece` bytes each way.
fn sans_io<E: std::fmt::Debug>(
    mut step: impl FnMut(&[u8], &mut [u8]) -> Result<(usize, usize), E>,
    input: &[u8],
    output: &mut [u8],
    piece: usize,
) -> Result<usize, Failure> {
    let (mut read, mut written) = (0_usize, 0_usize);
    loop {
        let into = input.len().min(read.saturating_add(piece));
        let room = output.len().min(written.saturating_add(piece));
        let (consumed, produced) = step(&input[read..into], &mut output[written..room])
            .map_err(|error| format!("{error:?}"))?;
        (read, written) = (read + consumed, written + produced);
        if (consumed, produced) == (0, 0) {
            return Ok(written);
        }
    }
}

fn ours_bzip2(input: &[u8], output: &mut [u8], piece: usize) -> Result<usize, Failure> {
    let mut decoder = decode_bzip2::Decoder::new();
    let written = sans_io(|i, o| decoder.decode(i, o), input, output, piece)?;
    decoder.finish().map_err(|error| error.to_string())?;
    Ok(written)
}

fn ours_zstd(input: &[u8], output: &mut [u8], piece: usize) -> Result<usize, Failure> {
    let mut decoder = decode_zstd::Decoder::new();
    let written = sans_io(|i, o| decoder.decode(i, o), input, output, piece)?;
    decoder.finish().map_err(|error| error.to_string())?;
    Ok(written)
}

/// Drives a `bzip2` crate's `Decompress`, of either version, as libbzip2's
/// own interface is driven: a stream, then what follows it, unread.
macro_rules! bzip2_decompress {
    ($name:ident, $krate:ident) => {
        fn $name(input: &[u8], output: &mut [u8], piece: usize) -> Result<usize, Failure> {
            let mut decoder = $krate::Decompress::new(false);
            loop {
                let (read, written) = (decoder.total_in() as usize, decoder.total_out() as usize);
                let into = input.len().min(read.saturating_add(piece));
                let room = output.len().min(written.saturating_add(piece));
                let status = decoder
                    .decompress(&input[read..into], &mut output[written..room])
                    .map_err(|error| error.to_string())?;
                if status == $krate::Status::StreamEnd {
                    return Ok(decoder.total_out() as usize);
                }
                let stuck = decoder.total_in() as usize == read;
                if stuck && decoder.total_out() as usize == written {
                    return Err("cut short".into());
                }
            }
        }
    };
}

bzip2_decompress!(libbzip2, bzip2_c);
bzip2_decompress!(libbz2_rs, libbz2_rs);

/// A reader that gives `bytes` in pieces of at most `piece` bytes, as a
/// file or a socket might.
struct Pieces<'a> {
    bytes: &'a [u8],
    piece: usize,
}

impl Read for Pieces<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let len = buf.len().min(self.piece).min(self.bytes.len());
        buf[..len].copy_from_slice(&self.bytes[..len]);
        self.bytes = &self.bytes[len..];
        Ok(len)
    }
}

/// Reads all `reader` gives into `output`, in pieces of at most `piece`
/// bytes.
fn read_into(mut reader: impl Read, output: &mut [u8], piece: usize) -> Result<usize, Failure> {
    let mut written = 0_usize;
    loop {
        let room = output.len().min(written.saturating_add(piece));
        match reader.read(&mut output[written..room]) {
            Ok(0) if written < output.len() => return Ok(written),
            Ok(0) => {
                // Full: anything more is more than the stream holds.
                let mut more = [0];
                return match reader.read(&mut more) {
                    Ok(0) => Ok(written),
                    Ok(_) => Err("decoded past its output".into()),
                    Err(error) => Err(error.to_string()),
                };
            },
            Ok(len) => written += len,
            Err(error) => return Err(error.to_string()),
        }
    }
}

fn bzip2_rs(input: &[u8], output: &mut [u8], piece: usize) -> Result<usize, Failure> {
    use bzip2_rs::decoder::{Decoder, ReadState, WriteState};
    let (mut decoder, mut rest, mut written) = (Decoder::new(), input, 0_usize);
    // It fails if asked to read into no room: once the output is full,
    // it reads into a byte of its own, which must stay empty. Version
    // 0.1.2 also refuses, as a truncated Huffman bitstream, streams of
    // random bytes that libbzip2 encodes and decodes, from about 100 KB:
    // so it is disqualified on them.
    let mut past = [0];
    loop {
        let room = output.len().min(written.saturating_add(piece));
        let into = match room > written {
            true => &mut output[written..room],
            false => &mut past[..],
        };
        match decoder.read(into) {
            Ok(ReadState::NeedsWrite(_)) => {
                // An empty write says the stream has ended.
                let given = &rest[..rest.len().min(piece)];
                match decoder.write(given) {
                    Ok(WriteState::Written(len)) => rest = &rest[len..],
                    Ok(WriteState::NeedsRead) => return Err("refused a write".into()),
                    Err(error) => return Err(error.to_string()),
                }
            },
            Ok(ReadState::Read(len)) if room > written => written += len,
            Ok(ReadState::Read(_)) => return Err("decoded past its output".into()),
            Ok(ReadState::Eof) => return Ok(written),
            Err(error) => return Err(error.to_string()),
        }
    }
}

fn libzstd(input: &[u8], output: &mut [u8], piece: usize) -> Result<usize, Failure> {
    use zstd::stream::raw::{InBuffer, Operation, OutBuffer};
    if piece == usize::MAX {
        return zstd::bulk::decompress_to_buffer(input, output).map_err(|error| error.to_string());
    }
    let mut decoder = zstd::stream::raw::Decoder::new().map_err(|error| error.to_string())?;
    let (mut read, mut written) = (0, 0);
    loop {
        let into = input.len().min(read + piece);
        let room = output.len().min(written + piece);
        let mut source = InBuffer::around(&input[read..into]);
        let mut sink = OutBuffer::around(&mut output[written..room]);
        let hint = decoder
            .run(&mut source, &mut sink)
            .map_err(|error| error.to_string())?;
        let (consumed, produced) = (source.pos(), sink.pos());
        (read, written) = (read + consumed, written + produced);
        // A frame ended, and the input with it.
        if hint == 0 && read == input.len() {
            return Ok(written);
        }
        if (consumed, produced) == (0, 0) {
            return Err("cut short".into());
        }
    }
}

fn ruzstd(input: &[u8], output: &mut [u8], piece: usize) -> Result<usize, Failure> {
    if piece == usize::MAX {
        let mut decoder = ruzstd::decoding::FrameDecoder::new();
        return decoder
            .decode_all(input, output)
            .map_err(|error| error.to_string());
    }
    let source = Pieces {
        bytes: input,
        piece,
    };
    let decoder = ruzstd::decoding::StreamingDecoder::new(source).map_err(|e| e.to_string())?;
    read_into(decoder, output, piece)
}

// This is free and unencumbered software released into the public domain.

//! zstd frames, decoded here, from RFC 8878 alone (`LEGAL.md`): the
//! decoder of our own that is to replace `ruzstd` behind [`Zstd`] (D61,
//! TODO PACK-21 to 24).
//!
//! [`Zstd`]: super::Zstd

use super::{
    MAGIC, MAX_BLOCK, SKIPPABLE,
    block::{self, Tables},
    corrupt, unsupported,
    xxh64::Xxh64,
};
use crate::Decoder;
use alloc::{boxed::Box, vec::Vec};
use filer_api::Error;

/// The largest window decoded, 128 MiB, the most zstd itself decodes by
/// default: a frame needing more is unsupported.
const MAX_WINDOW: u64 = 128 << 20;

/// A frame's header (RFC 8878 §3.1.1.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Header {
    /// How far back its blocks may copy from.
    window: usize,
    /// What it decodes to, if it says.
    size: Option<u64>,
    checksum: bool,
}

impl Header {
    /// Returns how long a header's dictionary identifier and content size
    /// are, from its descriptor.
    fn fields(descriptor: u8) -> (usize, usize) {
        let dictionary = match descriptor & 0x03 {
            0 => 0,
            1 => 1,
            2 => 2,
            _ => 4,
        };
        // A single segment always has a size.
        let size = match descriptor >> 6 {
            0 => usize::from(descriptor & 0x20 != 0),
            1 => 2,
            2 => 4,
            _ => 8,
        };
        (dictionary, size)
    }

    /// Returns how long a header is, magic number included, from its
    /// descriptor: a single segment has no window descriptor.
    fn len(descriptor: u8) -> usize {
        let (dictionary, size) = Self::fields(descriptor);
        5_usize
            .saturating_add(usize::from(descriptor & 0x20 == 0))
            .saturating_add(dictionary)
            .saturating_add(size)
    }

    /// Parses a whole header, magic number included.
    fn parse(header: &[u8]) -> Result<Self, Error> {
        let [_, _, _, _, descriptor, rest @ ..] = header else {
            return Err(corrupt());
        };
        if descriptor & 0x08 != 0 || header.len() != Self::len(*descriptor) {
            return Err(corrupt());
        }
        let single = descriptor & 0x20 != 0;
        let (window, rest) = match rest.split_first() {
            Some((&window, rest)) if !single => (Some(window), rest),
            _ => (None, rest),
        };
        let (dictionary, _) = Self::fields(*descriptor);
        let (dictionary, rest) = rest.split_at_checked(dictionary).ok_or_else(corrupt)?;
        // An identifier of zero names no dictionary.
        if dictionary.iter().any(|&byte| byte != 0) {
            return Err(unsupported());
        }
        let mut size = [0; 8];
        size.get_mut(..rest.len())
            .ok_or_else(corrupt)?
            .copy_from_slice(rest);
        let size = u64::from_le_bytes(size);
        let size = match rest.len() {
            0 => None,
            2 => Some(size.saturating_add(256)),
            _ => Some(size),
        };
        let window = match (window, size) {
            (Some(descriptor), _) => {
                let log = u32::from(descriptor >> 3).saturating_add(10);
                let base = 1_u64.checked_shl(log).ok_or_else(unsupported)?;
                base.saturating_add((base >> 3).saturating_mul(u64::from(descriptor & 0x07)))
            },
            // A single segment's window is its size.
            (None, Some(size)) => size,
            (None, None) => return Err(corrupt()),
        };
        if window > MAX_WINDOW {
            return Err(unsupported());
        }
        Ok(Self {
            window: usize::try_from(window).map_err(|_| unsupported())?,
            size,
            checksum: descriptor & 0x04 != 0,
        })
    }

    /// Returns the most a block of the frame may hold or decode to.
    fn max_block(&self) -> usize {
        self.window.min(MAX_BLOCK)
    }
}

/// What a frame has decoded: at least its window of the latest bytes,
/// which blocks copy from, and those not yet produced.
#[derive(Clone)]
struct Window {
    bytes: Vec<u8>,
    /// How many of `bytes` have been produced.
    produced: usize,
    size: usize,
}

impl Window {
    /// Drops the bytes produced and further back than the window, once
    /// they are as many again, so that each byte moves once at most.
    fn trim(&mut self) {
        let excess = self.bytes.len().saturating_sub(self.size);
        if excess >= self.size.max(MAX_BLOCK) {
            let excess = excess.min(self.produced);
            self.bytes.drain(..excess);
            self.produced = self.produced.saturating_sub(excess);
        }
    }

    /// Produces what it can into `output`, and returns how much.
    fn produce(&mut self, output: &mut [u8]) -> usize {
        let pending = self.bytes.get(self.produced..).unwrap_or_default();
        let n = pending.len().min(output.len());
        if let (Some(to), Some(from)) = (output.get_mut(..n), pending.get(..n)) {
            to.copy_from_slice(from);
        }
        self.produced = self.produced.saturating_add(n);
        n
    }

    fn pending(&self) -> bool {
        self.produced < self.bytes.len()
    }
}

/// A frame being decoded.
#[derive(Clone)]
struct Frame {
    header: Header,
    window: Window,
    tables: Tables,
    /// What its blocks have decoded to so far, hashed and counted.
    hash: Xxh64,
    decoded: u64,
}

/// Where a [`Frames`] decoder is in its stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    /// Before a frame, or in its header, gathered so far.
    Frame,
    /// In a skippable frame, with this many bytes left.
    Skip(u32),
    /// Before a block of the frame, or in it, gathered so far.
    Block,
    /// Past the frame's last block, with what it decoded to left to
    /// produce.
    Last,
}

/// A decoder of zstd (RFC 8878): frames, one after another, each checked
/// against its size and checksum, if it has them; skippable frames are
/// read past.
///
/// It gathers each frame header and block whole before it decodes it,
/// unless the input holds it whole, so it holds at most a block of packed
/// bytes; and of what they decode to, the frame's window, as much again
/// or 128 KiB if more, and a block. A frame whose window passes 128 MiB,
/// the most zstd itself decodes by default, or that needs a dictionary
/// is [`Unsupported`](filer_api::ErrorKind::Unsupported). What follows
/// the last frame must be another, as zstd requires.
pub(crate) struct Frames {
    /// The packed bytes of a frame header or block, gathered until whole.
    unit: Vec<u8>,
    /// What a block's literals decode to, before its sequences run.
    literals: Vec<u8>,
    state: State,
    frame: Option<Frame>,
    /// The frames decoded whole, skippable ones included.
    frames: u64,
}

impl Frames {
    /// Returns a decoder at the start of a stream.
    pub(crate) const fn new() -> Self {
        Self {
            unit: Vec::new(),
            literals: Vec::new(),
            state: State::Frame,
            frame: None,
            frames: 0,
        }
    }

    /// Takes what it can of `input`, decoding a frame header or block once
    /// whole, and returns how much it consumed, or nothing if it can make
    /// no progress.
    fn step(&mut self, input: &[u8]) -> Result<Option<usize>, Error> {
        match self.state {
            State::Skip(left) => {
                let n = input.len().min(usize::try_from(left).unwrap_or(usize::MAX));
                match left.saturating_sub(u32::try_from(n).unwrap_or(u32::MAX)) {
                    0 => self.ended(),
                    left => self.state = State::Skip(left),
                }
                Ok((n > 0).then_some(n))
            },
            State::Frame | State::Block => {
                // Straight from the input, if it holds the unit whole.
                if self.unit.is_empty() {
                    let need = self.need(input)?;
                    if let Some(unit) = input.get(..need) {
                        self.unit(unit)?;
                        return Ok(Some(need));
                    }
                }
                let need = self.need(&self.unit)?;
                match need.checked_sub(self.unit.len()) {
                    Some(0) | None => {
                        let unit = core::mem::take(&mut self.unit);
                        let parsed = self.unit(&unit);
                        self.unit = unit;
                        self.unit.clear();
                        parsed.map(|()| Some(0))
                    },
                    Some(more) => {
                        let taken = input.get(..more.min(input.len())).unwrap_or_default();
                        self.unit.extend_from_slice(taken);
                        Ok((!taken.is_empty()).then_some(taken.len()))
                    },
                }
            },
            State::Last => Ok(None),
        }
    }

    /// Returns how many bytes the frame header or block that `head` begins
    /// takes in all, as far as `head` tells.
    fn need(&self, head: &[u8]) -> Result<usize, Error> {
        match (self.state, &self.frame) {
            (State::Block, Some(frame)) => {
                let Some(&[a, b, c]) = head.first_chunk() else {
                    return Ok(3);
                };
                let header = u32::from_le_bytes([a, b, c, 0]);
                let size = usize::try_from(header >> 3).unwrap_or(usize::MAX);
                if size > frame.header.max_block() {
                    return Err(corrupt());
                }
                // Raw and compressed blocks hold their size, and RLE
                // blocks one byte, repeated; the fourth type is reserved.
                let content = match (header >> 1) & 0x03 {
                    0 | 2 => size,
                    1 => 1,
                    _ => return Err(corrupt()),
                };
                let last = header & 1 != 0;
                let trailer = if last && frame.header.checksum { 4 } else { 0 };
                Ok(3_usize.saturating_add(content).saturating_add(trailer))
            },
            (State::Frame, _) => {
                let Some(magic) = head.first_chunk().copied().map(u32::from_le_bytes) else {
                    return Ok(4);
                };
                if SKIPPABLE.contains(&magic) {
                    return Ok(8);
                }
                if magic != MAGIC {
                    return Err(corrupt());
                }
                match head.get(4) {
                    Some(descriptor) if descriptor & 0x08 != 0 => Err(corrupt()),
                    Some(&descriptor) => Ok(Header::len(descriptor)),
                    None => Ok(5),
                }
            },
            _ => Err(corrupt()),
        }
    }

    /// Reads a whole frame header or block.
    fn unit(&mut self, unit: &[u8]) -> Result<(), Error> {
        if self.state == State::Block {
            return self.block(unit);
        }
        let magic = unit.first_chunk().copied().map(u32::from_le_bytes);
        if magic.is_some_and(|magic| SKIPPABLE.contains(&magic)) {
            let len = unit.get(4..8).and_then(|len| len.try_into().ok());
            match len.map(u32::from_le_bytes) {
                Some(0) => self.ended(),
                Some(len) => self.state = State::Skip(len),
                None => return Err(corrupt()),
            }
            return Ok(());
        }
        let header = Header::parse(unit)?;
        self.frame = Some(Frame {
            header,
            window: Window {
                bytes: Vec::new(),
                produced: 0,
                size: header.window,
            },
            tables: Tables::default(),
            hash: Xxh64::new(),
            decoded: 0,
        });
        self.state = State::Block;
        Ok(())
    }

    /// Decodes a whole block, header included, onto the frame's window.
    fn block(&mut self, unit: &[u8]) -> Result<(), Error> {
        let frame = self.frame.as_mut().ok_or_else(corrupt)?;
        let Some((&[a, b, c], content)) = unit.split_first_chunk() else {
            return Err(corrupt());
        };
        let header = u32::from_le_bytes([a, b, c, 0]);
        let last = header & 1 != 0;
        let size = usize::try_from(header >> 3).map_err(|_| corrupt())?;
        let (content, checksum) = if last && frame.header.checksum {
            content.split_last_chunk::<4>().ok_or_else(corrupt)?
        } else {
            (content, &[0; 4])
        };
        let window = &mut frame.window;
        window.trim();
        let start = window.bytes.len();
        match (header >> 1) & 0x03 {
            0 => window.bytes.extend_from_slice(content),
            1 => {
                let byte = content.first().copied().ok_or_else(corrupt)?;
                window.bytes.resize(start.saturating_add(size), byte);
            },
            _ => {
                let (reach, max) = (frame.header.window, frame.header.max_block());
                let (tables, scratch) = (&mut frame.tables, &mut self.literals);
                block::decode(content, tables, scratch, &mut window.bytes, reach, max)?;
            },
        }
        let block = window.bytes.get(start..).unwrap_or_default();
        frame.hash.update(block);
        let n = u64::try_from(block.len()).unwrap_or(u64::MAX);
        frame.decoded = frame.decoded.saturating_add(n);
        if frame.header.size.is_some_and(|size| frame.decoded > size) {
            return Err(corrupt());
        }
        if last {
            if frame.header.size.is_some_and(|size| frame.decoded != size) {
                return Err(corrupt());
            }
            // The low four bytes of the hash, little-endian.
            let digest = frame.hash.digest().to_le_bytes();
            if frame.header.checksum && digest.get(..4) != Some(checksum) {
                return Err(corrupt());
            }
            self.state = State::Last;
        }
        Ok(())
    }

    /// Returns whether it is between frames, past at least one.
    fn between(&self) -> bool {
        self.frames > 0 && self.state == State::Frame && self.unit.is_empty()
    }

    /// Ends the frame in progress.
    fn ended(&mut self) {
        self.frame = None;
        self.frames = self.frames.saturating_add(1);
        self.state = State::Frame;
    }
}

impl core::fmt::Debug for Frames {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Frames")
            .field("state", &self.state)
            .field("frames", &self.frames)
            .finish_non_exhaustive()
    }
}

impl Decoder for Frames {
    fn decode(&mut self, input: &[u8], output: &mut [u8]) -> Result<(usize, usize), Error> {
        let (mut consumed, mut produced) = (0_usize, 0_usize);
        let frames = self.frames;
        loop {
            if let Some(frame) = &mut self.frame {
                let room = output.get_mut(produced..).unwrap_or_default();
                produced = produced.saturating_add(frame.window.produce(room));
                if frame.window.pending() {
                    return Ok((consumed, produced));
                }
                if self.state == State::Last {
                    self.ended();
                }
            }
            // Stop between frames, where a checkpoint may be, once there
            // is progress to report.
            if self.frames != frames && (consumed > 0 || produced > 0) {
                return Ok((consumed, produced));
            }
            let Some(n) = self.step(input.get(consumed..).unwrap_or_default())? else {
                return Ok((consumed, produced));
            };
            consumed = consumed.saturating_add(n);
        }
    }

    fn finish(&mut self) -> Result<(), Error> {
        if self.between() {
            Ok(())
        } else {
            Err(corrupt())
        }
    }

    fn checkpoint(&self) -> Option<Box<dyn Decoder>> {
        if !self.between() {
            return None;
        }
        // Frames decode alone, so a fresh decoder goes on from here.
        let fresh: Box<dyn Decoder> = Box::new(Self {
            frames: self.frames,
            ..Self::new()
        });
        Some(fresh)
    }

    fn buffered(&self) -> usize {
        self.unit.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Zstd,
        common::{Block, agree, skippable, zstd},
    };
    use alloc::{sync::Arc, vec};
    use core::ops::Range;
    use filer_api::ErrorKind;
    use filer_spec::{
        Image, Outcome,
        fuzz::{Fuzz, Mutations},
    };

    /// Decodes `stream` whole and in pieces, and checks that `ruzstd`,
    /// behind [`Zstd`], decodes it alike, or decodes what it should not.
    fn decode(stream: &[u8]) -> Result<Vec<u8>, Error> {
        let ours = agree(Frames::new, stream, 7);
        let theirs = agree(Zstd::new, stream, 64 * 1024);
        match (&ours, &theirs) {
            (Ok(ours), Ok(theirs)) => assert!(ours == theirs, "decoded otherwise"),
            (Err(ours), Err(theirs)) => assert_eq!(ours.kind(), theirs.kind()),
            (Err(_), Ok(_)) if lax(stream) => {},
            _ => panic!("ours: {ours:?}; ruzstd's: {theirs:?}"),
        }
        ours
    }

    fn kind(stream: &[u8]) -> ErrorKind {
        decode(stream).unwrap_err().kind()
    }

    /// Returns a frame of `blocks`, as [`zstd`] builds it, with a checksum
    /// of what they decode to, and a single segment of that size if
    /// `single`.
    fn checked(blocks: &[Block<'_>], single: bool) -> Vec<u8> {
        let mut hash = Xxh64::new();
        let mut size = 0;
        for block in blocks {
            match *block {
                Block::Raw(bytes) => hash.update(bytes),
                Block::Rle(byte, n) => hash.update(&vec![byte; n]),
            }
            size += match *block {
                Block::Raw(bytes) => bytes.len(),
                Block::Rle(_, n) => n,
            };
        }
        let mut frame = zstd(blocks);
        frame[4] |= 0x04;
        if single {
            // Eight bytes of size in place of the window.
            frame[4] |= 0xE0;
            frame.splice(5..6, (size as u64).to_le_bytes());
        }
        frame.extend_from_slice(&hash.digest().to_le_bytes()[..4]);
        frame
    }

    /// `raw.zst` is by zstd 1.5.7, of random bytes it cannot compress: 5000
    /// with a 1 KiB window, in five raw blocks; 100 in a single segment;
    /// and 2000 from a pipe, with neither size nor checksum.
    #[test]
    fn decodes_raw_blocks_as_zstd_writes_them() {
        let stream = include_bytes!("../../tests/data/raw.zst");
        assert_eq!(decode(stream).unwrap().len(), 7100);
    }

    /// `literals.zst` is by zstd 1.5.7, of bytes drawn at random, unevenly,
    /// from an alphabet, and too short a window to repeat: so they code as
    /// literals alone, in one Huffman-coded stream or four, of each size,
    /// with trees whose weights are coded with FSE or not, or reused.
    #[test]
    fn decodes_literals_as_zstd_writes_them() {
        let stream = include_bytes!("../../tests/data/literals.zst");
        assert_eq!(decode(stream).unwrap().len(), 28_200);
    }

    /// `sequences.zst` is by zstd 1.5.7: text at levels 1, 19, and 22 with
    /// a 1 KiB window; data shaped as LZ77 makes it, its offsets often
    /// repeated, at 3, and at 19 with a 64 KiB window, which it outgrows;
    /// records alike but for a counter; and a short pattern, repeated. They
    /// take tables of each mode, and repeated offsets of each kind.
    #[test]
    fn decodes_sequences_as_zstd_writes_them() {
        let stream = include_bytes!("../../tests/data/sequences.zst");
        assert_eq!(decode(stream).unwrap().len(), 471_800);
        let numbers = include_bytes!("../../tests/data/numbers.zst");
        assert_eq!(decode(numbers).unwrap(), crate::common::numbers());
    }

    /// Decodes each `.zst` file in the directory `FILER_PACK_ZSTD_CORPUS`
    /// names, as `ruzstd` does: `zstd` output of a varied corpus, at many
    /// levels and windows, checks more than the fixtures can. It times
    /// both, in a release build.
    #[test]
    #[ignore = "needs a corpus of zstd streams"]
    fn decodes_a_corpus_as_ruzstd_does() {
        use std::time::{Duration, Instant};

        /// Decodes `stream` whole, however large, and times it.
        fn whole(mut decoder: impl Decoder, stream: &[u8]) -> (Result<Vec<u8>, Error>, Duration) {
            let start = Instant::now();
            let (mut output, mut at) = (Vec::new(), 0);
            let mut buf = vec![0; 1 << 20];
            let decoded = loop {
                match decoder.decode(&stream[at..], &mut buf) {
                    Ok((0, 0)) => break decoder.finish().map(|()| output),
                    Ok((consumed, produced)) => {
                        at += consumed;
                        output.extend_from_slice(&buf[..produced]);
                    },
                    Err(error) => break Err(error),
                }
            };
            (decoded, start.elapsed())
        }

        let corpus = std::env::var("FILER_PACK_ZSTD_CORPUS").unwrap();
        let mut paths: Vec<_> = std::fs::read_dir(corpus)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "zst"))
            .collect();
        paths.sort();
        let (mut bytes, mut times) = (0, [Duration::ZERO; 2]);
        for path in &paths {
            let stream = std::fs::read(path).unwrap();
            let (ours, time) = whole(Frames::new(), &stream);
            times[0] += time;
            let (theirs, time) = whole(Zstd::new(), &stream);
            times[1] += time;
            match (&ours, &theirs) {
                (Ok(ours), Ok(theirs)) => assert!(ours == theirs, "{}", path.display()),
                (Err(_), Err(_)) => {},
                (Err(_), Ok(_)) if lax(&stream) => {},
                _ => panic!(
                    "{}: ours {:?}; ruzstd's {:?}",
                    path.display(),
                    ours.as_ref().map(Vec::len),
                    theirs.as_ref().map(Vec::len)
                ),
            }
            bytes += ours.map_or(0, |data| data.len());
        }
        let [ours, theirs] = times;
        std::println!(
            "{} streams, {bytes} bytes: ours in {ours:?}, ruzstd's in {theirs:?}",
            paths.len()
        );
    }

    #[test]
    fn trims_the_window_once_it_doubles() {
        let mut window = Window {
            bytes: vec![1; 300 * 1024],
            produced: 300 * 1024,
            size: 64 * 1024,
        };
        window.trim();
        assert_eq!(
            (window.bytes.len(), window.produced),
            (64 * 1024, 64 * 1024)
        );
        // Not until it holds the most a block holds past the window, and
        // never what is yet to be produced.
        window.bytes.resize(191 * 1024, 2);
        window.trim();
        assert_eq!(window.bytes.len(), 191 * 1024);
        window.bytes.resize(250 * 1024, 2);
        window.trim();
        assert_eq!((window.bytes.len(), window.produced), (186 * 1024, 0));
    }

    #[test]
    fn decodes_literals_stored_and_repeated() {
        // A compressed block of `content`, alone in its frame.
        let block = |content: &[u8]| {
            let mut stream = zstd(&[Block::Raw(content)]);
            stream[6] |= 2 << 1;
            stream
        };
        // Five bytes stored, in each size format, then none repeated.
        for header in [
            &[5 << 3][..],
            &[1 << 2 | 5 << 4, 0],
            &[3 << 2 | 5 << 4, 0, 0],
        ] {
            let stream = block(&[header, b"hello", &[0]].concat());
            assert_eq!(decode(&stream).unwrap(), b"hello");
        }
        assert_eq!(decode(&block(&[1 | 5 << 3, b'z', 0])).unwrap(), b"zzzzz");
        // The sequences' count must end the block, as zstd checks.
        assert_eq!(kind(&block(&[5 << 3, 1, 2, 3, 4, 5])), ErrorKind::Corrupt);
        assert_eq!(kind(&block(&[1 << 3, 1, 0, 0])), ErrorKind::Corrupt);
        // More literals than a block may decode to, which zstd refuses:
        // 128 KiB and one, repeated, in 20 bits.
        let large = block(&[1 | 3 << 2 | 1 << 4, 0x00, 0x20, b'z', 0]);
        assert_eq!(kind(&large), ErrorKind::Corrupt);
    }

    #[test]
    fn decodes_rle_blocks_checked() {
        for single in [false, true] {
            let blocks = [
                Block::Rle(b'a', 100_000),
                Block::Raw(b"between"),
                Block::Rle(0, 1),
                Block::Rle(b'z', 128 * 1024),
            ];
            let stream = checked(&blocks, single);
            let data = decode(&stream).unwrap();
            assert_eq!(data.len(), 100_000 + 7 + 1 + 128 * 1024);
            let mut bad = stream.clone();
            *bad.last_mut().unwrap() ^= 1;
            assert_eq!(kind(&bad), ErrorKind::Corrupt);
        }
    }

    #[test]
    fn reads_sizes_of_each_length() {
        // One, two, four, and eight bytes of size, each in a single
        // segment, and two past a window; 300 bytes, but for one.
        for (descriptor, size, len) in [
            (0x20, vec![44], 44),
            (0x60, vec![44, 0], 300),
            (0xA0, vec![44, 1, 0, 0], 300),
            (0xE0, vec![44, 1, 0, 0, 0, 0, 0, 0], 300),
        ] {
            let data = vec![7; len];
            let mut stream = zstd(&[Block::Raw(&data)]);
            stream[4] = descriptor;
            stream.splice(5..6, size);
            assert_eq!(decode(&stream).unwrap(), data, "{descriptor:#x}");
            // A size other than what the frame decodes to.
            let mut short = stream.clone();
            short[5] -= 1;
            assert_eq!(kind(&short), ErrorKind::Corrupt, "{descriptor:#x}");
        }
        let data = vec![7; 300];
        let mut stream = zstd(&[Block::Raw(&data)]);
        stream[4] = 0x40;
        stream.splice(6..6, [44, 0]);
        assert_eq!(decode(&stream).unwrap(), data);
    }

    #[test]
    fn reads_past_skippable_frames() {
        let stream = [
            skippable(b"before"),
            zstd(&[Block::Raw(b"one")]),
            skippable(b""),
            checked(&[Block::Rle(b'2', 2)], true),
            skippable(b"last"),
        ]
        .concat();
        assert_eq!(decode(&stream).unwrap(), b"one22");
    }

    #[test]
    fn refuses_malformed_frames() {
        let good = include_bytes!("../../tests/data/raw.zst");
        let corrupt = |at: usize, byte: u8| {
            let mut stream = good.to_vec();
            stream[at] = byte;
            kind(&stream)
        };
        // The magic number, the reserved bit, and the checksum.
        assert_eq!(corrupt(0, 0x29), ErrorKind::Corrupt);
        assert_eq!(corrupt(4, good[4] | 0x08), ErrorKind::Corrupt);
        assert_eq!(corrupt(5023, good[5023] ^ 1), ErrorKind::Corrupt);
        // A reserved block type, and a block too large for 128 KiB.
        let mut reserved = zstd(&[Block::Raw(b"x")]);
        reserved[6] |= 0x06;
        assert_eq!(kind(&reserved), ErrorKind::Corrupt);
        let mut large = zstd(&[Block::Raw(&[0; 128 * 1024])]);
        large[8] += 1;
        assert_eq!(kind(&large), ErrorKind::Corrupt);
        // What follows the last frame must be one.
        let stream = [&good[..], b"garbage"].concat();
        assert_eq!(kind(&stream), ErrorKind::Corrupt);
        // Ending early, anywhere but between frames.
        for end in (0..good.len()).filter(|&end| ![5027, 5140].contains(&end)) {
            assert_eq!(kind(&good[..end]), ErrorKind::Corrupt, "{end}");
        }
    }

    /// Where a frame of a stream lies: a skippable frame's bytes; or a
    /// frame's header, its blocks, headers included, and its checksum.
    enum Span {
        Skippable(Range<usize>),
        Frame {
            header: Range<usize>,
            blocks: Vec<Range<usize>>,
            checksum: Range<usize>,
        },
    }

    /// Returns where the frames of `stream` lie, if they all parse.
    fn spans(stream: &[u8]) -> Option<Vec<Span>> {
        let (mut spans, mut at) = (Vec::new(), 0);
        while at < stream.len() {
            let magic = u32::from_le_bytes(stream.get(at..at + 4)?.try_into().ok()?);
            if SKIPPABLE.contains(&magic) {
                let len = u32::from_le_bytes(stream.get(at + 4..at + 8)?.try_into().ok()?);
                let end = at + 8 + len as usize;
                spans.push(Span::Skippable(at..end));
                at = end;
                continue;
            }
            let header = at..at + Header::len(*stream.get(at + 4)?);
            let parsed = Header::parse(stream.get(header.clone())?).ok()?;
            let (mut blocks, mut end) = (Vec::new(), header.end);
            loop {
                let &[a, b, c] = stream.get(end..end + 3)?.first_chunk()?;
                let block = u32::from_le_bytes([a, b, c, 0]);
                let content = if (block >> 1) & 3 == 1 {
                    1
                } else {
                    (block >> 3) as usize
                };
                blocks.push(end..end + 3 + content);
                end += 3 + content;
                if block & 1 != 0 {
                    break;
                }
            }
            let checksum = end..end + if parsed.checksum { 4 } else { 0 };
            stream.get(checksum.clone())?;
            at = checksum.end;
            spans.push(Span::Frame {
                header,
                blocks,
                checksum,
            });
        }
        Some(spans)
    }

    /// Returns `stream` without its frames' checksums, so that what a
    /// stream decodes to, not just whether it decodes, is compared.
    fn unchecked(stream: &[u8]) -> Vec<u8> {
        let mut unchecked = Vec::new();
        for span in spans(stream).unwrap() {
            match span {
                Span::Skippable(bytes) => unchecked.extend_from_slice(&stream[bytes]),
                Span::Frame { header, blocks, .. } => {
                    let at = unchecked.len();
                    unchecked.extend_from_slice(&stream[header]);
                    unchecked[at + 4] &= !0x04;
                    for block in blocks {
                        unchecked.extend_from_slice(&stream[block]);
                    }
                },
            }
        }
        unchecked
    }

    /// Returns whether `ruzstd` decodes a frame of `stream` that zstd
    /// refuses, as ours does: one whose content size disagrees with what
    /// it decodes to, or with a block that decodes to more than the frame
    /// allows. It decodes each frame block by block to tell.
    fn lax(stream: &[u8]) -> bool {
        for span in spans(stream).unwrap_or_default() {
            let Span::Frame {
                header,
                blocks,
                checksum,
            } = span
            else {
                continue;
            };
            let header = &stream[header];
            let mut decoder = ruzstd::decoding::FrameDecoder::new();
            let (Ok(parsed), Ok(())) = (Header::parse(header), decoder.reset(header)) else {
                return false;
            };
            let (mut output, mut decoded) = (vec![0; 1 << 18], 0);
            let last = blocks.len() - 1;
            for (i, block) in blocks.into_iter().enumerate() {
                // The last block's unit holds the checksum too.
                let end = if i == last { checksum.end } else { block.end };
                let unit = &stream[block.start..end];
                let Ok((_, written)) = decoder.decode_from_to(unit, &mut output) else {
                    return false;
                };
                if written > parsed.max_block() {
                    return true;
                }
                decoded += written as u64;
            }
            if parsed.size.is_some_and(|size| size != decoded) {
                return true;
            }
        }
        false
    }

    /// Mutates zstd's own streams, and checks that ours decodes each as
    /// `ruzstd` does, or refuses what zstd refuses and `ruzstd` does not.
    /// Their errors' kinds may differ.
    #[test]
    fn decodes_mutations_as_ruzstd_does() {
        const BYTES: [u8; 12] = [0, 1, 2, 4, 6, 0x08, 0x20, 0x28, 0x50, 0xC0, 0xFD, 0xFF];
        const WORDS: [u32; 6] = [0, 1, 0xFFFF, 0x184D_2A50, 0xFD2F_B528, u32::MAX];
        const MUTATIONS: Mutations = Mutations::new(&BYTES, &WORDS).aligned(false).most(2);
        let seeds = [
            include_bytes!("../../tests/data/raw.zst").to_vec(),
            include_bytes!("../../tests/data/literals.zst").to_vec(),
            unchecked(include_bytes!("../../tests/data/literals.zst")),
            include_bytes!("../../tests/data/sequences.zst").to_vec(),
            unchecked(include_bytes!("../../tests/data/sequences.zst")),
            unchecked(include_bytes!("../../tests/data/numbers.zst")),
            checked(&[Block::Rle(b'a', 3000), Block::Raw(b"bc")], false),
            [skippable(b"x"), checked(&[Block::Raw(b"yz")], true)].concat(),
        ]
        .map(Arc::new);
        Fuzz::new("FILER_PACK_FUZZ", 0x21).iterations(400).run(
            seeds.len(),
            |source, rng| {
                let seed = &seeds[source];
                let regions = [0..16, 0..seed.len() as u64];
                let input = MUTATIONS.mutate(seed, &regions, rng);
                let mut stream = vec![0; input.length() as usize];
                input.copy(0, &mut stream);
                (stream, 1 + rng.below(64) as usize)
            },
            |(stream, piece)| {
                let ours = agree(Frames::new, &stream, piece);
                let theirs = agree(Zstd::new, &stream, piece);
                match (&ours, &theirs) {
                    (Err(_), Ok(_)) if lax(&stream) => {},
                    (Ok(ours), Ok(theirs)) => assert!(ours == theirs, "decoded otherwise"),
                    // `ruzstd` finds some windows too large corrupt, which
                    // zstd and ours find unsupported.
                    (Err(_), Err(_)) => {},
                    _ => panic!(
                        "ours: {:?}; ruzstd's: {:?}; head {:02x?} of {}",
                        ours.as_ref().map(Vec::len),
                        theirs.as_ref().map(Vec::len),
                        &stream[..16],
                        stream.len()
                    ),
                }
                Outcome {
                    opened: ours.is_ok(),
                    entries: 1,
                    bytes: ours.as_ref().map_or(0, |data| data.len() as u64),
                    errors: usize::from(ours.is_err()),
                    ..Outcome::default()
                }
            },
        );
    }

    #[test]
    fn refuses_what_it_cannot_decode() {
        // A window of 256 MiB, past the 128 MiB that zstd decodes by default.
        let mut large = zstd(&[Block::Raw(b"x")]);
        large[5] = 18 << 3;
        assert_eq!(kind(&large), ErrorKind::Unsupported);
        // A dictionary.
        let mut dictionary = zstd(&[Block::Raw(b"x")]);
        dictionary[4] = 0x01;
        dictionary.insert(6, 7);
        assert_eq!(kind(&dictionary), ErrorKind::Unsupported);
    }
}

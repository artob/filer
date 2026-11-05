// This is free and unencumbered software released into the public domain.

use crate::{Error, block, corrupt, unsupported};
use alloc::vec::Vec;
use compute_adler32::adler32;
use compute_crc::crc32;

/// The bytes an lzop file begins with.
const MAGIC: [u8; 9] = [0x89, b'L', b'Z', b'O', 0x00, 0x0D, 0x0A, 0x1A, 0x0A];

/// A header's flags: Adler-32s of what each block decodes to, and of what
/// it holds, if compressed; an extra field; CRC-32s, likewise; parts of a
/// multipart archive; a filter; the header's checksum a CRC-32, rather
/// than an Adler-32; and the bits that mean nothing yet.
const ADLER32_D: u32 = 0x0001;
const ADLER32_C: u32 = 0x0002;
const EXTRA_FIELD: u32 = 0x0040;
const CRC32_D: u32 = 0x0100;
const CRC32_C: u32 = 0x0200;
const MULTIPART: u32 = 0x0400;
const FILTER: u32 = 0x0800;
const HEADER_CRC32: u32 = 0x1000;
const RESERVED: u32 = 0x000F_C000;

/// The oldest version of the format; the first whose headers hold the
/// version needed to extract, the level, and the time's high bits; and
/// the newest that this decoder reads, lzop 1.04's.
const OLDEST: u16 = 0x0900;
const NEWER: u16 = 0x0940;
const NEWEST: u16 = 0x1040;

/// The most a block may decode to, and a header's extra field hold, as
/// lzop allows.
const MAX_BLOCK: usize = 64 << 20;

/// Returns the big-endian word at `at` in `bytes`, if they hold it.
fn be16(bytes: &[u8], at: usize) -> Option<u16> {
    let word = bytes.get(at..)?.first_chunk()?;
    Some(u16::from_be_bytes(*word))
}

fn be32(bytes: &[u8], at: usize) -> Option<u32> {
    let word = bytes.get(at..)?.first_chunk()?;
    Some(u32::from_be_bytes(*word))
}

/// What a header's fields are where: those of a version from 0.94 on,
/// after the version needed to extract, or before.
struct Layout {
    method: usize,
    flags: usize,
    newer: bool,
}

impl Layout {
    fn of(version: u16) -> Self {
        let newer = version >= NEWER;
        let method = if newer { 15 } else { 13 };
        let flags = if newer { 17 } else { 14 };
        Self {
            method,
            flags,
            newer,
        }
    }

    /// Where the name's length is, after the flags, the filter, if any,
    /// the mode, and the time.
    fn name(&self, flags: u32) -> usize {
        let filter = if flags & FILTER != 0 { 4 } else { 0 };
        let time = if self.newer { 8 } else { 4 };
        self.flags
            .saturating_add(8)
            .saturating_add(filter)
            .saturating_add(time)
    }
}

/// Returns the checksum of a header, or of its extra field: an Adler-32,
/// or a CRC-32 if its flags say.
fn header_checksum(flags: u32, bytes: &[u8]) -> u32 {
    if flags & HEADER_CRC32 != 0 {
        crc32(bytes)
    } else {
        adler32(bytes)
    }
}

/// Returns how long the file header that `head` begins is, or, as far as
/// `head` tells, how long it must be to tell more.
fn header_len(head: &[u8]) -> Result<usize, Error> {
    let known = MAGIC.len().min(head.len());
    if head.get(..known) != MAGIC.get(..known) {
        return Err(corrupt());
    }
    let Some(version) = be16(head, MAGIC.len()) else {
        return Ok(11);
    };
    let layout = Layout::of(version);
    let Some(flags) = be32(head, layout.flags) else {
        return Ok(layout.flags.saturating_add(4));
    };
    let at = layout.name(flags);
    let Some(&name) = head.get(at) else {
        return Ok(at.saturating_add(1));
    };
    // The name, and the header's checksum.
    let end = at
        .saturating_add(1)
        .saturating_add(usize::from(name))
        .saturating_add(4);
    if flags & EXTRA_FIELD == 0 {
        return Ok(end);
    }
    let Some(extra) = be32(head, end) else {
        return Ok(end.saturating_add(4));
    };
    let extra = usize::try_from(extra).unwrap_or(usize::MAX);
    if extra > MAX_BLOCK {
        return Err(corrupt());
    }
    // The extra field's length, what it holds, and its checksum.
    Ok(end.saturating_add(8).saturating_add(extra))
}

/// Parses a whole file header, and returns its flags.
fn parse_header(header: &[u8]) -> Result<u32, Error> {
    let version = be16(header, MAGIC.len()).ok_or_else(corrupt)?;
    let layout = Layout::of(version);
    let flags = be32(header, layout.flags).ok_or_else(corrupt)?;
    let at = layout.name(flags);
    let name = usize::from(*header.get(at).ok_or_else(corrupt)?);
    let end = at.saturating_add(1).saturating_add(name);
    let checked = header.get(MAGIC.len()..end).ok_or_else(corrupt)?;
    if be32(header, end) != Some(header_checksum(flags, checked)) {
        return Err(corrupt());
    }
    if flags & EXTRA_FIELD != 0 {
        // Its length and what it holds, and then their checksum, afresh,
        // as lzop reads it; lzop writes none.
        let start = end.saturating_add(4);
        let extra = header.len().saturating_sub(4);
        let checked = header.get(start..extra).ok_or_else(corrupt)?;
        if be32(header, extra) != Some(header_checksum(flags, checked)) {
            return Err(corrupt());
        }
    }
    if version < OLDEST {
        return Err(corrupt());
    }
    let needed = if layout.newer {
        be16(header, 13).ok_or_else(corrupt)?
    } else {
        version
    };
    let method = *header.get(layout.method).ok_or_else(corrupt)?;
    // LZO1X-1, LZO1X-1(15), and LZO1X-999, which decode alike.
    if needed > NEWEST || !(1..=3).contains(&method) {
        return Err(unsupported());
    }
    if flags & (RESERVED | FILTER | MULTIPART) != 0 {
        return Err(unsupported());
    }
    Ok(flags)
}

/// Returns how many checksums of four bytes `flags` give a block: of
/// what it decodes to, and if it is `compressed`, of what it holds.
fn checksums(flags: u32, compressed: bool) -> usize {
    let mut kinds = [ADLER32_D, CRC32_D].as_slice();
    if compressed {
        kinds = &[ADLER32_D, CRC32_D, ADLER32_C, CRC32_C];
    }
    kinds.iter().filter(|&&kind| flags & kind != 0).count()
}

/// What a block decoded to, and how much of it is produced.
#[derive(Clone, Debug, Default)]
struct Window {
    buf: Vec<u8>,
    produced: usize,
}

impl Window {
    /// Produces what it can into `output`, and returns how much.
    fn produce(&mut self, output: &mut [u8]) -> usize {
        let pending = self.buf.get(self.produced..).unwrap_or_default();
        let n = pending.len().min(output.len());
        if let (Some(to), Some(from)) = (output.get_mut(..n), pending.get(..n)) {
            to.copy_from_slice(from);
        }
        self.produced = self.produced.saturating_add(n);
        n
    }

    fn pending(&self) -> bool {
        self.produced < self.buf.len()
    }
}

/// Where a [`Decoder`] is in its stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    /// Before a file, or in its header, gathered so far.
    Header,
    /// Before a block of the file, or its end mark, or in either,
    /// gathered so far.
    Block,
    /// In the zero bytes after the last file.
    Padding,
}

/// A decoder of lzop files, one after another, as `lzop` writes them, and
/// the Linux kernel reads them from initramfs images: each block checked
/// against its checksums, Adler-32s or CRC-32s, if it has them, and its
/// header against its own.
///
/// It gathers each header and block whole before it decodes it, unless
/// the input holds it whole, so it holds at most a block of packed bytes
/// and a block of what they decode to: lzop writes blocks of 256 KiB, and
/// allows up to 64 MiB. A file filtered, part of a multipart archive, or
/// compressed otherwise than by LZO1X is
/// [`Unsupported`](Error::Unsupported). What follows a file must be
/// another, or zero bytes to the end, which `lzop` ignores.
///
/// # Examples
///
/// ```
/// use decode_lzo::Decoder;
///
/// // `printf hello | lzop`.
/// let packed = [
///     0x89, 0x4C, 0x5A, 0x4F, 0x00, 0x0D, 0x0A, 0x1A, 0x0A, 0x10, 0x40, 0x20,
///     0xA0, 0x09, 0x40, 0x01, 0x05, 0x03, 0x00, 0x00, 0x0D, 0x00, 0x00, 0x00,
///     0x00, 0x6A, 0xEC, 0x2A, 0x40, 0x00, 0x00, 0x00, 0x00, 0x00, 0x2D, 0x1C,
///     0x03, 0x30, 0x00, 0x00, 0x00, 0x05, 0x00, 0x00, 0x00, 0x05, 0x06, 0x2C,
///     0x02, 0x15, 0x68, 0x65, 0x6C, 0x6C, 0x6F, 0x00, 0x00, 0x00, 0x00,
/// ];
/// let mut decoder = Decoder::new();
/// let mut output = [0; 8];
/// assert_eq!(decoder.decode(&packed, &mut output)?, (59, 5));
/// assert_eq!(&output[..5], b"hello");
/// decoder.finish()?;
/// # Ok::<(), decode_lzo::Error>(())
/// ```
#[derive(Clone)]
pub struct Decoder {
    /// The packed bytes of a header or block, gathered until whole.
    unit: Vec<u8>,
    state: State,
    /// The flags of the file in progress.
    flags: u32,
    window: Window,
    /// The files decoded whole.
    files: u64,
}

impl Decoder {
    /// Returns a decoder at the start of a stream.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            unit: Vec::new(),
            state: State::Header,
            flags: 0,
            window: Window {
                buf: Vec::new(),
                produced: 0,
            },
            files: 0,
        }
    }

    /// Decodes what it can of `input` into `output`, and returns how many
    /// bytes of each it consumed and produced.
    ///
    /// It returns neither consumed nor produced once it can go no further: needing
    /// more input, or more room, or past the end of the stream, which
    /// [`finish`](Self::finish) then tells apart. It returns at the end of
    /// each file.
    ///
    /// # Errors
    ///
    /// [`Corrupt`](Error::Corrupt) for a malformed stream, and
    /// [`Unsupported`](Error::Unsupported) for a file this decoder does not
    /// read. The decoder is not to be used after an error.
    pub fn decode(&mut self, input: &[u8], output: &mut [u8]) -> Result<(usize, usize), Error> {
        let (mut consumed, mut produced) = (0_usize, 0_usize);
        let files = self.files;
        loop {
            let room = output.get_mut(produced..).unwrap_or_default();
            produced = produced.saturating_add(self.window.produce(room));
            if self.window.pending() {
                return Ok((consumed, produced));
            }
            // Stop between files, once there is progress to report.
            if self.files != files && (consumed > 0 || produced > 0) {
                return Ok((consumed, produced));
            }
            let Some(n) = self.step(input.get(consumed..).unwrap_or_default())? else {
                return Ok((consumed, produced));
            };
            consumed = consumed.saturating_add(n);
        }
    }

    /// Checks that the stream has ended where a stream may: past a whole
    /// file, or in the zero bytes after one, and everything it decoded to
    /// produced.
    ///
    /// # Errors
    ///
    /// [`Corrupt`](Error::Corrupt) if it ended anywhere else, or before
    /// any file.
    pub fn finish(&self) -> Result<(), Error> {
        let between = match self.state {
            State::Header => self.files > 0,
            State::Padding => true,
            State::Block => false,
        };
        if between && self.unit.is_empty() && !self.window.pending() {
            Ok(())
        } else {
            Err(corrupt())
        }
    }

    /// Returns a decoder that goes on from where this one is: given the
    /// input from where this one has consumed to, less what it has
    /// [`buffered`](Self::buffered), it decodes what this one would. A
    /// file's blocks decode each alone, so it is a copy of this one's
    /// state and what it has not yet produced, which
    /// [`footprint`](Self::footprint) measures. It is always offered, so
    /// `Option` only for the shape that the `decode-*` crates share.
    #[must_use]
    pub fn checkpoint(&self) -> Option<Self> {
        let pending = self.window.buf.get(self.window.produced..);
        Some(Self {
            unit: Vec::new(),
            state: self.state,
            flags: self.flags,
            window: Window {
                buf: pending.unwrap_or_default().to_vec(),
                produced: 0,
            },
            files: self.files,
        })
    }

    /// Returns how many of the bytes it has consumed it holds, gathered
    /// toward a header or block that is not yet whole.
    #[must_use]
    pub fn buffered(&self) -> usize {
        self.unit.len()
    }

    /// Returns about how many bytes of memory it holds: mostly room for a
    /// block, gathered and decoded.
    #[must_use]
    pub fn footprint(&self) -> usize {
        size_of::<Self>()
            .saturating_add(self.window.buf.capacity())
            .saturating_add(self.unit.capacity())
    }

    /// Takes what it can of `input`, decoding a header or block once whole,
    /// and returns how much it consumed, or nothing if it can make no
    /// progress.
    fn step(&mut self, input: &[u8]) -> Result<Option<usize>, Error> {
        match self.state {
            State::Padding => {
                let zeros = input.iter().take_while(|&&byte| byte == 0).count();
                if zeros < input.len() {
                    return Err(corrupt());
                }
                Ok((zeros > 0).then_some(zeros))
            },
            State::Header
                if self.files > 0 && self.unit.is_empty() && input.first() == Some(&0) =>
            {
                self.state = State::Padding;
                Ok(Some(0))
            },
            State::Header | State::Block => {
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
        }
    }

    /// Returns how many bytes the header or block that `head` begins takes
    /// in all, as far as `head` tells.
    fn need(&self, head: &[u8]) -> Result<usize, Error> {
        if self.state == State::Header {
            return header_len(head);
        }
        let Some(size) = be32(head, 0) else {
            return Ok(4);
        };
        // The end mark.
        if size == 0 {
            return Ok(4);
        }
        if usize::try_from(size).unwrap_or(usize::MAX) > MAX_BLOCK {
            return Err(corrupt());
        }
        let Some(packed) = be32(head, 4) else {
            return Ok(8);
        };
        if packed == 0 || packed > size {
            return Err(corrupt());
        }
        let sums = checksums(self.flags, packed < size).saturating_mul(4);
        let packed = usize::try_from(packed).unwrap_or(usize::MAX);
        Ok(sums.saturating_add(8).saturating_add(packed))
    }

    /// Reads a whole header or block.
    fn unit(&mut self, unit: &[u8]) -> Result<(), Error> {
        if self.state == State::Header {
            self.flags = parse_header(unit)?;
            self.state = State::Block;
            return Ok(());
        }
        let (size, rest) = unit.split_first_chunk::<4>().ok_or_else(corrupt)?;
        let size = usize::try_from(u32::from_be_bytes(*size)).unwrap_or(usize::MAX);
        if size == 0 {
            self.files = self.files.saturating_add(1);
            self.state = State::Header;
            return Ok(());
        }
        let (packed, mut rest) = rest.split_first_chunk::<4>().ok_or_else(corrupt)?;
        let compressed = usize::try_from(u32::from_be_bytes(*packed)).unwrap_or(usize::MAX) < size;
        // The checksums, in the order of their flags', each of what the
        // block decodes to or of what it holds.
        let mut sums: [Option<(u32, u32, bool)>; 4] = [None; 4];
        let kinds = [
            (ADLER32_D, false),
            (CRC32_D, false),
            (ADLER32_C, true),
            (CRC32_C, true),
        ];
        for ((kind, packed), sum) in kinds.into_iter().zip(&mut sums) {
            if self.flags & kind == 0 || (packed && !compressed) {
                continue;
            }
            let (value, after) = rest.split_first_chunk::<4>().ok_or_else(corrupt)?;
            *sum = Some((kind, u32::from_be_bytes(*value), packed));
            rest = after;
        }
        let check = |bytes: &[u8], packed: bool| {
            sums.iter()
                .flatten()
                .filter(|sum| sum.2 == packed)
                .all(|&(kind, value, _)| {
                    let sum = if kind & (ADLER32_D | ADLER32_C) != 0 {
                        adler32(bytes)
                    } else {
                        crc32(bytes)
                    };
                    sum == value
                })
        };
        if !check(rest, true) {
            return Err(corrupt());
        }
        let buf = &mut self.window.buf;
        buf.clear();
        buf.resize(size, 0);
        if compressed {
            if block::decode_block(rest, buf)? != size {
                return Err(corrupt());
            }
        } else {
            buf.get_mut(..rest.len())
                .ok_or_else(corrupt)?
                .copy_from_slice(rest);
        }
        self.window.produced = 0;
        if !check(buf, false) {
            return Err(corrupt());
        }
        Ok(())
    }
}

impl Default for Decoder {
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Debug for Decoder {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Decoder")
            .field("state", &self.state)
            .field("files", &self.files)
            .finish_non_exhaustive()
    }
}

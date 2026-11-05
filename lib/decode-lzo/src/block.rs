// This is free and unencumbered software released into the public domain.

//! LZO1X blocks: instructions, each a match or a run of literals, every
//! match followed by up to three literals more, and a mark at the end.

use crate::{Error, corrupt, unsupported};

/// Decodes `input`, a whole LZO1X block, as btrfs, SquashFS, and F2FS
/// store them and lzop's blocks hold them, into `output`, and returns how
/// many bytes it decoded to. Its matches copy only from what it decodes
/// to.
///
/// It refuses what is malformed: a match from before the block's start,
/// and a block that ends anywhere but at its end mark, a match of 3 bytes
/// from 16 KiB back. As the Linux kernel does, and lzokay does not, it
/// refuses bytes after the mark too. A block that begins with 17, and
/// holds five bytes or more, names its version in its second byte, as
/// the Linux kernel's `lzo-rle` blocks do, whose version 1 adds an
/// instruction for runs of zeros; this decoder reads version 0 only.
///
/// # Errors
///
/// [`Corrupt`](Error::Corrupt) if it is malformed, or decodes to more than
/// `output` holds; [`Unsupported`](Error::Unsupported) if it names a
/// version.
///
/// # Examples
///
/// ```
/// use decode_lzo::decode_block;
///
/// // `hello`, five bytes again from five back, and the end mark.
/// let block = [22, b'h', b'e', b'l', b'l', b'o', 0x90, 0, 0x11, 0, 0];
/// let mut output = [0; 32];
/// let len = decode_block(&block, &mut output)?;
/// assert_eq!(&output[..len], b"hellohello");
/// # Ok::<(), decode_lzo::Error>(())
/// ```
pub fn decode_block(input: &[u8], output: &mut [u8]) -> Result<usize, Error> {
    Block {
        input,
        at: 0,
        output,
        pos: 0,
    }
    .decode()
}

/// A block being decoded: what of its input is read, and what of its
/// output is written.
struct Block<'a> {
    input: &'a [u8],
    at: usize,
    output: &'a mut [u8],
    pos: usize,
}

impl Block<'_> {
    fn decode(&mut self) -> Result<usize, Error> {
        // A first byte of 17, with four or more after it, names the
        // block's version in the next; no version 0 block is decodable
        // from one, as the match it would be has nothing to copy.
        if self.input.len() >= 5 && self.input.first() == Some(&17) {
            return Err(unsupported());
        }
        // How many literals the last instruction copied, 0 to 3, or 4 for
        // more, which tells what an instruction below 16 means.
        let mut state = 0;
        // A first byte above 17 is a run of literals alone.
        if let Some(&first) = self.input.first()
            && first > 17
        {
            self.at = 1;
            let n = usize::from(first.saturating_sub(17));
            self.literals(n)?;
            state = n.min(4);
        }
        loop {
            let op = self.byte()?;
            let (len, distance, next) = match op {
                // After a match with no literals, a run of 4 or more.
                0..=15 if state == 0 => {
                    let n = self.length(op, 15)?.saturating_add(3);
                    self.literals(n)?;
                    state = 4;
                    continue;
                },
                // After literals, a short match from close by: 2 bytes
                // from within 1 KiB after 1 to 3 literals, and 3 bytes
                // from 2 to 3 KiB back after more.
                0..=15 => {
                    let back = usize::from(self.byte()?) << 2 | usize::from(op >> 2 & 3);
                    if state == 4 {
                        (3, back.saturating_add(2049), usize::from(op & 3))
                    } else {
                        (2, back.saturating_add(1), usize::from(op & 3))
                    }
                },
                // From 16 KiB to 48 KiB back; from exactly 16 KiB, the
                // end mark.
                16..=31 => {
                    let len = self.length(op & 7, 7)?.saturating_add(2);
                    let word = self.word()?;
                    let back = usize::from(op & 8) << 11 | usize::from(word >> 2);
                    if back == 0 {
                        return if len == 3 && self.at == self.input.len() {
                            Ok(self.pos)
                        } else {
                            Err(corrupt())
                        };
                    }
                    (len, back.saturating_add(16384), usize::from(word & 3))
                },
                // From within 16 KiB.
                32..=63 => {
                    let len = self.length(op & 31, 31)?.saturating_add(2);
                    let word = self.word()?;
                    (
                        len,
                        usize::from(word >> 2).saturating_add(1),
                        usize::from(word & 3),
                    )
                },
                // 3 to 8 bytes from within 2 KiB.
                64..=255 => {
                    let back = usize::from(self.byte()?) << 3 | usize::from(op >> 2 & 7);
                    let len = usize::from(op >> 5).saturating_add(1);
                    (len, back.saturating_add(1), usize::from(op & 3))
                },
            };
            self.copy(distance, len)?;
            state = next;
            self.literals(state)?;
        }
    }

    fn byte(&mut self) -> Result<u8, Error> {
        let byte = *self.input.get(self.at).ok_or_else(corrupt)?;
        self.at = self.at.saturating_add(1);
        Ok(byte)
    }

    /// Reads a little-endian 16-bit word: a distance, and how many
    /// literals follow.
    fn word(&mut self) -> Result<u16, Error> {
        let (low, high) = (self.byte()?, self.byte()?);
        Ok(u16::from_le_bytes([low, high]))
    }

    /// Returns a length whose field is `field`, or, if zero, `max` and
    /// what the bytes after say: 255 for each zero, and the first other.
    fn length(&mut self, field: u8, max: usize) -> Result<usize, Error> {
        if field != 0 {
            return Ok(usize::from(field));
        }
        let mut len = max;
        loop {
            match self.byte()? {
                0 => len = len.checked_add(255).ok_or_else(corrupt)?,
                byte => return len.checked_add(usize::from(byte)).ok_or_else(corrupt),
            }
        }
    }

    /// Copies `n` literals from the input to the output.
    fn literals(&mut self, n: usize) -> Result<(), Error> {
        let (end, to) = (self.at.saturating_add(n), self.pos.saturating_add(n));
        let (Some(from), Some(into)) = (
            self.input.get(self.at..end),
            self.output.get_mut(self.pos..to),
        ) else {
            return Err(corrupt());
        };
        into.copy_from_slice(from);
        (self.at, self.pos) = (end, to);
        Ok(())
    }

    /// Copies `len` bytes from `distance` bytes back, which repeat every
    /// `distance` bytes where they overlap what they copy to.
    fn copy(&mut self, distance: usize, len: usize) -> Result<(), Error> {
        let (pos, end) = (self.pos, self.pos.saturating_add(len));
        let Some(from) = pos.checked_sub(distance) else {
            return Err(corrupt());
        };
        if end > self.output.len() {
            return Err(corrupt());
        }
        // Each copy doubles what repeats, until it is all copied.
        let mut done = 0;
        while done < len {
            let n = distance.saturating_add(done).min(len.saturating_sub(done));
            let (head, tail) = self
                .output
                .split_at_mut_checked(pos.saturating_add(done))
                .ok_or_else(corrupt)?;
            let (Some(source), Some(into)) =
                (head.get(from..from.saturating_add(n)), tail.get_mut(..n))
            else {
                return Err(corrupt());
            };
            into.copy_from_slice(source);
            done = done.saturating_add(n);
        }
        self.pos = end;
        Ok(())
    }
}

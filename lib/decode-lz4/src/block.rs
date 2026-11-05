// This is free and unencumbered software released into the public domain.

//! LZ4 blocks (the LZ4 Block Format Description): sequences, each of
//! literals and then a match, but the last, of literals alone.

use crate::{Error, corrupt};

/// The shortest match.
const MIN_MATCH: usize = 4;

/// A length field's value that more bytes follow.
const MORE: usize = 15;

/// Decodes `input`, a whole LZ4 block, into `output`, and returns how many
/// bytes it decoded to. The block is independent: its matches copy only
/// from what it decodes to, as SquashFS's, EROFS's, and ZFS's blocks do.
///
/// It refuses what is malformed: a match at offset zero, or before the
/// block's start, and a block that ends anywhere but after a sequence's
/// literals. Blocks that break the end conditions LZ4's encoders keep,
/// such as a match within the last 12 bytes, decode all the same.
///
/// # Errors
///
/// [`Corrupt`](Error::Corrupt) if it is malformed, or decodes to more than
/// `output` holds.
///
/// # Examples
///
/// ```
/// use decode_lz4::decode_block;
///
/// // `hello`, a match of six bytes from five back, and a space.
/// let block = [0x52, b'h', b'e', b'l', b'l', b'o', 5, 0, 0x10, b' '];
/// let mut output = [0; 32];
/// let len = decode_block(&block, &mut output)?;
/// assert_eq!(&output[..len], b"hellohelloh ");
/// # Ok::<(), decode_lz4::Error>(())
/// ```
pub fn decode_block(input: &[u8], output: &mut [u8]) -> Result<usize, Error> {
    decode(input, output, 0)
}

/// Decodes `input`, a whole block, into `buf` from `start`, after what its
/// matches may copy from too, and returns where it ends.
pub(crate) fn decode(input: &[u8], buf: &mut [u8], start: usize) -> Result<usize, Error> {
    let (mut at, mut pos) = (0_usize, start);
    loop {
        let token = *input.get(at).ok_or_else(corrupt)?;
        at = at.saturating_add(1);
        let literals = length(input, &mut at, usize::from(token >> 4))?;
        let (end, to) = (at.saturating_add(literals), pos.saturating_add(literals));
        let (Some(from), Some(into)) = (input.get(at..end), buf.get_mut(pos..to)) else {
            return Err(corrupt());
        };
        into.copy_from_slice(from);
        (at, pos) = (end, to);
        // The last sequence is of literals alone.
        if at == input.len() {
            return Ok(pos);
        }
        let Some(&[low, high]) = input.get(at..).and_then(<[u8]>::first_chunk) else {
            return Err(corrupt());
        };
        at = at.saturating_add(2);
        let offset = usize::from(u16::from_le_bytes([low, high]));
        if offset == 0 || offset > pos {
            return Err(corrupt());
        }
        let len = length(input, &mut at, usize::from(token & 0x0F))?.saturating_add(MIN_MATCH);
        copy(buf, pos, offset, len)?;
        pos = pos.saturating_add(len);
    }
}

/// Returns a length whose token field is `len`, reading the bytes after
/// `at` that a field of 15 says follow.
fn length(input: &[u8], at: &mut usize, mut len: usize) -> Result<usize, Error> {
    if len < MORE {
        return Ok(len);
    }
    loop {
        let byte = *input.get(*at).ok_or_else(corrupt)?;
        *at = at.saturating_add(1);
        len = len.checked_add(usize::from(byte)).ok_or_else(corrupt)?;
        if byte != 0xFF {
            return Ok(len);
        }
    }
}

/// Copies `len` bytes to `pos` in `buf` from `offset` bytes back, which
/// repeat every `offset` bytes where they overlap what they copy to.
fn copy(buf: &mut [u8], pos: usize, offset: usize, len: usize) -> Result<(), Error> {
    let from = pos.saturating_sub(offset);
    if pos.saturating_add(len) > buf.len() {
        return Err(corrupt());
    }
    // Each copy doubles what repeats, until it is all copied.
    let mut done = 0;
    while done < len {
        let n = offset.saturating_add(done).min(len.saturating_sub(done));
        let (head, tail) = buf
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
    Ok(())
}

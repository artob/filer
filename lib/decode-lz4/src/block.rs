// This is free and unencumbered software released into the public domain.

//! LZ4 blocks (the LZ4 Block Format Description): sequences, each of
//! literals and then a match, but the last, of literals alone.

use crate::{Error, corrupt};

/// The shortest match.
const MIN_MATCH: usize = 4;

/// A length field's value that more bytes follow.
const MORE: usize = 15;

/// The bytes of a short copy, which copies a whole chunk even when fewer
/// are wanted, where the input and the room hold it: most literals and
/// matches are this short or little longer.
const SHORT: usize = 16;

/// The longest match copied by short copies, or by copies of 8 bytes
/// from 8 to 15 back; longer ones, and those nearer, are copied whole.
const LONG: usize = 64;

/// Decodes `input`, a whole LZ4 block, into `output`, and returns how many
/// bytes it decoded to. The block is independent: its matches copy only
/// from what it decodes to, as SquashFS's, EROFS's, and ZFS's blocks do.
///
/// It refuses what is malformed: a match at offset zero, or before the
/// block's start, and a block that ends anywhere but after a sequence's
/// literals. Blocks that break the end conditions LZ4's encoders keep,
/// such as a match within the last 12 bytes, decode all the same. It may
/// overwrite up to 16 bytes of `output` past what it decodes to.
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
/// matches may copy from too, and returns where it ends: on x86-64, never
/// inlined, so that its loop has the 16 registers to itself, where on
/// Zen 1 inlined it spilled them, 13% slower; AArch64 has 31, and is 3%
/// faster with it inlined, on Apple M4.
#[cfg_attr(target_arch = "x86_64", inline(never))]
pub(crate) fn decode(input: &[u8], buf: &mut [u8], start: usize) -> Result<usize, Error> {
    let (mut at, mut pos) = (0_usize, start);
    loop {
        let token = *input.get(at).ok_or_else(corrupt)?;
        // Wrapping, as saturating would lengthen the chain from token to
        // token: `at` is within `input` and `pos` within `buf`, and what
        // wraps past either ends before it begins, which no slice is.
        at = at.wrapping_add(1);
        let literals = length(input, &mut at, usize::from(token >> 4))?;
        let (end, to) = (at.wrapping_add(literals), pos.wrapping_add(literals));
        let source = input.get(at..).and_then(<[u8]>::first_chunk::<SHORT>);
        let target = buf
            .get_mut(pos..)
            .and_then(<[u8]>::first_chunk_mut::<SHORT>);
        match (source, target) {
            // What it copies past the literals, the next ones overwrite.
            (Some(source), Some(target)) if literals <= SHORT && end <= input.len() => {
                *target = *source;
            },
            _ => {
                let (Some(from), Some(into)) = (input.get(at..end), buf.get_mut(pos..to)) else {
                    return Err(corrupt());
                };
                into.copy_from_slice(from);
            },
        }
        (at, pos) = (end, to);
        // The last sequence is of literals alone.
        if at == input.len() {
            return Ok(pos);
        }
        let Some(&[low, high]) = input.get(at..).and_then(<[u8]>::first_chunk) else {
            return Err(corrupt());
        };
        at = at.wrapping_add(2);
        let offset = usize::from(u16::from_le_bytes([low, high]));
        if offset == 0 || offset > pos {
            return Err(corrupt());
        }
        let len = length(input, &mut at, usize::from(token & 0x0F))?;
        pos = copy(
            buf,
            pos,
            offset,
            len.checked_add(MIN_MATCH).ok_or_else(corrupt)?,
        )?;
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
        *at = at.wrapping_add(1);
        len = len.checked_add(usize::from(byte)).ok_or_else(corrupt)?;
        if byte != 0xFF {
            return Ok(len);
        }
    }
}

/// Copies `len` bytes to `pos` in `buf` from `offset` bytes back, which
/// repeat every `offset` bytes where they overlap what they copy to, and
/// returns where they end.
fn copy(buf: &mut [u8], pos: usize, offset: usize, len: usize) -> Result<usize, Error> {
    let from = pos.saturating_sub(offset);
    let end = pos.checked_add(len).ok_or_else(corrupt)?;
    if end > buf.len() {
        return Err(corrupt());
    }
    let short = match offset {
        _ if len > LONG => false,
        SHORT.. => shorts::<SHORT>(buf, from, pos, end),
        8.. => shorts::<8>(buf, from, pos, end),
        _ => false,
    };
    if short {
        return Ok(end);
    }
    repeat(buf, pos, offset, len)?;
    Ok(end)
}

/// Copies `len` bytes to `pos` in `buf` from `offset` bytes back, as
/// [`copy`] does where short copies do not serve: on x86-64, never
/// inlined, so that the loop that calls it keeps its registers for what it
/// does most, 3% faster on Zen 1; and elsewhere always, as on Apple M4
/// it is 3% faster inlined.
#[cfg_attr(target_arch = "x86_64", inline(never))]
#[cfg_attr(not(target_arch = "x86_64"), inline(always))]
fn repeat(buf: &mut [u8], pos: usize, offset: usize, len: usize) -> Result<(), Error> {
    let from = pos.saturating_sub(offset);
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

/// Copies to `pos` in `buf`, up to `end`, from `from` on, by copies of `N`
/// bytes, which write past `end`, and returns whether `buf` has room for
/// them: `from` must lie at least `N` back, so that each reads only what
/// is copied before it.
fn shorts<const N: usize>(buf: &mut [u8], mut from: usize, mut pos: usize, end: usize) -> bool {
    if end.saturating_add(N) > buf.len() {
        return false;
    }
    while pos < end {
        let Some(&source) = buf.get(from..).and_then(<[u8]>::first_chunk::<N>) else {
            return false;
        };
        let Some(target) = buf.get_mut(pos..).and_then(<[u8]>::first_chunk_mut) else {
            return false;
        };
        *target = source;
        // Within `buf`, as the chunks were.
        (from, pos) = (from.wrapping_add(N), pos.wrapping_add(N));
    }
    true
}

// This is free and unencumbered software released into the public domain.

//! LZO1X blocks: instructions, each a match or a run of literals, every
//! match followed by up to three literals more, and a mark at the end.

use crate::{Error, corrupt, unsupported};

/// The bytes of a short copy, which copies a whole chunk even when fewer
/// are wanted, where the input and the room hold it: most literals and
/// matches are this short or little longer.
const SHORT: usize = 16;

/// The longest match copied by short copies, or by copies of 8 bytes
/// from 8 to 15 back; longer ones, and those nearer, are copied whole.
const LONG: usize = 64;

/// Decodes `input`, a whole LZO1X block, as btrfs, SquashFS, and F2FS
/// store them and lzop's blocks hold them, into `output`, and returns how
/// many bytes it decoded to. Its matches copy only from what it decodes
/// to.
///
/// It refuses what is malformed: a match from before the block's start,
/// and a block that ends anywhere but at its end mark, a match of 3 bytes
/// from 16 KiB back. As the Linux kernel does, and lzokay does not, it
/// refuses bytes after the mark too. It may overwrite up to 16 bytes of
/// `output` past what it decodes to.
///
/// A block that begins with 17, and holds five bytes or more, names its
/// version in its second byte, as the Linux kernel's `lzo-rle` blocks do,
/// and its instructions follow. Version 1, theirs, adds a run of 4 to
/// 2,051 zeros, written as a match from 48 KiB less a byte back: its
/// length's low 3 bits where a match's are, and its high 8 in a byte
/// after the distance. A block that names no version is of version 0.
///
/// # Errors
///
/// [`Corrupt`](Error::Corrupt) if it is malformed, or decodes to more than
/// `output` holds; [`Unsupported`](Error::Unsupported) if it names a
/// version past 1.
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
///
/// // Version 1: `a`, 8 zeros, and the end mark.
/// let block = [17, 1, 18, b'a', 0x1C, 0xFC, 0xFF, 0, 0x11, 0, 0];
/// let len = decode_block(&block, &mut output)?;
/// assert_eq!(&output[..len], b"a\0\0\0\0\0\0\0\0");
/// # Ok::<(), decode_lzo::Error>(())
/// ```
pub fn decode_block(input: &[u8], output: &mut [u8]) -> Result<usize, Error> {
    // Where it is in `input` and `output`, kept in locals, not a struct,
    // so that they stay in registers. Each advances by wrapping, as
    // saturating would lengthen the chain from instruction to
    // instruction: `at` is within `input` and `pos` within `output`, and
    // what wraps past either ends before it begins, which no slice is.
    let (mut at, mut pos) = (0_usize, 0_usize);
    // A first byte of 17, with four or more after it, names the block's
    // version in the next, as no block that names none could begin: the
    // match it would be has nothing to copy.
    let mut version = 0;
    if input.len() >= 5 && input.first() == Some(&17) {
        at = 1;
        version = byte(input, &mut at)?;
        if version > 1 {
            return Err(unsupported());
        }
    }
    // How many literals the last instruction copied, 0 to 3, or 4 for
    // more, which tells what an instruction below 16 means.
    let mut state = 0;
    // A first instruction above 17 is a run of literals alone.
    if let Some(&first) = input.get(at)
        && first > 17
    {
        at = at.wrapping_add(1);
        let n = usize::from(first.saturating_sub(17));
        (at, pos) = literals(input, at, output, pos, n)?;
        state = n.min(4);
    }
    loop {
        let op = byte(input, &mut at)?;
        let (len, distance, next) = match op {
            // After a match with no literals, a run of 4 or more.
            0..=15 if state == 0 => {
                let n = length(input, &mut at, op, 15)?
                    .checked_add(3)
                    .ok_or_else(corrupt)?;
                (at, pos) = literals(input, at, output, pos, n)?;
                state = 4;
                continue;
            },
            // After literals, a short match from close by: 2 bytes from
            // within 1 KiB after 1 to 3 literals, and 3 bytes from 2 to 3
            // KiB back after more.
            0..=15 => {
                let back = usize::from(byte(input, &mut at)?) << 2 | usize::from(op >> 2 & 3);
                if state == 4 {
                    (3, back.wrapping_add(2049), usize::from(op & 3))
                } else {
                    (2, back.wrapping_add(1), usize::from(op & 3))
                }
            },
            // In version 1, from 48 KiB less a byte back, a run of zeros,
            // its distance read before any length's bytes.
            24..=31 if version == 1 && zeros_next(input, at) => {
                let word = word(input, &mut at)?;
                let high = usize::from(byte(input, &mut at)?);
                let len = (high << 3 | usize::from(op & 7)).wrapping_add(4);
                let to = pos.checked_add(len).ok_or_else(corrupt)?;
                output.get_mut(pos..to).ok_or_else(corrupt)?.fill(0);
                pos = to;
                state = usize::from(word & 3);
                (at, pos) = literals(input, at, output, pos, state)?;
                continue;
            },
            // From 16 KiB to 48 KiB back; from exactly 16 KiB, the end
            // mark.
            16..=31 => {
                let len = length(input, &mut at, op & 7, 7)?
                    .checked_add(2)
                    .ok_or_else(corrupt)?;
                let word = word(input, &mut at)?;
                let back = usize::from(op & 8) << 11 | usize::from(word >> 2);
                if back == 0 {
                    return if len == 3 && at == input.len() {
                        Ok(pos)
                    } else {
                        Err(corrupt())
                    };
                }
                (len, back.wrapping_add(16384), usize::from(word & 3))
            },
            // From within 16 KiB.
            32..=63 => {
                let len = length(input, &mut at, op & 31, 31)?
                    .checked_add(2)
                    .ok_or_else(corrupt)?;
                let word = word(input, &mut at)?;
                (
                    len,
                    usize::from(word >> 2).wrapping_add(1),
                    usize::from(word & 3),
                )
            },
            // 3 to 8 bytes from within 2 KiB.
            64..=255 => {
                let back = usize::from(byte(input, &mut at)?) << 3 | usize::from(op >> 2 & 7);
                let len = usize::from(op >> 5).wrapping_add(1);
                (len, back.wrapping_add(1), usize::from(op & 3))
            },
        };
        pos = copy(output, pos, distance, len)?;
        state = next;
        (at, pos) = literals(input, at, output, pos, state)?;
    }
}

/// Reads the byte at `at` in `input`, and moves `at` past it.
fn byte(input: &[u8], at: &mut usize) -> Result<u8, Error> {
    let byte = *input.get(*at).ok_or_else(corrupt)?;
    *at = at.wrapping_add(1);
    Ok(byte)
}

/// Reads a little-endian 16-bit word at `at`: a distance, and how many
/// literals follow.
fn word(input: &[u8], at: &mut usize) -> Result<u16, Error> {
    let &word = input
        .get(*at..)
        .and_then(<[u8]>::first_chunk)
        .ok_or_else(corrupt)?;
    *at = at.wrapping_add(2);
    Ok(u16::from_le_bytes(word))
}

/// Whether the next two bytes are a distance whose bits are all ones,
/// which in version 1 makes a run of zeros of a match from 48 KiB back.
fn zeros_next(input: &[u8], at: usize) -> bool {
    let next = input.get(at..).and_then(<[u8]>::first_chunk);
    matches!(next, Some(&[low, 0xFF]) if low & 0xFC == 0xFC)
}

/// Returns a length whose field is `field`, or, if zero, `max` and what
/// the bytes after say: 255 for each zero, and the first other.
fn length(input: &[u8], at: &mut usize, field: u8, max: usize) -> Result<usize, Error> {
    if field != 0 {
        return Ok(usize::from(field));
    }
    let mut len = max;
    loop {
        match byte(input, at)? {
            0 => len = len.checked_add(255).ok_or_else(corrupt)?,
            byte => return len.checked_add(usize::from(byte)).ok_or_else(corrupt),
        }
    }
}

/// Copies `n` literals from `at` in `input` to `pos` in `output`, and
/// returns where each ends.
fn literals(
    input: &[u8],
    at: usize,
    output: &mut [u8],
    pos: usize,
    n: usize,
) -> Result<(usize, usize), Error> {
    let (end, to) = (at.wrapping_add(n), pos.wrapping_add(n));
    let source = input.get(at..).and_then(<[u8]>::first_chunk::<SHORT>);
    let target = output
        .get_mut(pos..)
        .and_then(<[u8]>::first_chunk_mut::<SHORT>);
    match (source, target) {
        // What it copies past the literals, what follows overwrites.
        (Some(source), Some(target)) if n <= SHORT => *target = *source,
        _ => {
            let (Some(from), Some(into)) = (input.get(at..end), output.get_mut(pos..to)) else {
                return Err(corrupt());
            };
            into.copy_from_slice(from);
        },
    }
    Ok((end, to))
}

/// Copies `len` bytes to `pos` in `output` from `distance` bytes back,
/// which repeat every `distance` bytes where they overlap what they copy
/// to, and returns where they end.
fn copy(output: &mut [u8], pos: usize, distance: usize, len: usize) -> Result<usize, Error> {
    let from = pos.checked_sub(distance).ok_or_else(corrupt)?;
    let end = pos.checked_add(len).ok_or_else(corrupt)?;
    if end > output.len() {
        return Err(corrupt());
    }
    let short = match distance {
        _ if len > LONG => false,
        SHORT.. => shorts::<SHORT>(output, from, pos, end),
        8.. => shorts::<8>(output, from, pos, end),
        _ => false,
    };
    if short {
        return Ok(end);
    }
    // Each copy doubles what repeats, until it is all copied.
    let mut done = 0;
    while done < len {
        let n = distance.saturating_add(done).min(len.saturating_sub(done));
        let (head, tail) = output
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
    Ok(end)
}

/// Copies to `pos` in `output`, up to `end`, from `from` on, by copies of
/// `N` bytes, which write past `end`, and returns whether `output` has
/// room for them: `from` must lie at least `N` back, so that each reads
/// only what is copied before it.
fn shorts<const N: usize>(output: &mut [u8], mut from: usize, mut pos: usize, end: usize) -> bool {
    if end.saturating_add(N) > output.len() {
        return false;
    }
    while pos < end {
        let Some(&source) = output.get(from..).and_then(<[u8]>::first_chunk::<N>) else {
            return false;
        };
        let Some(target) = output.get_mut(pos..).and_then(<[u8]>::first_chunk_mut) else {
            return false;
        };
        *target = source;
        // Within `output`, as the chunks were.
        (from, pos) = (from.wrapping_add(N), pos.wrapping_add(N));
    }
    true
}

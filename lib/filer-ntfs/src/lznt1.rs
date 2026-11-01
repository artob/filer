// This is free and unencumbered software released into the public domain.

//! LZNT1 decompression, as [MS-XCA] §2.5 specifies it.
//!
//! A compressed stream is a sequence of chunks, each standing for up to
//! 4096 bytes of output. A chunk starts with a 16-bit header: its low 12
//! bits are its size, header included, minus 3; its top bit says whether
//! it is compressed. A zero header ends the stream. A compressed chunk is
//! groups of a flag byte and eight tokens: a clear flag bit is a literal
//! byte, a set one a 16-bit back-reference, split into a displacement and
//! a length (plus 3) whose widths depend on how much of the chunk has been
//! decompressed: the displacement has 4 bits up to 16 bytes, and one more
//! each time that doubles, up to 12.

/// The output of one chunk.
pub(crate) const CHUNK: usize = 4096;

/// Decompresses `input` into `output`, chunk by chunk, each into its own
/// 4096 bytes; bytes no chunk reaches are left as they are (zeros, when
/// decompressing a compression unit). Returns `None` if the stream is
/// malformed: a chunk past `input` or `output`, a back-reference before
/// the chunk's start, or output past the chunk's end.
#[must_use]
pub(crate) fn decompress(input: &[u8], output: &mut [u8]) -> Option<()> {
    let mut at = 0usize;
    for chunk_output in output.chunks_mut(CHUNK) {
        let Some(header) = input.get(at..).and_then(<[u8]>::first_chunk::<2>) else {
            // The stream may end where its data does.
            return Some(());
        };
        let header = u16::from_le_bytes(*header);
        if header == 0 {
            return Some(());
        }
        let size = usize::from(header & 0x0FFF).checked_add(3)?;
        let data = input.get(at.checked_add(2)?..at.checked_add(size)?)?;
        if header & 0x8000 == 0 {
            chunk_output.get_mut(..data.len())?.copy_from_slice(data);
        } else {
            chunk(data, chunk_output)?;
        }
        at = at.checked_add(size)?;
    }
    // Chunks past the output are more data than the unit holds.
    match input.get(at..).and_then(<[u8]>::first_chunk::<2>) {
        None | Some(&[0, 0]) => Some(()),
        Some(_) => None,
    }
}

/// Decompresses one compressed chunk's `data` into `output`.
fn chunk(data: &[u8], output: &mut [u8]) -> Option<()> {
    let mut at = 0usize;
    let mut pos = 0usize;
    while at < data.len() {
        let flags = *data.get(at)?;
        at = at.checked_add(1)?;
        for bit in 0..8 {
            if at >= data.len() {
                break;
            }
            if flags & (1 << bit) == 0 {
                *output.get_mut(pos)? = *data.get(at)?;
                pos = pos.checked_add(1)?;
                at = at.checked_add(1)?;
                continue;
            }
            let token = u16::from_le_bytes(*data.get(at..)?.first_chunk::<2>()?);
            at = at.checked_add(2)?;
            // A reference needs output before it.
            let mut length_bits = 12u32;
            let mut p = pos.checked_sub(1)?;
            while p >= 0x10 {
                length_bits = length_bits.checked_sub(1)?;
                p >>= 1;
            }
            let length_mask = (1u16 << length_bits).checked_sub(1)?;
            let displacement = usize::from(token >> length_bits).checked_add(1)?;
            let length = usize::from(token & length_mask).checked_add(3)?;
            let mut from = pos.checked_sub(displacement)?;
            // Byte by byte, as the copy may overlap its own output.
            for _ in 0..length {
                *output.get_mut(pos)? = *output.get(from)?;
                pos = pos.checked_add(1)?;
                from = from.checked_add(1)?;
            }
        }
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{vec, vec::Vec};

    /// Wraps compressed chunk `data` in its header.
    fn compressed(data: &[u8]) -> Vec<u8> {
        let header = 0xB000 | u16::try_from(data.len() + 2 - 3).unwrap();
        let mut out = header.to_le_bytes().to_vec();
        out.extend(data);
        out
    }

    #[test]
    fn decompresses_literals_and_references() {
        // "abc", then a reference 3 back for 9 bytes: at position 3, the
        // displacement takes 4 bits, the length 12: (3 - 1) << 12 | (9 - 3).
        let token = (2u16 << 12 | 6).to_le_bytes();
        let input = compressed(&[0b0000_1000, b'a', b'b', b'c', token[0], token[1]]);
        let mut output = vec![0; 32];
        decompress(&input, &mut output).unwrap();
        assert_eq!(&output[..12], b"abcabcabcabc");
        assert_eq!(&output[12..], &[0; 20]);
    }

    #[test]
    fn widens_displacements_as_the_chunk_grows() {
        // 17 literals, then a reference: at position 17 the displacement
        // takes 5 bits and the length 11, so 17 back for 4 is
        // (17 - 1) << 11 | (4 - 3).
        let token = (16u16 << 11 | 1).to_le_bytes();
        let mut data = vec![0u8];
        data.extend(b"ABCDEFGH");
        data.push(0);
        data.extend(b"IJKLMNOP");
        data.push(0b0000_0010);
        data.push(b'Q');
        data.extend(token);
        let mut output = vec![0; 32];
        decompress(&compressed(&data), &mut output).unwrap();
        assert_eq!(&output[..21], b"ABCDEFGHIJKLMNOPQABCD");
    }

    #[test]
    fn copies_uncompressed_chunks_and_zero_fills() {
        let mut input = (0x3000u16 | (4096 + 2 - 3)).to_le_bytes().to_vec();
        input.extend(vec![7u8; 4096]);
        input.extend([0, 0]);
        let mut output = vec![0; 3 * CHUNK];
        decompress(&input, &mut output).unwrap();
        assert_eq!(&output[..CHUNK], &[7; CHUNK]);
        assert_eq!(&output[CHUNK..], &[0; 2 * CHUNK]);
    }

    #[test]
    fn places_each_chunk_in_its_own_4096_bytes() {
        let mut input = compressed(&[0, b'x']);
        input.extend(compressed(&[0, b'y']));
        let mut output = vec![0; 2 * CHUNK];
        decompress(&input, &mut output).unwrap();
        assert_eq!((output[0], output[1], output[CHUNK]), (b'x', 0, b'y'));
    }

    #[test]
    fn rejects_malformed_streams() {
        let reference = |token: u16| {
            let token = token.to_le_bytes();
            compressed(&[0b0000_0010, b'a', token[0], token[1]])
        };
        for input in [
            // A reference with no output before it.
            compressed(&[0b0000_0001, 0, 0]),
            // A displacement past the chunk's start.
            reference(5 << 12),
            // A chunk longer than the input.
            vec![0x10, 0xB0, 0],
        ] {
            assert_eq!(decompress(&input, &mut [0; CHUNK]), None, "{input:02X?}");
        }
        // Output past the chunk: 4095 literals, then a reference of 3.
        let mut data = Vec::new();
        for group in vec![b'z'; 4095].chunks(8) {
            data.push(0);
            data.extend(group);
        }
        // 4095 = 511 groups of 8 and one of 7: add the reference there.
        let last = data.len() - 8;
        data[last] = 0b1000_0000;
        data.extend((0u16).to_le_bytes());
        assert_eq!(decompress(&compressed(&data), &mut [0; CHUNK]), None);
        // More chunks than the output holds.
        let mut two = compressed(&[0, b'x']);
        two.extend(compressed(&[0, b'y']));
        assert_eq!(decompress(&two, &mut [0; CHUNK]), None);
    }
}

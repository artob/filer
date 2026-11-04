// This is free and unencumbered software released into the public domain.

//! XXH64, which zstd checks frames with (RFC 8878 §3.1.1), written here from
//! the xxHash specification (`LEGAL.md`).

const P1: u64 = 0x9E37_79B1_85EB_CA87;
const P2: u64 = 0xC2B2_AE3D_27D4_EB4F;
const P3: u64 = 0x1656_67B1_9E37_79F9;
const P4: u64 = 0x85EB_CA77_C2B2_AE63;
const P5: u64 = 0x27D4_EB2F_1656_67C5;

/// The bytes the accumulators take at a time.
const STRIPE: usize = 32;

/// XXH64, with a seed of zero, as zstd hashes, of the bytes given so far.
#[derive(Clone, Debug)]
pub(crate) struct Xxh64 {
    /// The four accumulators, over each whole stripe so far.
    acc: [u64; 4],
    /// The bytes past the last whole stripe, `tailed` of them.
    tail: [u8; STRIPE],
    tailed: usize,
    len: u64,
}

impl Xxh64 {
    pub(crate) const fn new() -> Self {
        Self {
            acc: [P1.wrapping_add(P2), P2, 0, 0_u64.wrapping_sub(P1)],
            tail: [0; STRIPE],
            tailed: 0,
            len: 0,
        }
    }

    pub(crate) fn update(&mut self, mut bytes: &[u8]) {
        self.len = self
            .len
            .wrapping_add(u64::try_from(bytes.len()).unwrap_or(u64::MAX));
        if self.tailed > 0 {
            let take = STRIPE.saturating_sub(self.tailed).min(bytes.len());
            let (head, rest) = bytes.split_at(take);
            let end = self.tailed.saturating_add(take);
            if let Some(slot) = self.tail.get_mut(self.tailed..end) {
                slot.copy_from_slice(head);
            }
            (self.tailed, bytes) = (end, rest);
            if self.tailed < STRIPE {
                return;
            }
            let tail = self.tail;
            self.stripe(&tail);
            self.tailed = 0;
        }
        let (stripes, rest) = bytes.as_chunks::<STRIPE>();
        for stripe in stripes {
            self.stripe(stripe);
        }
        if let Some(slot) = self.tail.get_mut(..rest.len()) {
            slot.copy_from_slice(rest);
        }
        self.tailed = rest.len();
    }

    fn stripe(&mut self, stripe: &[u8; STRIPE]) {
        let (lanes, _) = stripe.as_chunks::<8>();
        for (acc, lane) in self.acc.iter_mut().zip(lanes) {
            *acc = round(*acc, u64::from_le_bytes(*lane));
        }
    }

    pub(crate) fn digest(&self) -> u64 {
        let mut hash = if self.len >= 32 {
            let [a, b, c, d] = self.acc;
            let hash = a
                .rotate_left(1)
                .wrapping_add(b.rotate_left(7))
                .wrapping_add(c.rotate_left(12))
                .wrapping_add(d.rotate_left(18));
            self.acc.iter().fold(hash, |hash, &acc| {
                (hash ^ round(0, acc)).wrapping_mul(P1).wrapping_add(P4)
            })
        } else {
            P5
        };
        hash = hash.wrapping_add(self.len);
        let tail = self.tail.get(..self.tailed).unwrap_or_default();
        let (lanes, mut rest) = tail.as_chunks::<8>();
        for lane in lanes {
            hash ^= round(0, u64::from_le_bytes(*lane));
            hash = hash.rotate_left(27).wrapping_mul(P1).wrapping_add(P4);
        }
        if let Some((word, after)) = rest.split_first_chunk::<4>() {
            hash ^= u64::from(u32::from_le_bytes(*word)).wrapping_mul(P1);
            hash = hash.rotate_left(23).wrapping_mul(P2).wrapping_add(P3);
            rest = after;
        }
        for &byte in rest {
            hash ^= u64::from(byte).wrapping_mul(P5);
            hash = hash.rotate_left(11).wrapping_mul(P1);
        }
        hash ^= hash >> 33;
        hash = hash.wrapping_mul(P2);
        hash ^= hash >> 29;
        hash = hash.wrapping_mul(P3);
        hash ^ hash >> 32
    }
}

fn round(acc: u64, lane: u64) -> u64 {
    acc.wrapping_add(lane.wrapping_mul(P2))
        .rotate_left(31)
        .wrapping_mul(P1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    fn xxh64(bytes: &[u8]) -> u64 {
        let mut hash = Xxh64::new();
        hash.update(bytes);
        hash.digest()
    }

    #[test]
    fn hashes_as_xxhash_does() {
        assert_eq!(xxh64(b""), 0xEF46_DB37_51D8_E999);
        assert_eq!(xxh64(b"a"), 0xD24E_C4F1_A98C_6E5B);
        assert_eq!(xxh64(b"abc"), 0x44BC_2CF5_AD77_0999);
    }

    #[test]
    fn hashes_the_same_in_pieces() {
        let bytes: Vec<u8> = (0..200_u32).map(|n| (n * 37 % 251) as u8).collect();
        for len in 0..bytes.len() {
            let whole = xxh64(&bytes[..len]);
            for piece in [1, 3, 31, 32, 33] {
                let mut hash = Xxh64::new();
                for chunk in bytes[..len].chunks(piece) {
                    hash.update(chunk);
                }
                assert_eq!(hash.digest(), whole, "{len} in pieces of {piece}");
            }
        }
    }
}

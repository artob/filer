// This is free and unencumbered software released into the public domain.

//! XXH32 (§"XXH32 Algorithm Description").

/// XXH32's primes.
const P1: u32 = 0x9E37_79B1;
const P2: u32 = 0x85EB_CA77;
const P3: u32 = 0xC2B2_AE3D;
const P4: u32 = 0x27D4_EB2F;
const P5: u32 = 0x1656_67B1;

/// The bytes the accumulators take at a time.
const STRIPE: usize = 16;

/// The stripes an accumulator takes at a turn, where each takes its own:
/// few enough that the processor has all four's under way at once. Of 2
/// to 8, Neoverse V3 is fastest by 3, and Apple M4 as fast by any.
const TURN: usize = 3;

/// XXH32 of the bytes fed so far, from a seed.
///
/// ```
/// use compute_xxhash::{Xxh32, xxh32};
///
/// let mut hash = Xxh32::new(0);
/// hash.update(b"a");
/// hash.update(b"bc");
/// assert_eq!(hash.finish(), 0x32D1_53FF);
/// assert_eq!(hash.finish(), xxh32(b"abc", 0));
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Xxh32 {
    seed: u32,
    /// The four accumulators, over each whole stripe so far.
    acc: [u32; 4],
    /// The bytes past the last whole stripe, `tailed` of them.
    tail: [u8; STRIPE],
    tailed: usize,
    len: u64,
}

impl Xxh32 {
    /// Starts an XXH32 over no bytes, from `seed`, which LZ4's is zero.
    #[must_use]
    pub const fn new(seed: u32) -> Self {
        Self {
            seed,
            acc: [
                seed.wrapping_add(P1).wrapping_add(P2),
                seed.wrapping_add(P2),
                seed,
                seed.wrapping_sub(P1),
            ],
            tail: [0; STRIPE],
            tailed: 0,
            len: 0,
        }
    }

    /// Feeds `bytes`.
    pub fn update(&mut self, mut bytes: &[u8]) {
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
        self.stripes(stripes);
        if let Some(slot) = self.tail.get_mut(..rest.len()) {
            slot.copy_from_slice(rest);
        }
        self.tailed = rest.len();
    }

    /// Feeds `bytes`, and returns the hash that goes on from there, so
    /// that calls chain.
    #[must_use]
    pub fn chain(mut self, bytes: &[u8]) -> Self {
        self.update(bytes);
        self
    }

    /// Takes whole stripes, a lane of each into each accumulator.
    ///
    /// On ARM, an accumulator takes its lane of a few stripes before the
    /// next takes its own: taken together, the four are made a vector,
    /// whose multiplications each wait twice as long as a register's, and
    /// the stripes with them; taken apart, each waits on its own alone,
    /// and the processor runs the four at once.
    fn stripes(&mut self, stripes: &[[u8; STRIPE]]) {
        if !cfg!(target_arch = "aarch64") {
            for stripe in stripes {
                self.stripe(stripe);
            }
            return;
        }
        for turn in stripes.chunks(TURN) {
            for (lane, acc) in self.acc.iter_mut().enumerate() {
                let mut now = *acc;
                for stripe in turn {
                    let (lanes, _) = stripe.as_chunks::<4>();
                    if let Some(word) = lanes.get(lane) {
                        now = round(now, u32::from_le_bytes(*word));
                    }
                }
                *acc = now;
            }
        }
    }

    /// Takes a stripe, a lane into each accumulator.
    fn stripe(&mut self, stripe: &[u8; STRIPE]) {
        let (lanes, _) = stripe.as_chunks::<4>();
        for (acc, lane) in self.acc.iter_mut().zip(lanes) {
            *acc = round(*acc, u32::from_le_bytes(*lane));
        }
    }

    /// Returns the XXH32 of the bytes fed, which it may go on from.
    #[must_use]
    pub fn finish(&self) -> u32 {
        let mut hash = if self.len >= 16 {
            let [a, b, c, d] = self.acc;
            a.rotate_left(1)
                .wrapping_add(b.rotate_left(7))
                .wrapping_add(c.rotate_left(12))
                .wrapping_add(d.rotate_left(18))
        } else {
            self.seed.wrapping_add(P5)
        };
        // Only the length's low 32 bits.
        let [a, b, c, d, ..] = self.len.to_le_bytes();
        hash = hash.wrapping_add(u32::from_le_bytes([a, b, c, d]));
        let tail = self.tail.get(..self.tailed).unwrap_or_default();
        let (words, rest) = tail.as_chunks::<4>();
        for word in words {
            hash = hash.wrapping_add(u32::from_le_bytes(*word).wrapping_mul(P3));
            hash = hash.rotate_left(17).wrapping_mul(P4);
        }
        for &byte in rest {
            hash = hash.wrapping_add(u32::from(byte).wrapping_mul(P5));
            hash = hash.rotate_left(11).wrapping_mul(P1);
        }
        hash ^= hash >> 15;
        hash = hash.wrapping_mul(P2);
        hash ^= hash >> 13;
        hash = hash.wrapping_mul(P3);
        hash ^ hash >> 16
    }
}

/// Takes `lane` into the accumulator `acc`.
fn round(acc: u32, lane: u32) -> u32 {
    acc.wrapping_add(lane.wrapping_mul(P2))
        .rotate_left(13)
        .wrapping_mul(P1)
}

impl Default for Xxh32 {
    /// From a seed of zero.
    fn default() -> Self {
        Self::new(0)
    }
}

/// Returns the XXH32 of `bytes`, from `seed`.
#[must_use]
pub fn xxh32(bytes: &[u8], seed: u32) -> u32 {
    Xxh32::new(seed).chain(bytes).finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_as_xxhash_does() {
        assert_eq!(xxh32(b"", 0), 0x02CC_5D05);
        assert_eq!(xxh32(b"a", 0), 0x550D_7456);
        assert_eq!(xxh32(b"abc", 0), 0x32D1_53FF);
    }

    #[test]
    fn hashes_the_same_in_pieces() {
        let bytes: [u8; 100] = core::array::from_fn(|n| (n * 37 % 251) as u8);
        for seed in [0, 1, u32::MAX] {
            for len in 0..bytes.len() {
                let whole = xxh32(&bytes[..len], seed);
                for piece in [1, 3, 15, 16, 17] {
                    let mut hash = Xxh32::new(seed);
                    for chunk in bytes[..len].chunks(piece) {
                        hash.update(chunk);
                    }
                    assert_eq!(hash.finish(), whole, "{len} in pieces of {piece}");
                }
            }
        }
    }
}

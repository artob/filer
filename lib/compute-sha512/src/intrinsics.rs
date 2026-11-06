// This is free and unencumbered software released into the public domain.

//! ARMv8.2's SHA-512 instructions, behind the `intrinsics` feature: chosen
//! at run time, with `std`, or else where the build enables them, by
//! `compute-multiversion`, a fork of `multiversion` that lets their
//! intrinsics be called without `unsafe` code here. Rust names their
//! extension `sha3`, which includes them. x86-64's SHA-512 extension is
//! on too few CPUs yet to be tested, and is not used.
//!
//! What each instruction computes was established by observation, each
//! compared with FIPS 180-4's functions over a million random inputs, lane
//! 0 the low half: `SHA512SU0(x, y)` is `[x0 + σ0(x1), x1 + σ0(y0)]`;
//! `SHA512SU1(x, y, z)` is `x + σ1(y) + z`, lane by lane; `SHA512H(a, b,
//! c)` is `hi = Ch(c1, b0, b1) + Σ1(c1) + a1`, then with `t = hi + c0`,
//! `lo = Ch(t, c1, b0) + Σ1(t) + a0`; and `SHA512H2(a, b, c)` is `hi =
//! Maj(b0, c1, c0) + Σ0(c0) + a1`, then `lo = Maj(hi, c0, c1) + Σ0(hi) +
//! a0`. So two rounds, from `a` to `h` and the two rounds' `K + W`, `kw`,
//! take `SHA512H([g, h] + [kw1, kw0], [f, g], [d, e])`, which is their
//! two `T1`s, `[t1', t1]`; the new `e`s are `[c, d] + [t1', t1]`, and the
//! new `a`s `SHA512H2([t1', t1], [c, d], [a, b])`.

/// The words of `block`, big-endian, two to a vector.
#[cfg(target_arch = "aarch64")]
fn pairs(block: &[u8; 128]) -> [[u64; 2]; 8] {
    let mut pairs = [[0; 2]; 8];
    for (pair, bytes) in pairs.iter_mut().zip(block.as_chunks::<16>().0) {
        for (word, bytes) in pair.iter_mut().zip(bytes.as_chunks::<8>().0) {
            *word = u64::from_be_bytes(*bytes);
        }
    }
    pairs
}

/// Takes `blocks` into `hash` by the CPU's SHA-512 instructions, and
/// returns whether it has them; if not, `hash` is as it was.
#[multiversion::multiversion(targets("aarch64+sha3"))]
pub(crate) fn compress(hash: &mut [u64; 8], blocks: &[[u8; 128]]) -> bool {
    #[multiversion::target::target_cfg(all(target_arch = "aarch64", target_feature = "sha3"))]
    #[multiversion::inherit_target]
    fn rounds(hash: &mut [u64; 8], blocks: &[[u8; 128]]) -> bool {
        use core::arch::aarch64::{
            uint64x2_t, vaddq_u64, vdupq_n_u64, vextq_u64, vgetq_lane_u64, vsetq_lane_u64,
            vsha512h2q_u64, vsha512hq_u64, vsha512su0q_u64, vsha512su1q_u64, vzip2q_u64,
        };
        let set = |[x0, x1]: [u64; 2]| vsetq_lane_u64::<1>(x1, vdupq_n_u64(x0));
        let get = |v: uint64x2_t| [vgetq_lane_u64::<0>(v), vgetq_lane_u64::<1>(v)];
        let mut k = [vdupq_n_u64(0); 40];
        for (k, pair) in k.iter_mut().zip(crate::K.as_chunks::<2>().0) {
            *k = set(*pair);
        }
        for block in blocks {
            let [a, b, c, d, e, f, g, h] = *hash;
            let (mut ab, mut cd, mut de) = (set([a, b]), set([c, d]), set([d, e]));
            let (mut fg, mut gh) = (set([f, g]), set([g, h]));
            let mut w = pairs(block).map(set);
            for &k in &k {
                let [w0, w1, w2, w3, w4, w5, w6, w7] = w;
                let kw = vaddq_u64(w0, k);
                let t1 = vsha512hq_u64(vaddq_u64(gh, vextq_u64::<1>(kw, kw)), fg, de);
                let es = vaddq_u64(cd, t1);
                let next_ab = vsha512h2q_u64(t1, cd, ab);
                // The state two rounds on: [e, f], [e', e], [b, e''], and
                // [a, b], from what it was.
                gh = vextq_u64::<1>(de, fg);
                fg = vzip2q_u64(es, de);
                de = vextq_u64::<1>(ab, es);
                cd = ab;
                ab = next_ab;
                // The schedule's next two words, from the last sixteen.
                let next = vsha512su0q_u64(w0, w1);
                let next = vsha512su1q_u64(next, w7, vextq_u64::<1>(w4, w5));
                w = [w1, w2, w3, w4, w5, w6, w7, next];
            }
            let ([a, b], [c, d], [_, e]) = (get(ab), get(cd), get(de));
            let ([f, g], [_, h]) = (get(fg), get(gh));
            for (hash, word) in hash.iter_mut().zip([a, b, c, d, e, f, g, h]) {
                *hash = hash.wrapping_add(word);
            }
        }
        true
    }

    #[multiversion::target::target_cfg(not(all(
        target_arch = "aarch64",
        target_feature = "sha3"
    )))]
    fn rounds(_: &mut [u64; 8], _: &[[u8; 128]]) -> bool {
        false
    }

    rounds(hash, blocks)
}

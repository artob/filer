// This is free and unencumbered software released into the public domain.

//! SHA-256's instructions: x86-64's SHA extensions, and ARMv8's SHA2,
//! behind the `intrinsics` feature: chosen at run time, with `std`, or else
//! where the build enables them, by `compute-multiversion`, a fork of
//! `multiversion` that lets their intrinsics be called without `unsafe`
//! code here.
//!
//! Each takes four rounds and four words of the message schedule an
//! instruction or two, over the schedule's last four words of four,
//! each as one vector.

/// The words of `block`, big-endian, four to a vector.
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
fn quads(block: &[u8; 64]) -> [[u32; 4]; 4] {
    let mut quads = [[0; 4]; 4];
    for (quad, bytes) in quads.iter_mut().zip(block.as_chunks::<16>().0) {
        for (word, bytes) in quad.iter_mut().zip(bytes.as_chunks::<4>().0) {
            *word = u32::from_be_bytes(*bytes);
        }
    }
    quads
}

/// Takes `blocks` into `hash` by the CPU's SHA-256 instructions, and
/// returns whether it has them; if not, `hash` is as it was.
#[multiversion::multiversion(targets("x86_64+sha+sse4.1+ssse3", "aarch64+sha2"))]
pub(crate) fn compress(hash: &mut [u32; 8], blocks: &[[u8; 64]]) -> bool {
    #[multiversion::target::target_cfg(all(target_arch = "x86_64", target_feature = "sha"))]
    #[multiversion::inherit_target]
    fn rounds(hash: &mut [u32; 8], blocks: &[[u8; 64]]) -> bool {
        use core::arch::x86_64::{
            __m128i, _mm_add_epi32, _mm_alignr_epi8, _mm_blend_epi16, _mm_extract_epi32,
            _mm_set_epi32, _mm_setzero_si128, _mm_sha256msg1_epu32, _mm_sha256msg2_epu32,
            _mm_sha256rnds2_epu32, _mm_shuffle_epi32,
        };
        let set = |[a, b, c, d]: [u32; 4]| {
            _mm_set_epi32(
                d.cast_signed(),
                c.cast_signed(),
                b.cast_signed(),
                a.cast_signed(),
            )
        };
        let get = |v: __m128i| {
            [
                _mm_extract_epi32::<0>(v).cast_unsigned(),
                _mm_extract_epi32::<1>(v).cast_unsigned(),
                _mm_extract_epi32::<2>(v).cast_unsigned(),
                _mm_extract_epi32::<3>(v).cast_unsigned(),
            ]
        };
        let mut k = [_mm_setzero_si128(); 16];
        for (k, quad) in k.iter_mut().zip(crate::K.as_chunks::<4>().0) {
            *k = set(*quad);
        }
        // The instructions take the state as ABEF and CDGH.
        let [a, b, c, d, e, f, g, h] = *hash;
        let dcba = _mm_shuffle_epi32::<0xB1>(set([a, b, c, d]));
        let efgh = _mm_shuffle_epi32::<0x1B>(set([e, f, g, h]));
        let mut abef = _mm_alignr_epi8::<8>(dcba, efgh);
        let mut cdgh = _mm_blend_epi16::<0xF0>(efgh, dcba);
        for block in blocks {
            let (abef0, cdgh0) = (abef, cdgh);
            let mut w = quads(block).map(set);
            for &k in &k {
                let [w0, w1, w2, w3] = w;
                let wk = _mm_add_epi32(w0, k);
                cdgh = _mm_sha256rnds2_epu32(cdgh, abef, wk);
                abef = _mm_sha256rnds2_epu32(abef, cdgh, _mm_shuffle_epi32::<0x0E>(wk));
                // The schedule's next four words, from the last sixteen.
                let next = _mm_sha256msg1_epu32(w0, w1);
                let next = _mm_add_epi32(next, _mm_alignr_epi8::<4>(w3, w2));
                w = [w1, w2, w3, _mm_sha256msg2_epu32(next, w3)];
            }
            abef = _mm_add_epi32(abef, abef0);
            cdgh = _mm_add_epi32(cdgh, cdgh0);
        }
        let feba = _mm_shuffle_epi32::<0x1B>(abef);
        let dchg = _mm_shuffle_epi32::<0xB1>(cdgh);
        let [a, b, c, d] = get(_mm_blend_epi16::<0xF0>(feba, dchg));
        let [e, f, g, h] = get(_mm_alignr_epi8::<8>(dchg, feba));
        *hash = [a, b, c, d, e, f, g, h];
        true
    }

    #[multiversion::target::target_cfg(all(target_arch = "aarch64", target_feature = "sha2"))]
    #[multiversion::inherit_target]
    fn rounds(hash: &mut [u32; 8], blocks: &[[u8; 64]]) -> bool {
        use core::arch::aarch64::{
            uint32x4_t, vaddq_u32, vdupq_n_u32, vgetq_lane_u32, vsetq_lane_u32, vsha256h2q_u32,
            vsha256hq_u32, vsha256su0q_u32, vsha256su1q_u32,
        };
        let set = |[a, b, c, d]: [u32; 4]| {
            let v = vsetq_lane_u32::<1>(b, vdupq_n_u32(a));
            vsetq_lane_u32::<3>(d, vsetq_lane_u32::<2>(c, v))
        };
        let get = |v: uint32x4_t| {
            [
                vgetq_lane_u32::<0>(v),
                vgetq_lane_u32::<1>(v),
                vgetq_lane_u32::<2>(v),
                vgetq_lane_u32::<3>(v),
            ]
        };
        let mut k = [vdupq_n_u32(0); 16];
        for (k, quad) in k.iter_mut().zip(crate::K.as_chunks::<4>().0) {
            *k = set(*quad);
        }
        let [a, b, c, d, e, f, g, h] = *hash;
        let (mut abcd, mut efgh) = (set([a, b, c, d]), set([e, f, g, h]));
        // Each of the two instructions of four rounds overwrites the half
        // of the state it takes on, and needs the other's as it was, so
        // one half is copied each time. Left to it, the compiler unrolls
        // the rounds and copies the half that the first instruction
        // overwrites, and the rounds wait on the copy, each four on the
        // four before. Summed with zeros that the compiler does not know
        // for zeros, the copy is the second instruction's, which waits on
        // less: 100 cycles a block on Neoverse V3, not 130. Apple's cores
        // wait on that sum too, but not on a plain copy that the second
        // instruction reads, as a loop carries it: there the rounds are a
        // loop of a count the compiler does not know, which it leaves
        // rolled, 83 cycles a block on M4, not 98. Each shape is the
        // slower on the other's cores (2026-11-13).
        #[cfg(target_vendor = "apple")]
        let groups = core::hint::black_box(k.len());
        #[cfg(not(target_vendor = "apple"))]
        let (groups, zeros) = (k.len(), vdupq_n_u32(core::hint::black_box(0)));
        for block in blocks {
            let (abcd0, efgh0) = (abcd, efgh);
            let mut w = quads(block).map(set);
            for &k in k.iter().take(groups) {
                let [w0, w1, w2, w3] = w;
                let wk = vaddq_u32(w0, k);
                #[cfg(target_vendor = "apple")]
                let before = abcd;
                #[cfg(not(target_vendor = "apple"))]
                let before = core::arch::aarch64::veorq_u32(abcd, zeros);
                abcd = vsha256hq_u32(abcd, efgh, wk);
                efgh = vsha256h2q_u32(efgh, before, wk);
                // The schedule's next four words, from the last sixteen;
                // those of the last four rounds go unused.
                w = [w1, w2, w3, vsha256su1q_u32(vsha256su0q_u32(w0, w1), w2, w3)];
            }
            abcd = vaddq_u32(abcd, abcd0);
            efgh = vaddq_u32(efgh, efgh0);
        }
        let ([a, b, c, d], [e, f, g, h]) = (get(abcd), get(efgh));
        *hash = [a, b, c, d, e, f, g, h];
        true
    }

    #[multiversion::target::target_cfg(not(any(
        all(target_arch = "x86_64", target_feature = "sha"),
        all(target_arch = "aarch64", target_feature = "sha2")
    )))]
    fn rounds(_: &mut [u32; 8], _: &[[u8; 64]]) -> bool {
        false
    }

    rounds(hash, blocks)
}

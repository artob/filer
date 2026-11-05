// This is free and unencumbered software released into the public domain.

//! XXH32 and XXH64 of bytes of every length to 1,000 and seeds at the
//! edges, checked against `twox-hash`'s.

#[cfg(feature = "xxh32")]
use compute_xxhash::{Xxh32, xxh32};
#[cfg(feature = "xxh64")]
use compute_xxhash::{Xxh64, xxh64};
#[cfg(feature = "xxh32")]
use twox_hash::XxHash32;
#[cfg(feature = "xxh64")]
use twox_hash::XxHash64;

#[cfg(feature = "xxh64")]
#[test]
fn hashes_as_twox_hash_does() {
    let bytes: Vec<u8> = (0..1000_u32)
        .map(|n| (n.wrapping_mul(2_654_435_761) >> 13) as u8)
        .collect();
    for seed in [0, 1, 0x9E37_79B1, u64::MAX] {
        for len in 0..=bytes.len() {
            let bytes = &bytes[..len];
            assert_eq!(
                xxh64(bytes, seed),
                XxHash64::oneshot(seed, bytes),
                "{len} {seed}"
            );
        }
        let mut hash = Xxh64::new(seed);
        for chunk in bytes.chunks(7) {
            hash.update(chunk);
        }
        assert_eq!(hash.finish(), XxHash64::oneshot(seed, &bytes));
    }
}

#[cfg(feature = "xxh32")]
#[test]
fn hashes_xxh32_as_twox_hash_does() {
    let bytes: Vec<u8> = (0..1000_u32)
        .map(|n| (n.wrapping_mul(2_654_435_761) >> 13) as u8)
        .collect();
    for seed in [0, 1, 0x9E37_79B1, u32::MAX] {
        for len in 0..=bytes.len() {
            let bytes = &bytes[..len];
            assert_eq!(
                xxh32(bytes, seed),
                XxHash32::oneshot(seed, bytes),
                "{len} {seed}"
            );
        }
        let mut hash = Xxh32::new(seed);
        for chunk in bytes.chunks(7) {
            hash.update(chunk);
        }
        assert_eq!(hash.finish(), XxHash32::oneshot(seed, &bytes));
    }
}

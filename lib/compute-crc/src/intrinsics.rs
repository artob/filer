// This is free and unencumbered software released into the public domain.

//! The CRC instructions of x86-64's SSE4.2, for CRC-32C, and of ARM's
//! `crc` extension, for CRC-32 and CRC-32C, behind the `intrinsics`
//! feature: chosen at run time, with `std`, or else where the build enables
//! them, by `compute-multiversion`, a fork of `multiversion` that lets
//! their intrinsics be called without `unsafe` code here.
//!
//! Each takes the register eight bytes an instruction, from any state, as
//! the tables do, and returns what it comes to; or `None` where the CPU
//! lacks the instruction, and the tables are to be used. Each runs as one
//! stream: none splits the bytes into streams to combine.

/// Feeds `bytes` to a CRC-32C register, by SSE4.2's `crc32` or ARM's
/// `crc32c*`.
#[cfg(feature = "crc32c")]
#[multiversion::multiversion(targets("x86_64+sse4.2", "aarch64+crc"))]
pub(crate) fn crc32c(state: u32, bytes: &[u8]) -> Option<u32> {
    #[multiversion::target::target_cfg(all(target_arch = "x86_64", target_feature = "sse4.2"))]
    #[multiversion::inherit_target]
    fn feed(state: u32, bytes: &[u8]) -> Option<u32> {
        use core::arch::x86_64::{_mm_crc32_u8, _mm_crc32_u64};
        let (words, rest) = bytes.as_chunks::<8>();
        let crc = words.iter().fold(u64::from(state), |crc, word| {
            _mm_crc32_u64(crc, u64::from_le_bytes(*word))
        });
        // Always fits: the instruction leaves the top half zero.
        let crc = u32::try_from(crc).unwrap_or_default();
        Some(rest.iter().fold(crc, |crc, &byte| _mm_crc32_u8(crc, byte)))
    }

    #[multiversion::target::target_cfg(all(target_arch = "aarch64", target_feature = "crc"))]
    #[multiversion::inherit_target]
    fn feed(state: u32, bytes: &[u8]) -> Option<u32> {
        use core::arch::aarch64::{__crc32cb, __crc32cd};
        let (words, rest) = bytes.as_chunks::<8>();
        let crc = words
            .iter()
            .fold(state, |crc, word| __crc32cd(crc, u64::from_le_bytes(*word)));
        Some(rest.iter().fold(crc, |crc, &byte| __crc32cb(crc, byte)))
    }

    #[multiversion::target::target_cfg(not(any(
        all(target_arch = "x86_64", target_feature = "sse4.2"),
        all(target_arch = "aarch64", target_feature = "crc")
    )))]
    fn feed(_: u32, _: &[u8]) -> Option<u32> {
        None
    }

    feed(state, bytes)
}

/// Feeds `bytes` to a CRC-32 register, by ARM's `crc32*`; x86 has no
/// instruction for it.
#[cfg(feature = "crc32")]
#[multiversion::multiversion(targets("aarch64+crc"))]
pub(crate) fn crc32(state: u32, bytes: &[u8]) -> Option<u32> {
    #[multiversion::target::target_cfg(all(target_arch = "aarch64", target_feature = "crc"))]
    #[multiversion::inherit_target]
    fn feed(state: u32, bytes: &[u8]) -> Option<u32> {
        use core::arch::aarch64::{__crc32b, __crc32d};
        let (words, rest) = bytes.as_chunks::<8>();
        let crc = words
            .iter()
            .fold(state, |crc, word| __crc32d(crc, u64::from_le_bytes(*word)));
        Some(rest.iter().fold(crc, |crc, &byte| __crc32b(crc, byte)))
    }

    #[multiversion::target::target_cfg(not(all(target_arch = "aarch64", target_feature = "crc")))]
    fn feed(_: u32, _: &[u8]) -> Option<u32> {
        None
    }

    feed(state, bytes)
}

/// Feeds `bytes` to a CRC-32/BZIP2 register, by ARM's `crc32*`: CRC-32's
/// polynomial, read from each byte's most significant bit, as the
/// instructions read from its least with the bits of the register and of
/// each word reversed.
#[cfg(feature = "crc32-bzip2")]
#[multiversion::multiversion(targets("aarch64+crc"))]
pub(crate) fn crc32_bzip2(state: u32, bytes: &[u8]) -> Option<u32> {
    #[multiversion::target::target_cfg(all(target_arch = "aarch64", target_feature = "crc"))]
    #[multiversion::inherit_target]
    fn feed(state: u32, bytes: &[u8]) -> Option<u32> {
        use core::arch::aarch64::{__crc32b, __crc32d};
        let (words, rest) = bytes.as_chunks::<8>();
        let crc = words.iter().fold(state.reverse_bits(), |crc, word| {
            __crc32d(crc, u64::from_be_bytes(*word).reverse_bits())
        });
        let crc = rest
            .iter()
            .fold(crc, |crc, &byte| __crc32b(crc, byte.reverse_bits()));
        Some(crc.reverse_bits())
    }

    #[multiversion::target::target_cfg(not(all(target_arch = "aarch64", target_feature = "crc")))]
    fn feed(_: u32, _: &[u8]) -> Option<u32> {
        None
    }

    feed(state, bytes)
}

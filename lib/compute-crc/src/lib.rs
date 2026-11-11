// This is free and unencumbered software released into the public domain.

//! Cyclic redundancy checks, written clean room from their parameters, and
//! dedicated to the public domain: those that archives and file systems
//! stamp on what they store, each a type and a feature of its own, all
//! enabled by `all`.
//!
//! | Type | CRC | Feature | Stamped by |
//! | --- | --- | --- | --- |
//! | [`Crc16Modbus`] | CRC-16/MODBUS | `crc16-modbus` | ext4's group descriptors, before `metadata_csum` |
//! | [`Crc32`] | CRC-32/ISO-HDLC | `crc32` | gzip, PNG, ZIP, xz, GPT |
//! | [`Crc32c`] | CRC-32/ISCSI | `crc32c` | ext4, XFS, Btrfs, iSCSI |
//! | [`Crc32Bzip2`] | CRC-32/BZIP2 | `crc32-bzip2` | bzip2 |
//! | [`Crc64Xz`] | CRC-64/XZ | `crc64-xz` | xz |
//!
//! Each computes over bytes fed in pieces, as they arrive, by
//! [`update`](Crc32::update), or by [`chain`](Crc32::chain), which returns
//! the CRC that goes on from there; each is a `Copy` value, so that a copy
//! taken anywhere goes on from there. [`state`](Crc32::state) and
//! [`from_state`](Crc32::from_state) expose the register, before the final
//! inversion, which formats that chain checksums store, as ext4 does.
//! A function of each, such as [`crc32`], computes it over one slice.
//!
//! They take sixteen bytes at a time, by sixteen tables of each byte
//! followed by none to fifteen zeros, built at compile time, and the rest
//! by eight, four, two, and one, by the same tables; a variant's tables
//! are in a build only if its feature is. By the tables alone, they are
//! as fast as zlib's CRC-32 on x86-64, and faster than the `crc` crate's,
//! whole and in pieces (`BENCH.md`). The sources are each CRC's
//! parameters, its polynomial, its bit order, and its start and final
//! values, which are facts, as the catalogue of CRCs lists them; no
//! other implementation's code is copied. It is pure, safe Rust without
//! dependencies, and builds with `#![no_std]`, without an allocator. The
//! `std` feature, enabled by default, adds nothing by itself.
//!
//! The `intrinsics` feature, enabled by default (D66), takes the CPU's
//! instructions where it has them: with `std`, chosen at run time;
//! without it, only where the build enables them. It depends on
//! `compute-multiversion`, a fork of `multiversion` that lets their
//! intrinsics be called without `unsafe` code here.
//!
//! On x86-64, they are carry-less multiplication's, for every CRC here,
//! from 64 bytes up: PCLMULQDQ's of 128 bits, VPCLMULQDQ's of 256, and of
//! 512 where the CPU does not slow its clock for them. A CRC is linear,
//! so each 64-bit word of a block is multiplied by its own weight, `x` to
//! the power of the bits after it modulo the polynomial, of a table of 16
//! KiB a CRC; the products are summed, none waiting on another; and the
//! sum is fed to the byte tables, once a block of 16 KiB. No remainder is
//! folded along the message, which a patent in force until 2029 claims
//! (`LEGAL.md`). Over a mebibyte, 66 to 67 GB/s on Zen 5, where the
//! instruction's own pace allows 72, and 78 to 101 on Granite Rapids
//! (`BENCH.md`). CRC-32C takes fewer than 512 bytes by SSE4.2's `crc32`.
//!
//! On ARM, they are PMULL's, of 128 bits, in the same sums, for every CRC
//! here, from 64 bytes up: over a mebibyte, 40 to 41 GB/s on Neoverse V3,
//! and 31 for CRC-32/BZIP2, whose bytes each vector turns about; on Apple
//! M4, 75 to 77, and 51. CRC-32 and CRC-32C take fewer than 384 bytes,
//! and CRC-32/BZIP2 fewer than 256, or each fewer than 128 built for
//! Apple's targets, faster so, and all of them where the CPU lacks PMULL,
//! by the `crc` extension's instructions: eight bytes each, from any
//! register, as the tables do, in three streams of the bytes at once where
//! there are enough, which combine by the CRC's own arithmetic, as one
//! instruction waits on the one before.
//!
//! Without `intrinsics`, as `default-features = false,
//! features = ["all", "std"]` builds it, it has no dependencies, all of it
//! public domain, and takes every CRC by the tables: over long inputs, 9
//! to 15 times slower on Zen 5, 4.6 to 7.1 GB/s; 16 to 23 times on Granite
//! Rapids, 3.8 to 5.3; 9 to 12 times on Neoverse V3, 3.4 to 4.2; and 10
//! to 16 times on Apple M4, 4.6 to 5.8 (2026-11-11).
//!
//! It is one of the `compute-*` crates: clean-room, public-domain
//! checksums and hashes, each of the same shape: `new`, `update`, `chain`,
//! and `finish`.
//!
//! # Examples
//!
//! ```
//! # #[cfg(feature = "crc32")] {
//! use compute_crc::{Crc32, crc32};
//!
//! assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
//! let mut crc = Crc32::new();
//! crc.update(b"1234");
//! crc.update(b"56789");
//! assert_eq!(crc.finish(), 0xCBF4_3926);
//! assert_eq!(Crc32::new().chain(b"1234").chain(b"56789"), crc);
//! # }
//! ```

#![no_std]
#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
// No input may make this crate panic.
#![deny(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::let_underscore_must_use,
    clippy::panic,
    clippy::string_slice,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable,
    clippy::unwrap_used,
    unused_must_use
)]
// Tests may panic: that is how they fail.
#![cfg_attr(
    test,
    allow(
        clippy::arithmetic_side_effects,
        clippy::as_conversions,
        clippy::expect_used,
        clippy::indexing_slicing,
        clippy::let_underscore_must_use,
        clippy::panic,
        clippy::unreachable,
        clippy::unwrap_used
    )
)]

/// Defines a CRC of the register type `$reg`, with the polynomial `$poly`,
/// its bits reversed if `reflected`, starting from all ones: its tables,
/// its type, and its function. A reflected CRC reads each byte from its
/// least significant bit, and shifts its register down; one that is not
/// reads from the most significant, and shifts up. `$invert` is whether
/// the final value is inverted. `$instructions`, if given, is a function of
/// the `intrinsics` module that feeds the register by the CPU's
/// instructions, if it has them. `$least`, if given, is the fewest bytes
/// that x86-64, ARM, and Apple's ARM multiply, in that order, where they
/// have instructions that take fewer faster: at least `carryless::LEAST`.
macro_rules! crc {
    (
        $(#[$meta:meta])*
        $name:ident, $function:ident, $feature:literal, $reg:ty, $poly:expr, $tables:ident,
        reflected: $reflected:literal, invert: $invert:literal, check: $check:expr,
        label: $label:literal $(, instructions: $instructions:path)?
        $(, least: ($x86:literal, $arm:literal, $apple:literal))?
    ) => {
        /// The tables for taking sixteen bytes at a time: `TABLES[0][b]`
        /// is the CRC of byte `b`, in the register's byte that shifts out
        /// first, and `TABLES[k][b]` that of `b` followed by `k` zero
        /// bytes. A `static`, as a `const` would be copied wherever it is
        /// used, unless optimized away.
        #[cfg(feature = $feature)]
        static $tables: [[$reg; 256]; 16] = {
            // Evaluated at compile time, where any overflow or index out
            // of range fails the build instead of panicking.
            #[allow(
                clippy::arithmetic_side_effects,
                clippy::as_conversions,
                clippy::indexing_slicing,
                reason = "evaluated at compile time"
            )]
            const fn build() -> [[$reg; 256]; 16] {
                const TOP: u32 = <$reg>::BITS - 8;
                let mut tables = [[0; 256]; 16];
                let mut byte = 0;
                while byte < 256 {
                    let mut crc = if $reflected {
                        byte as $reg
                    } else {
                        (byte as $reg) << TOP
                    };
                    let mut bit = 0;
                    while bit < 8 {
                        crc = if $reflected {
                            if crc & 1 == 0 { crc >> 1 } else { (crc >> 1) ^ $poly }
                        } else if crc >> (TOP + 7) == 0 {
                            crc << 1
                        } else {
                            (crc << 1) ^ $poly
                        };
                        bit += 1;
                    }
                    tables[0][byte] = crc;
                    byte += 1;
                }
                let mut k = 1;
                while k < 16 {
                    let mut byte = 0;
                    while byte < 256 {
                        let previous = tables[k - 1][byte];
                        tables[k][byte] = if $reflected {
                            (previous >> 8) ^ tables[0][(previous & 0xFF) as usize]
                        } else {
                            (previous << 8) ^ tables[0][(previous >> TOP) as usize]
                        };
                        byte += 1;
                    }
                    k += 1;
                }
                tables
            }
            build()
        };

        $(#[$meta])*
        #[cfg(feature = $feature)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub struct $name {
            /// The register, which starts as all ones.
            state: $reg,
        }

        #[cfg(feature = $feature)]
        impl $name {
            #[doc = concat!("Starts a ", $label, " over no bytes.")]
            #[must_use]
            pub const fn new() -> Self {
                Self { state: !0 }
            }

            /// Feeds `bytes`.
            pub fn update(&mut self, bytes: &[u8]) {
                // Too few bytes to multiply stay here, a comparison away
                // from the tables.
                #[cfg(all(
                feature = "intrinsics",
                any(target_arch = "x86_64", target_arch = "aarch64")
            ))]
                let bytes = if bytes.len() < Self::LEAST {
                    bytes
                } else {
                    let (state, bytes) = Self::multiplied(self.state, bytes, usize::MAX);
                    self.state = state;
                    bytes
                };
                $(
                    #[cfg(feature = "intrinsics")]
                    if let Some(state) = $instructions(self.state, bytes) {
                        self.state = state;
                        return;
                    }
                )?
                self.state = Self::tables(self.state, bytes);
            }

            /// The fewest bytes that `multiplied` takes.
            #[cfg(all(
                feature = "intrinsics",
                any(target_arch = "x86_64", target_arch = "aarch64")
            ))]
            const LEAST: usize = {
                let least = carryless::LEAST;
                $(
                    let own = if cfg!(target_arch = "x86_64") {
                        $x86
                    } else if cfg!(target_vendor = "apple") {
                        $apple
                    } else {
                        $arm
                    };
                    let least = if own > least { own } else { least };
                )?
                least
            };

            /// Feeds the units of sixteen bytes that `bytes` begins with to
            /// the register `crc`, by the CPU's carry-less multiplication,
            /// where it has one and they are enough to gain by it, a block
            /// at a time: each block's sum, of `carryless`, fed to the
            /// tables from zero. Returns the register and the bytes left,
            /// all of them if it took none. `widest` is the widest vector
            /// to take, in bits.
            #[cfg(all(
                feature = "intrinsics",
                any(target_arch = "x86_64", target_arch = "aarch64")
            ))]
            #[inline(never)]
            fn multiplied(mut crc: $reg, mut bytes: &[u8], widest: usize) -> ($reg, &[u8]) {
                #[allow(clippy::as_conversions, reason = "evaluated at compile time")]
                static WEIGHTS: carryless::Weights =
                    carryless::weights(<$reg>::BITS, $poly as u64, $reflected);
                // More than a block: from where each vector's bytes are
                // in one line of the cache, which the widest, of 64 bytes,
                // would else span two of, slower read.
                if bytes.len() > carryless::UNITS.saturating_mul(carryless::UNIT) {
                    let head = bytes.as_ptr().align_offset(64);
                    if let Some((head, rest)) = bytes.split_at_checked(head) {
                        (crc, bytes) = (Self::tables(crc, head), rest);
                    }
                }
                while bytes.len() >= Self::LEAST {
                    let (units, _) = bytes.as_chunks::<{ carryless::UNIT }>();
                    let units = units.get(..carryless::UNITS).unwrap_or(units);
                    // A shorter block's words are the last of a whole one's.
                    let first = carryless::UNITS.saturating_sub(units.len());
                    let Some(weights) = WEIGHTS.0.get(first..) else {
                        break;
                    };
                    // The register, where the block's first bits are.
                    let register = if $reflected {
                        u64::from(crc)
                    } else {
                        let up = u64::BITS.wrapping_sub(<$reg>::BITS);
                        u64::from(crc).checked_shl(up).unwrap_or(0)
                    };
                    let sum = carryless::sum(register, units, weights, !$reflected, widest);
                    let Some(sum) = sum else {
                        break;
                    };
                    crc = Self::group(
                        0,
                        if $reflected {
                            sum.to_le_bytes()
                        } else {
                            sum.to_be_bytes()
                        },
                    );
                    bytes = bytes.get(units.as_flattened().len()..).unwrap_or_default();
                }
                (crc, bytes)
            }

            /// Feeds `bytes` to the register `crc` by the tables, and
            /// returns what it comes to: sixteen bytes at a time, and the
            /// rest by eight, four, two, and one, each at most once.
            fn tables(mut crc: $reg, bytes: &[u8]) -> $reg {
                let (groups, rest) = bytes.as_chunks::<16>();
                for group in groups {
                    crc = Self::group(crc, *group);
                }
                let (groups, rest) = rest.as_chunks::<8>();
                for group in groups {
                    crc = Self::group(crc, *group);
                }
                let (groups, rest) = rest.as_chunks::<4>();
                for group in groups {
                    crc = Self::group(crc, *group);
                }
                let (groups, rest) = rest.as_chunks::<2>();
                for group in groups {
                    crc = Self::group(crc, *group);
                }
                for &byte in rest {
                    crc = Self::group(crc, [byte]);
                }
                crc
            }

            /// Feeds `N` bytes, at most sixteen, to the register `crc` by
            /// the tables. The register's bytes go into the group's first,
            /// the one that shifts out first into the first, and each byte
            /// of the group out by its table, that of the zeros after it:
            /// last to first, so that those which wait on the register come
            /// last. What the group does not shift out shifts along.
            #[inline(always)]
            fn group<const N: usize>(crc: $reg, mut group: [u8; N]) -> $reg {
                // Always found: `k` is below 16, and a byte below 256.
                let table = |k: usize, byte: u8| {
                    $tables
                        .get(k)
                        .and_then(|table| table.get(usize::from(byte)))
                        .copied()
                        .unwrap_or(0)
                };
                // As a word where the group has one, which takes fewer of
                // the CPU's loads than byte by byte: 14% faster on Zen 1.
                if let Some((head, _)) = group.split_first_chunk_mut::<8>() {
                    *head = if $reflected {
                        (u64::from_le_bytes(*head) ^ u64::from(crc)).to_le_bytes()
                    } else {
                        // The register's top byte, the first to shift out,
                        // into the word's.
                        let up = u64::BITS.wrapping_sub(<$reg>::BITS);
                        let register = u64::from(crc).checked_shl(up).unwrap_or(0);
                        (u64::from_be_bytes(*head) ^ register).to_be_bytes()
                    };
                } else {
                    let register = if $reflected {
                        crc.to_le_bytes()
                    } else {
                        crc.to_be_bytes()
                    };
                    for (byte, reg) in group.iter_mut().zip(register) {
                        *byte ^= reg;
                    }
                }
                // What of the register is past the group, shifted along;
                // and the bytes, into two sums, which halves the chain of
                // operations that each waits on the one before.
                let bits = u32::try_from(N.saturating_mul(8)).unwrap_or(u32::MAX);
                let mut even = if $reflected {
                    crc.checked_shr(bits).unwrap_or(0)
                } else {
                    crc.checked_shl(bits).unwrap_or(0)
                };
                let mut odd = 0;
                for (k, &byte) in group.iter().enumerate().rev() {
                    let term = table(N.wrapping_sub(1).wrapping_sub(k), byte);
                    if k % 2 == 0 {
                        even ^= term;
                    } else {
                        odd ^= term;
                    }
                }
                even ^ odd
            }

            /// Feeds `bytes`, and returns the CRC that goes on from there,
            /// so that calls chain.
            #[must_use]
            pub fn chain(mut self, bytes: &[u8]) -> Self {
                self.update(bytes);
                self
            }

            #[doc = concat!("Returns the ", $label, " of the bytes fed.")]
            #[must_use]
            pub const fn finish(&self) -> $reg {
                if $invert { !self.state } else { self.state }
            }

            #[doc = concat!("Resumes a ", $label, " from `state`, a register as")]
            /// [`state`](Self::state) returns it.
            #[must_use]
            pub const fn from_state(state: $reg) -> Self {
                Self { state }
            }

            /// Returns the register: the CRC of the bytes fed, before any
            /// final inversion.
            #[must_use]
            pub const fn state(&self) -> $reg {
                self.state
            }

            /// The CRC of the nine bytes `123456789`, which identifies it.
            pub const CHECK: $reg = $check;
        }

        #[cfg(feature = $feature)]
        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        #[doc = concat!("Returns the ", $label, " of `bytes`.")]
        #[cfg(feature = $feature)]
        #[must_use]
        pub fn $function(bytes: &[u8]) -> $reg {
            $name::new().chain(bytes).finish()
        }
    };
}

// Detecting the CPU's instructions at run time takes `std`.
#[cfg(all(feature = "intrinsics", feature = "std"))]
extern crate std;

#[cfg(feature = "intrinsics")]
mod intrinsics;

#[cfg(all(
    feature = "intrinsics",
    any(target_arch = "x86_64", target_arch = "aarch64"),
    any(
        feature = "crc16-modbus",
        feature = "crc32",
        feature = "crc32-bzip2",
        feature = "crc32c",
        feature = "crc64-xz"
    )
))]
mod carryless;

crc! {
    /// CRC-16/MODBUS: the polynomial 0x8005, each byte read from its least
    /// significant bit, from `0xFFFF`, and not inverted at the end; which
    /// ext4 stamps on its group descriptors without `metadata_csum`.
    ///
    /// ```
    /// use compute_crc::{Crc16Modbus, crc16_modbus};
    ///
    /// assert_eq!(crc16_modbus(b"123456789"), 0x4B37);
    /// let crc = Crc16Modbus::new().chain(b"1234").chain(b"56789");
    /// assert_eq!(crc.finish(), Crc16Modbus::CHECK);
    /// ```
    Crc16Modbus, crc16_modbus, "crc16-modbus", u16, 0xA001, CRC16_MODBUS,
    reflected: true, invert: false, check: 0x4B37, label: "CRC-16/MODBUS"
}

crc! {
    /// CRC-32 (CRC-32/ISO-HDLC): the polynomial 0x04C11DB7, each byte read
    /// from its least significant bit, from all ones, and inverted at the
    /// end; which gzip, PNG, ZIP, xz, and GPT stamp.
    ///
    /// ```
    /// use compute_crc::{Crc32, crc32};
    ///
    /// assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    /// let crc = Crc32::new().chain(b"1234").chain(b"56789");
    /// assert_eq!(crc.finish(), Crc32::CHECK);
    /// ```
    Crc32, crc32, "crc32", u32, 0xEDB8_8320, CRC32,
    reflected: true, invert: true, check: 0xCBF4_3926, label: "CRC-32",
    instructions: intrinsics::crc32, least: (0, 384, 128)
}

crc! {
    /// CRC-32C (CRC-32/ISCSI), Castagnoli's: the polynomial 0x1EDC6F41, as
    /// CRC-32 is otherwise; which ext4, XFS, and Btrfs stamp on their
    /// metadata, and iSCSI on its frames.
    ///
    /// Formats that chain checksums store the register rather than the
    /// CRC: ext4 its metadata checksums and their seed, and JBD2 its own,
    /// each continuing the register of the seed over its bytes.
    ///
    /// ```
    /// use compute_crc::{Crc32c, crc32c};
    ///
    /// assert_eq!(crc32c(b"123456789"), 0xE306_9283);
    /// let seed = Crc32c::new().chain(b"1234").state();
    /// let crc = Crc32c::from_state(seed).chain(b"56789");
    /// assert_eq!(crc.finish(), Crc32c::CHECK);
    /// assert_eq!(crc.state(), !0xE306_9283);
    /// ```
    Crc32c, crc32c, "crc32c", u32, 0x82F6_3B78, CRC32C,
    reflected: true, invert: true, check: 0xE306_9283, label: "CRC-32C",
    instructions: intrinsics::crc32c, least: (512, 384, 128)
}

crc! {
    /// CRC-64/XZ: the polynomial 0x42F0E1EBA9EA3693 of ECMA-182, each byte
    /// read from its least significant bit, from all ones, and inverted at
    /// the end; which xz stamps on blocks.
    ///
    /// ```
    /// use compute_crc::{Crc64Xz, crc64_xz};
    ///
    /// assert_eq!(crc64_xz(b"123456789"), 0x995D_C9BB_DF19_39FA);
    /// let crc = Crc64Xz::new().chain(b"1234").chain(b"56789");
    /// assert_eq!(crc.finish(), Crc64Xz::CHECK);
    /// ```
    Crc64Xz, crc64_xz, "crc64-xz", u64, 0xC96C_5795_D787_0F42, CRC64_XZ,
    reflected: true, invert: true, check: 0x995D_C9BB_DF19_39FA, label: "CRC-64/XZ"
}

crc! {
    /// CRC-32/BZIP2: CRC-32's polynomial, 0x04C11DB7, but each byte read from
    /// its most significant bit; from all ones, and inverted at the end;
    /// which bzip2 stamps on blocks.
    ///
    /// ```
    /// use compute_crc::{Crc32Bzip2, crc32_bzip2};
    ///
    /// assert_eq!(crc32_bzip2(b"123456789"), 0xFC89_1918);
    /// let crc = Crc32Bzip2::new().chain(b"1234").chain(b"56789");
    /// assert_eq!(crc.finish(), Crc32Bzip2::CHECK);
    /// ```
    Crc32Bzip2, crc32_bzip2, "crc32-bzip2", u32, 0x04C1_1DB7, CRC32_BZIP2,
    reflected: false, invert: true, check: 0xFC89_1918, label: "CRC-32/BZIP2",
    instructions: intrinsics::crc32_bzip2, least: (0, 256, 128)
}

// The README's example runs as a doctest, with the CRCs it uses.
#[cfg(all(doctest, feature = "crc32", feature = "crc32c"))]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

// The tests need at least one CRC; each checks the ones in the build.
#[cfg(all(
    test,
    any(
        feature = "crc16-modbus",
        feature = "crc32",
        feature = "crc32-bzip2",
        feature = "crc32c",
        feature = "crc64-xz"
    )
))]
mod tests {
    use super::*;

    /// Computes a CRC bit by bit, from its parameters: the slow way, as
    /// the catalogue of CRCs defines them.
    fn bitwise(width: u32, poly: u64, reflected: bool, invert: bool, bytes: &[u8]) -> u64 {
        let top = 1_u64 << (width - 1);
        let mask = if width == 64 {
            u64::MAX
        } else {
            (1 << width) - 1
        };
        let mut crc = mask;
        for &byte in bytes {
            let byte = if reflected { byte.reverse_bits() } else { byte };
            crc ^= u64::from(byte) << (width - 8);
            for _ in 0..8 {
                crc = if crc & top != 0 {
                    (crc << 1) ^ poly
                } else {
                    crc << 1
                } & mask;
            }
        }
        if reflected {
            crc = crc.reverse_bits() >> (64 - width);
        }
        if invert { !crc & mask } else { crc }
    }

    /// Bytes of every length to 100, in pieces of every size to 17.
    fn each(check: impl Fn(&[u8], usize)) {
        let bytes: [u8; 100] = core::array::from_fn(|n| (n * 37 % 251) as u8);
        for len in 0..=bytes.len() {
            for piece in 1..=17 {
                check(&bytes[..len], piece);
            }
        }
    }

    /// Returns what `start` comes to, fed `bytes` in pieces of `piece`.
    fn pieces<T>(start: T, bytes: &[u8], piece: usize, update: impl Fn(T, &[u8]) -> T) -> T {
        bytes.chunks(piece).fold(start, update)
    }

    /// The CPU's instructions, where it has them, come to what the tables
    /// do, from any register: in one stream, and in blocks of three that
    /// combine, whole or with bytes to spare.
    #[cfg(all(
        feature = "intrinsics",
        any(feature = "crc32", feature = "crc32-bzip2", feature = "crc32c")
    ))]
    #[test]
    fn instructions_match_the_tables() {
        let bytes: [u8; 20_000] = core::array::from_fn(|n| (n * 131 % 251) as u8);
        let blocks = [3071, 3072, 3073, 3079, 6144, 6151, 9215, 20_000];
        for state in [0, !0, 0x1234_5678, 0xDEAD_BEEF] {
            for len in (0..=64).chain([100, 999, 1000]).chain(blocks) {
                let bytes = &bytes[..len];
                #[cfg(feature = "crc32")]
                assert_eq!(
                    Crc32::from_state(state).chain(bytes).state(),
                    Crc32::tables(state, bytes)
                );
                #[cfg(feature = "crc32c")]
                assert_eq!(
                    Crc32c::from_state(state).chain(bytes).state(),
                    Crc32c::tables(state, bytes)
                );
                #[cfg(feature = "crc32-bzip2")]
                assert_eq!(
                    Crc32Bzip2::from_state(state).chain(bytes).state(),
                    Crc32Bzip2::tables(state, bytes)
                );
            }
        }
    }

    /// The CPU's carry-less multiplication, where it has one, comes to
    /// what the tables do, from any register: in vectors of each width, over
    /// less than a block, a block, and several, with bytes to spare.
    #[cfg(all(
        feature = "intrinsics",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    #[test]
    fn multiplication_matches_the_tables() {
        let bytes: [u8; 70_000] = core::array::from_fn(|n| (n * 131 % 251) as u8);
        let block = carryless::UNITS * carryless::UNIT;
        let blocks = [block - 1, block, block + 1, block + 15, block + 16];
        let blocks = blocks
            .into_iter()
            .chain([2 * block, 2 * block + 17, 4 * block + 99]);
        // Whether the CPU multiplies without carries.
        #[cfg(all(feature = "std", target_arch = "x86_64"))]
        let multiplies = std::is_x86_feature_detected!("pclmulqdq");
        #[cfg(all(feature = "std", target_arch = "aarch64"))]
        let multiplies = std::arch::is_aarch64_feature_detected!("aes");
        for widest in [128, 256, 512] {
            for state in [0, !0, 0x1234_5678_9ABC_DEF0_u64, 0xDEAD_BEEF_0BAD_F00D] {
                let lens = (0..=300)
                    .chain([999, 1000, 1023, 1024])
                    .chain(blocks.clone());
                for (n, len) in lens.enumerate() {
                    // From every place in a line of the cache, by turns.
                    let bytes = &bytes[n % 64..][..len];
                    macro_rules! check {
                        ($name:ident, $reg:ty) => {{
                            let state = state as $reg;
                            let (crc, rest) = $name::multiplied(state, bytes, widest);
                            // With the instruction, all but too few bytes
                            // to gain by it.
                            #[cfg(feature = "std")]
                            if multiplies {
                                assert!(rest.len() < $name::LEAST);
                            }
                            assert_eq!(
                                $name::tables(crc, rest),
                                $name::tables(state, bytes),
                                "{} of {len} bytes from {state:#x} by {widest} bits",
                                stringify!($name)
                            );
                        }};
                    }
                    #[cfg(feature = "crc16-modbus")]
                    check!(Crc16Modbus, u16);
                    #[cfg(feature = "crc32")]
                    check!(Crc32, u32);
                    #[cfg(feature = "crc32c")]
                    check!(Crc32c, u32);
                    #[cfg(feature = "crc32-bzip2")]
                    check!(Crc32Bzip2, u32);
                    #[cfg(feature = "crc64-xz")]
                    check!(Crc64Xz, u64);
                }
            }
        }
    }

    #[test]
    fn matches_the_bitwise_definitions() {
        each(|bytes, piece| {
            #[cfg(feature = "crc16-modbus")]
            {
                let crc = pieces(Crc16Modbus::new(), bytes, piece, Crc16Modbus::chain);
                let poly = 0x8005;
                assert_eq!(
                    u64::from(crc.finish()),
                    bitwise(16, poly, true, false, bytes)
                );
            }
            #[cfg(feature = "crc32")]
            {
                let crc = pieces(Crc32::new(), bytes, piece, Crc32::chain);
                let poly = 0x04C1_1DB7;
                assert_eq!(
                    u64::from(crc.finish()),
                    bitwise(32, poly, true, true, bytes)
                );
            }
            #[cfg(feature = "crc32c")]
            {
                let crc = pieces(Crc32c::new(), bytes, piece, Crc32c::chain);
                let poly = 0x1EDC_6F41;
                assert_eq!(
                    u64::from(crc.finish()),
                    bitwise(32, poly, true, true, bytes)
                );
            }
            #[cfg(feature = "crc32-bzip2")]
            {
                let crc = pieces(Crc32Bzip2::new(), bytes, piece, Crc32Bzip2::chain);
                let poly = 0x04C1_1DB7;
                assert_eq!(
                    u64::from(crc.finish()),
                    bitwise(32, poly, false, true, bytes)
                );
            }
            #[cfg(feature = "crc64-xz")]
            {
                let crc = pieces(Crc64Xz::new(), bytes, piece, Crc64Xz::chain);
                let poly = 0x42F0_E1EB_A9EA_3693;
                assert_eq!(crc.finish(), bitwise(64, poly, true, true, bytes));
            }
        });
    }

    #[test]
    fn matches_the_check_values() {
        let check = b"123456789";
        #[cfg(feature = "crc16-modbus")]
        {
            assert_eq!(crc16_modbus(check), Crc16Modbus::CHECK);
            assert_eq!(crc16_modbus(b""), 0xFFFF);
        }
        #[cfg(feature = "crc32")]
        {
            assert_eq!(crc32(check), Crc32::CHECK);
            assert_eq!(crc32(b""), 0);
        }
        #[cfg(feature = "crc32c")]
        {
            assert_eq!(crc32c(check), Crc32c::CHECK);
            // RFC 3720's, for iSCSI.
            assert_eq!(crc32c(&[0; 32]), 0x8A91_36AA);
            assert_eq!(crc32c(&[0xFF; 32]), 0x62A8_AB43);
            let ascending: [u8; 32] = core::array::from_fn(|n| n as u8);
            assert_eq!(crc32c(&ascending), 0x46DD_794E);
            let descending: [u8; 32] = core::array::from_fn(|n| 31 - n as u8);
            assert_eq!(crc32c(&descending), 0x113F_DB5C);
        }
        #[cfg(feature = "crc32-bzip2")]
        assert_eq!(crc32_bzip2(check), Crc32Bzip2::CHECK);
        #[cfg(feature = "crc64-xz")]
        assert_eq!(crc64_xz(check), Crc64Xz::CHECK);
    }
}

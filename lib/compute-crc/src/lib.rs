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
//! Each computes over bytes fed in pieces, as they arrive: a value, `Copy`,
//! whose [`update`](Crc32::update) returns the next, so that a copy taken
//! anywhere goes on from there. [`state`](Crc32::state) and
//! [`from_state`](Crc32::from_state) expose the register, before the final
//! inversion, which formats that chain checksums store, as ext4 does.
//! A function of each, such as [`crc32`], computes it over one slice.
//!
//! They take eight bytes at a time, by eight tables of each byte followed
//! by none to seven zeros, built at compile time; a variant's tables are
//! in a build only if its feature is. The sources are each CRC's
//! parameters, its polynomial, its bit order, and its start and final
//! values, which are facts, as the catalogue of CRCs lists them; no
//! other implementation's code is copied. It is pure, safe Rust without
//! dependencies, and builds with `#![no_std]`, without an allocator. The
//! `std` feature, enabled by default, currently adds nothing.
//!
//! It is one of the `compute-*` crates: clean-room, public-domain
//! checksums and hashes, each of the same shape.
//!
//! # Examples
//!
//! ```
//! # #[cfg(feature = "crc32")] {
//! use compute_crc::{Crc32, crc32};
//!
//! assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
//! let crc = Crc32::new().update(b"1234").update(b"56789");
//! assert_eq!(crc.finish(), 0xCBF4_3926);
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

/// Defines a CRC whose bits are read from each byte's least significant
/// first, of the register type `$reg`, with the polynomial `$poly`, its
/// bits reversed, starting from all ones: its tables, its type, and its
/// function. `$finish` is the final value's inversion, if any.
macro_rules! reflected {
    (
        $(#[$meta:meta])*
        $name:ident, $function:ident, $feature:literal, $reg:ty, $poly:expr, $tables:ident,
        invert: $invert:literal, check: $check:expr, label: $label:literal
    ) => {
        /// The tables for taking eight bytes at a time: `TABLES[0][b]` is
        /// the CRC of byte `b`, and `TABLES[k][b]` that of `b` followed by
        /// `k` zero bytes. A `static`, as a `const` would be copied
        /// wherever it is used, unless optimized away.
        #[cfg(feature = $feature)]
        static $tables: [[$reg; 256]; 8] = {
            // Evaluated at compile time, where any overflow or index out
            // of range fails the build instead of panicking.
            #[allow(
                clippy::arithmetic_side_effects,
                clippy::as_conversions,
                clippy::indexing_slicing,
                reason = "evaluated at compile time"
            )]
            const fn build() -> [[$reg; 256]; 8] {
                let mut tables = [[0; 256]; 8];
                let mut byte = 0;
                while byte < 256 {
                    let mut crc = byte as $reg;
                    let mut bit = 0;
                    while bit < 8 {
                        crc = if crc & 1 == 0 { crc >> 1 } else { (crc >> 1) ^ $poly };
                        bit += 1;
                    }
                    tables[0][byte] = crc;
                    byte += 1;
                }
                let mut k = 1;
                while k < 8 {
                    let mut byte = 0;
                    while byte < 256 {
                        let previous = tables[k - 1][byte];
                        tables[k][byte] = (previous >> 8) ^ tables[0][(previous & 0xFF) as usize];
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

            /// Feeds `bytes`, and returns the CRC that goes on from there.
            #[must_use]
            pub fn update(self, bytes: &[u8]) -> Self {
                // Always found: `k` is below 8, and a byte below 256.
                let table = |k: usize, byte: u8| {
                    $tables
                        .get(k)
                        .and_then(|table| table.get(usize::from(byte)))
                        .copied()
                        .unwrap_or(0)
                };
                let mut crc = self.state;
                let (groups, rest) = bytes.as_chunks::<8>();
                for group in groups {
                    // The register's bytes go into the group's first, and
                    // each byte of the group out by its table.
                    let mut group = *group;
                    for (byte, reg) in group.iter_mut().zip(crc.to_le_bytes()) {
                        *byte ^= reg;
                    }
                    crc = 0;
                    for (k, &byte) in group.iter().enumerate() {
                        crc ^= table(7_usize.wrapping_sub(k), byte);
                    }
                }
                for &byte in rest {
                    let [low, ..] = crc.to_le_bytes();
                    crc = table(0, low ^ byte) ^ (crc >> 8);
                }
                Self { state: crc }
            }

            #[doc = concat!("Returns the ", $label, " of the bytes fed.")]
            #[must_use]
            pub const fn finish(self) -> $reg {
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
            pub const fn state(self) -> $reg {
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
            $name::new().update(bytes).finish()
        }
    };
}

reflected! {
    /// CRC-16/MODBUS: the polynomial 0x8005, each byte read from its least
    /// significant bit, from `0xFFFF`, and not inverted at the end; which
    /// ext4 stamps on its group descriptors without `metadata_csum`.
    ///
    /// ```
    /// use compute_crc::{Crc16Modbus, crc16_modbus};
    ///
    /// assert_eq!(crc16_modbus(b"123456789"), 0x4B37);
    /// let crc = Crc16Modbus::new().update(b"1234").update(b"56789");
    /// assert_eq!(crc.finish(), Crc16Modbus::CHECK);
    /// ```
    Crc16Modbus, crc16_modbus, "crc16-modbus", u16, 0xA001, CRC16_MODBUS,
    invert: false, check: 0x4B37, label: "CRC-16/MODBUS"
}

reflected! {
    /// CRC-32 (CRC-32/ISO-HDLC): the polynomial 0x04C11DB7, each byte read
    /// from its least significant bit, from all ones, and inverted at the
    /// end; which gzip, PNG, ZIP, xz, and GPT stamp.
    ///
    /// ```
    /// use compute_crc::{Crc32, crc32};
    ///
    /// assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    /// let crc = Crc32::new().update(b"1234").update(b"56789");
    /// assert_eq!(crc.finish(), Crc32::CHECK);
    /// ```
    Crc32, crc32, "crc32", u32, 0xEDB8_8320, CRC32,
    invert: true, check: 0xCBF4_3926, label: "CRC-32"
}

reflected! {
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
    /// let seed = Crc32c::new().update(b"1234").state();
    /// let crc = Crc32c::from_state(seed).update(b"56789");
    /// assert_eq!(crc.finish(), Crc32c::CHECK);
    /// assert_eq!(crc.state(), !0xE306_9283);
    /// ```
    Crc32c, crc32c, "crc32c", u32, 0x82F6_3B78, CRC32C,
    invert: true, check: 0xE306_9283, label: "CRC-32C"
}

reflected! {
    /// CRC-64/XZ: the polynomial 0x42F0E1EBA9EA3693 of ECMA-182, each byte
    /// read from its least significant bit, from all ones, and inverted at
    /// the end; which xz stamps on blocks.
    ///
    /// ```
    /// use compute_crc::{Crc64Xz, crc64_xz};
    ///
    /// assert_eq!(crc64_xz(b"123456789"), 0x995D_C9BB_DF19_39FA);
    /// let crc = Crc64Xz::new().update(b"1234").update(b"56789");
    /// assert_eq!(crc.finish(), Crc64Xz::CHECK);
    /// ```
    Crc64Xz, crc64_xz, "crc64-xz", u64, 0xC96C_5795_D787_0F42, CRC64_XZ,
    invert: true, check: 0x995D_C9BB_DF19_39FA, label: "CRC-64/XZ"
}

/// The tables of CRC-32/BZIP2, as `reflected!`'s, for bits read from each
/// byte's most significant first: `TABLES[k][b]` is the CRC of byte `b`,
/// in the register's top byte, followed by `k` zero bytes.
#[cfg(feature = "crc32-bzip2")]
static CRC32_BZIP2: [[u32; 256]; 8] = {
    #[allow(
        clippy::arithmetic_side_effects,
        clippy::as_conversions,
        clippy::indexing_slicing,
        reason = "evaluated at compile time"
    )]
    const fn build() -> [[u32; 256]; 8] {
        let mut tables = [[0; 256]; 8];
        let mut byte = 0;
        while byte < 256 {
            let mut crc = (byte as u32) << 24;
            let mut bit = 0;
            while bit < 8 {
                crc = if crc & 0x8000_0000 == 0 {
                    crc << 1
                } else {
                    (crc << 1) ^ 0x04C1_1DB7
                };
                bit += 1;
            }
            tables[0][byte] = crc;
            byte += 1;
        }
        let mut k = 1;
        while k < 8 {
            let mut byte = 0;
            while byte < 256 {
                let previous = tables[k - 1][byte];
                tables[k][byte] = (previous << 8) ^ tables[0][(previous >> 24) as usize];
                byte += 1;
            }
            k += 1;
        }
        tables
    }
    build()
};

/// CRC-32/BZIP2: CRC-32's polynomial, 0x04C11DB7, but each byte read from
/// its most significant bit; from all ones, and inverted at the end;
/// which bzip2 stamps on blocks.
///
/// ```
/// use compute_crc::{Crc32Bzip2, crc32_bzip2};
///
/// assert_eq!(crc32_bzip2(b"123456789"), 0xFC89_1918);
/// let crc = Crc32Bzip2::new().update(b"1234").update(b"56789");
/// assert_eq!(crc.finish(), Crc32Bzip2::CHECK);
/// ```
#[cfg(feature = "crc32-bzip2")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Crc32Bzip2 {
    /// The register, which starts as all ones.
    state: u32,
}

#[cfg(feature = "crc32-bzip2")]
impl Crc32Bzip2 {
    /// Starts a CRC-32/BZIP2 over no bytes.
    #[must_use]
    pub const fn new() -> Self {
        Self { state: !0 }
    }

    /// Feeds `bytes`, and returns the CRC that goes on from there.
    #[must_use]
    pub fn update(self, bytes: &[u8]) -> Self {
        // Always found: `k` is below 8, and a byte below 256.
        let table = |k: usize, byte: u8| {
            CRC32_BZIP2
                .get(k)
                .and_then(|table| table.get(usize::from(byte)))
                .copied()
                .unwrap_or(0)
        };
        let mut crc = self.state;
        let (groups, rest) = bytes.as_chunks::<8>();
        for group in groups {
            // The register's bytes go into the group's first, the highest
            // first, and each byte of the group out by its table.
            let mut group = *group;
            for (byte, reg) in group.iter_mut().zip(crc.to_be_bytes()) {
                *byte ^= reg;
            }
            crc = 0;
            for (k, &byte) in group.iter().enumerate() {
                crc ^= table(7_usize.wrapping_sub(k), byte);
            }
        }
        for &byte in rest {
            let [top, ..] = crc.to_be_bytes();
            crc = table(0, top ^ byte) ^ (crc << 8);
        }
        Self { state: crc }
    }

    /// Returns the CRC-32/BZIP2 of the bytes fed.
    #[must_use]
    pub const fn finish(self) -> u32 {
        !self.state
    }

    /// Resumes a CRC-32/BZIP2 from `state`, a register as
    /// [`state`](Self::state) returns it.
    #[must_use]
    pub const fn from_state(state: u32) -> Self {
        Self { state }
    }

    /// Returns the register: the CRC of the bytes fed, before the final
    /// inversion.
    #[must_use]
    pub const fn state(self) -> u32 {
        self.state
    }

    /// The CRC of the nine bytes `123456789`, which identifies it.
    pub const CHECK: u32 = 0xFC89_1918;
}

#[cfg(feature = "crc32-bzip2")]
impl Default for Crc32Bzip2 {
    fn default() -> Self {
        Self::new()
    }
}

/// Returns the CRC-32/BZIP2 of `bytes`.
#[cfg(feature = "crc32-bzip2")]
#[must_use]
pub fn crc32_bzip2(bytes: &[u8]) -> u32 {
    Crc32Bzip2::new().update(bytes).finish()
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

    #[test]
    fn matches_the_bitwise_definitions() {
        each(|bytes, piece| {
            #[cfg(feature = "crc16-modbus")]
            {
                let crc = pieces(Crc16Modbus::new(), bytes, piece, Crc16Modbus::update);
                let poly = 0x8005;
                assert_eq!(
                    u64::from(crc.finish()),
                    bitwise(16, poly, true, false, bytes)
                );
            }
            #[cfg(feature = "crc32")]
            {
                let crc = pieces(Crc32::new(), bytes, piece, Crc32::update);
                let poly = 0x04C1_1DB7;
                assert_eq!(
                    u64::from(crc.finish()),
                    bitwise(32, poly, true, true, bytes)
                );
            }
            #[cfg(feature = "crc32c")]
            {
                let crc = pieces(Crc32c::new(), bytes, piece, Crc32c::update);
                let poly = 0x1EDC_6F41;
                assert_eq!(
                    u64::from(crc.finish()),
                    bitwise(32, poly, true, true, bytes)
                );
            }
            #[cfg(feature = "crc32-bzip2")]
            {
                let crc = pieces(Crc32Bzip2::new(), bytes, piece, Crc32Bzip2::update);
                let poly = 0x04C1_1DB7;
                assert_eq!(
                    u64::from(crc.finish()),
                    bitwise(32, poly, false, true, bytes)
                );
            }
            #[cfg(feature = "crc64-xz")]
            {
                let crc = pieces(Crc64Xz::new(), bytes, piece, Crc64Xz::update);
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

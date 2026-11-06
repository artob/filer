// This is free and unencumbered software released into the public domain.

//! Every checksum and hash the harness measures, ours from the `compute-*`
//! crates and the counterparts their `BENCH.md` names, driven through one
//! interface: bytes fed whole, by one update, or in pieces of a size.
//! Counterparts are run, never read.

/// What an implementation computes, as bytes to compare with ours: a
/// checksum's value little-endian, or a digest, then zeros.
pub type Digest = [u8; 64];

/// Feeds `bytes` to an implementation, in pieces of at most the size it
/// is given, or whole for `usize::MAX`, and returns what it computed.
pub type Compute = fn(&[u8], usize) -> Digest;

/// An implementation, ours or a counterpart.
pub struct Implementation {
    /// Its name in tables.
    pub name: &'static str,
    /// The crate it is, as `Cargo.toml` names it, and its language.
    pub krate: &'static str,
    pub language: &'static str,
    pub compute: Compute,
}

/// An algorithm, and its implementations, ours first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Algorithm {
    Crc16Modbus,
    Crc32,
    Crc32c,
    Crc32Bzip2,
    Crc64Xz,
    Adler32,
    Fletcher2,
    Fletcher4,
    Fletcher64,
    Sha256,
    Sha512,
    Sha512_256,
    Xxh32,
    Xxh64,
}

impl Algorithm {
    pub const ALL: [Self; 14] = [
        Self::Crc16Modbus,
        Self::Crc32,
        Self::Crc32c,
        Self::Crc32Bzip2,
        Self::Crc64Xz,
        Self::Adler32,
        Self::Fletcher2,
        Self::Fletcher4,
        Self::Fletcher64,
        Self::Sha256,
        Self::Sha512,
        Self::Sha512_256,
        Self::Xxh32,
        Self::Xxh64,
    ];

    /// Its name on the command line and in the results.
    pub fn name(self) -> &'static str {
        match self {
            Self::Crc16Modbus => "crc16-modbus",
            Self::Crc32 => "crc32",
            Self::Crc32c => "crc32c",
            Self::Crc32Bzip2 => "crc32-bzip2",
            Self::Crc64Xz => "crc64-xz",
            Self::Adler32 => "adler32",
            Self::Fletcher2 => "fletcher2",
            Self::Fletcher4 => "fletcher4",
            Self::Fletcher64 => "fletcher64",
            Self::Sha256 => "sha256",
            Self::Sha512 => "sha512",
            Self::Sha512_256 => "sha512-256",
            Self::Xxh32 => "xxh32",
            Self::Xxh64 => "xxh64",
        }
    }

    /// The crate of ours that computes it.
    pub fn krate(self) -> &'static str {
        match self {
            Self::Crc16Modbus | Self::Crc32 | Self::Crc32c | Self::Crc32Bzip2 | Self::Crc64Xz => {
                "compute-crc"
            },
            Self::Adler32 => "compute-adler32",
            Self::Fletcher2 | Self::Fletcher4 | Self::Fletcher64 => "compute-fletcher",
            Self::Sha256 => "compute-sha256",
            Self::Sha512 | Self::Sha512_256 => "compute-sha512",
            Self::Xxh32 | Self::Xxh64 => "compute-xxhash",
        }
    }

    pub fn implementations(self) -> &'static [Implementation] {
        match self {
            Self::Crc16Modbus => &CRC16_MODBUS,
            Self::Crc32 => &CRC32,
            Self::Crc32c => &CRC32C,
            Self::Crc32Bzip2 => &CRC32_BZIP2,
            Self::Crc64Xz => &CRC64_XZ,
            Self::Adler32 => &ADLER32,
            Self::Fletcher2 => &FLETCHER2,
            Self::Fletcher4 => &FLETCHER4,
            Self::Fletcher64 => &FLETCHER64,
            Self::Sha256 => &SHA256,
            Self::Sha512 => &SHA512,
            Self::Sha512_256 => &SHA512_256,
            Self::Xxh32 => &XXH32,
            Self::Xxh64 => &XXH64,
        }
    }
}

/// Feeds `bytes` to `update` whole, or in pieces of `piece` bytes.
fn feed(bytes: &[u8], piece: usize, mut update: impl FnMut(&[u8])) {
    match piece {
        usize::MAX => update(bytes),
        piece => bytes.chunks(piece).for_each(update),
    }
}

/// A checksum's value as a digest.
fn value(value: u64) -> Digest {
    let mut digest = [0; 64];
    digest[..8].copy_from_slice(&value.to_le_bytes());
    digest
}

/// A digest's bytes as a digest.
fn bytes(bytes: &[u8]) -> Digest {
    let mut digest = [0; 64];
    digest[..bytes.len()].copy_from_slice(bytes);
    digest
}

/// Fletcher's four sums as a digest.
fn sums(sums: [u64; 4]) -> Digest {
    bytes(&sums.map(u64::to_le_bytes).concat())
}

/// An implementation of the name, crate, and language given, which runs
/// `$body` with `$bytes` and `$piece` bound.
macro_rules! implementation {
    ($name:literal, $krate:literal, $language:literal, |$b:ident, $p:ident| $body:expr) => {
        Implementation {
            name: $name,
            krate: $krate,
            language: $language,
            compute: {
                fn compute($b: &[u8], $p: usize) -> Digest {
                    $body
                }
                compute
            },
        }
    };
}

/// Ours, a type of a `compute-*` crate with `new`, `update`, and `finish`,
/// as a digest by `$digest`.
macro_rules! ours {
    ($krate:literal, $new:expr, $digest:expr) => {
        implementation!("ours", $krate, "Rust", |b, p| {
            let mut state = $new;
            feed(b, p, |piece| state.update(piece));
            $digest(state.finish())
        })
    };
}

/// The `crc` crate's CRC of the catalog's name given, in a register of
/// its width, by its fastest tables.
macro_rules! crc_crate {
    ($register:ty, $algorithm:ident) => {
        implementation!("crc", "crc", "Rust, portable", |b, p| {
            const CRC: crc::Crc<$register, crc::Table<16>> =
                crc::Crc::<$register, crc::Table<16>>::new(&crc::$algorithm);
            let mut digest = CRC.digest();
            feed(b, p, |piece| digest.update(piece));
            value(digest.finalize().into())
        })
    };
}

/// `crc-fast`'s CRC of the algorithm given.
macro_rules! crc_fast {
    ($algorithm:ident) => {
        implementation!("crc-fast", "crc-fast", "Rust, with SIMD", |b, p| {
            let mut digest = crc_fast::Digest::new(crc_fast::CrcAlgorithm::$algorithm);
            feed(b, p, |piece| digest.update(piece));
            value(digest.finalize())
        })
    };
}

const CRC16_MODBUS: [Implementation; 3] = [
    ours!("compute-crc", compute_crc::Crc16Modbus::new(), |v: u16| {
        value(v.into())
    }),
    crc_fast!(Crc16Modbus),
    crc_crate!(u16, CRC_16_MODBUS),
];

const CRC32: [Implementation; 5] = [
    ours!("compute-crc", compute_crc::Crc32::new(), |v: u32| value(
        v.into()
    )),
    implementation!("zlib", "libz-sys", "C", |b, p| {
        let mut crc = 0;
        feed(b, p, |piece| {
            // SAFETY: zlib reads `piece` within its bounds.
            crc = unsafe { libz_sys::crc32(crc, piece.as_ptr(), piece.len() as u32) };
        });
        value(crc)
    }),
    implementation!("crc32fast", "crc32fast", "Rust, with SIMD", |b, p| {
        let mut hasher = crc32fast::Hasher::new();
        feed(b, p, |piece| hasher.update(piece));
        value(hasher.finalize().into())
    }),
    crc_fast!(Crc32IsoHdlc),
    crc_crate!(u32, CRC_32_ISO_HDLC),
];

const CRC32C: [Implementation; 3] = [
    ours!("compute-crc", compute_crc::Crc32c::new(), |v: u32| value(
        v.into()
    )),
    crc_fast!(Crc32Iscsi),
    crc_crate!(u32, CRC_32_ISCSI),
];

const CRC32_BZIP2: [Implementation; 3] = [
    ours!("compute-crc", compute_crc::Crc32Bzip2::new(), |v: u32| {
        value(v.into())
    }),
    crc_fast!(Crc32Bzip2),
    crc_crate!(u32, CRC_32_BZIP2),
];

const CRC64_XZ: [Implementation; 4] = [
    ours!("compute-crc", compute_crc::Crc64Xz::new(), value),
    implementation!("crc64fast", "crc64fast", "Rust, with SIMD", |b, p| {
        let mut digest = crc64fast::Digest::new();
        feed(b, p, |piece| digest.write(piece));
        value(digest.sum64())
    }),
    crc_fast!(Crc64Xz),
    crc_crate!(u64, CRC_64_XZ),
];

const ADLER32: [Implementation; 4] = [
    ours!(
        "compute-adler32",
        compute_adler32::Adler32::new(),
        |v: u32| { value(v.into()) }
    ),
    implementation!("zlib", "libz-sys", "C", |b, p| {
        let mut adler = 1;
        feed(b, p, |piece| {
            // SAFETY: zlib reads `piece` within its bounds.
            adler = unsafe { libz_sys::adler32(adler, piece.as_ptr(), piece.len() as u32) };
        });
        value(adler)
    }),
    implementation!("adler2", "adler2", "Rust", |b, p| {
        let mut adler = adler2::Adler32::new();
        feed(b, p, |piece| adler.write_slice(piece));
        value(adler.checksum().into())
    }),
    implementation!("simd-adler32", "simd-adler32", "Rust, with SIMD", |b, p| {
        let mut adler = simd_adler32::Adler32::new();
        feed(b, p, |piece| adler.write(piece));
        value(adler.finish().into())
    }),
];

const FLETCHER2: [Implementation; 1] = [ours!(
    "compute-fletcher",
    compute_fletcher::Fletcher2::new(),
    sums
)];

const FLETCHER4: [Implementation; 1] = [ours!(
    "compute-fletcher",
    compute_fletcher::Fletcher4::new(),
    sums
)];

const FLETCHER64: [Implementation; 1] = [ours!(
    "compute-fletcher",
    compute_fletcher::Fletcher64::new(),
    value
)];

/// RustCrypto's `sha2` of the type given.
macro_rules! sha2 {
    ($hash:ident) => {
        implementation!("sha2", "sha2", "Rust, with SHA instructions", |b, p| {
            use sha2::Digest as _;
            let mut hasher = sha2::$hash::new();
            feed(b, p, |piece| hasher.update(piece));
            bytes(&hasher.finalize())
        })
    };
}

/// `ring`'s digest of the algorithm given.
macro_rules! ring {
    ($algorithm:ident) => {
        implementation!("ring", "ring", "C and assembly", |b, p| {
            let mut context = ring::digest::Context::new(&ring::digest::$algorithm);
            feed(b, p, |piece| context.update(piece));
            bytes(context.finish().as_ref())
        })
    };
}

const SHA256: [Implementation; 3] = [
    ours!(
        "compute-sha256",
        compute_sha256::Sha256::new(),
        |d: [u8; 32]| { bytes(&d) }
    ),
    sha2!(Sha256),
    ring!(SHA256),
];

const SHA512: [Implementation; 3] = [
    ours!(
        "compute-sha512",
        compute_sha512::Sha512::new(),
        |d: [u8; 64]| { bytes(&d) }
    ),
    sha2!(Sha512),
    ring!(SHA512),
];

const SHA512_256: [Implementation; 3] = [
    ours!(
        "compute-sha512",
        compute_sha512::Sha512_256::new(),
        |d: [u8; 32]| bytes(&d)
    ),
    sha2!(Sha512_256),
    ring!(SHA512_256),
];

const XXH32: [Implementation; 4] = [
    ours!("compute-xxhash", compute_xxhash::Xxh32::new(0), |v: u32| {
        value(v.into())
    }),
    implementation!("xxHash", "xxhash-c-sys", "C", |b, p| {
        use xxhash_c_sys::{XXH32_digest, XXH32_reset, XXH32_state_t, XXH32_update};
        // SAFETY: the state is plain integers, which `XXH32_reset` sets
        // before use, and xxHash reads each piece within its bounds.
        unsafe {
            let mut state: XXH32_state_t = std::mem::zeroed();
            XXH32_reset(&mut state, 0);
            feed(b, p, |piece| {
                XXH32_update(&mut state, piece.as_ptr().cast(), piece.len());
            });
            value(XXH32_digest(&state).into())
        }
    }),
    implementation!("xxhash-rust", "xxhash-rust", "Rust", |b, p| {
        let mut hasher = xxhash_rust::xxh32::Xxh32::new(0);
        feed(b, p, |piece| hasher.update(piece));
        value(hasher.digest().into())
    }),
    implementation!("twox-hash", "twox-hash", "Rust", |b, p| {
        use std::hash::Hasher as _;
        let mut hasher = twox_hash::XxHash32::with_seed(0);
        feed(b, p, |piece| hasher.write(piece));
        value(hasher.finish_32().into())
    }),
];

const XXH64: [Implementation; 4] = [
    ours!("compute-xxhash", compute_xxhash::Xxh64::new(0), value),
    implementation!("xxHash", "xxhash-c-sys", "C", |b, p| {
        use xxhash_c_sys::{XXH64_digest, XXH64_reset, XXH64_state_t, XXH64_update};
        // SAFETY: as for XXH32.
        unsafe {
            let mut state: XXH64_state_t = std::mem::zeroed();
            XXH64_reset(&mut state, 0);
            feed(b, p, |piece| {
                XXH64_update(&mut state, piece.as_ptr().cast(), piece.len());
            });
            value(XXH64_digest(&state))
        }
    }),
    implementation!("xxhash-rust", "xxhash-rust", "Rust", |b, p| {
        let mut hasher = xxhash_rust::xxh64::Xxh64::new(0);
        feed(b, p, |piece| hasher.update(piece));
        value(hasher.digest())
    }),
    implementation!("twox-hash", "twox-hash", "Rust", |b, p| {
        use std::hash::Hasher as _;
        let mut hasher = twox_hash::XxHash64::with_seed(0);
        feed(b, p, |piece| hasher.write(piece));
        value(hasher.finish())
    }),
];

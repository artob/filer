// This is free and unencumbered software released into the public domain.

//! The minimal `#![no_std]` program of the harness's `size` command: a
//! function, `probe`, that decodes a stream with the decoder, or computes
//! a buffer's checksum or hash by the algorithm, its feature enables, from
//! construction to `finish`, every result kept by `black_box`, so that the
//! code that one call needs stays; and the panic handler and allocator any
//! such program has, which are never called. Built alone, it is the
//! baseline from which `size` measures what each adds.

#![no_std]

use core::{alloc::Layout, hint::black_box};

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

/// An allocator that allocates nothing, as `probe` is never run.
struct Nothing;

// SAFETY: it never allocates, which `GlobalAlloc` allows.
unsafe impl core::alloc::GlobalAlloc for Nothing {
    unsafe fn alloc(&self, _: Layout) -> *mut u8 {
        core::ptr::null_mut()
    }

    unsafe fn dealloc(&self, _: *mut u8, _: Layout) {}
}

#[global_allocator]
static NOTHING: Nothing = Nothing;

/// Decodes `input` into `output` with the decoder given, then finishes.
#[allow(unused_macros)]
macro_rules! decode {
    ($input:ident, $output:ident, $decoder:expr) => {{
        let mut decoder = $decoder;
        black_box(decoder.decode($input, $output).is_ok());
        black_box(decoder.finish().is_ok());
    }};
}

/// Computes the checksum or hash of `input` with the value given.
#[allow(unused_macros)]
macro_rules! compute {
    ($input:ident, $value:expr) => {{
        let mut value = $value;
        value.update($input);
        black_box(value.finish());
    }};
}

/// Decodes `input`, or computes over it, with what the feature enables.
#[unsafe(no_mangle)]
#[allow(unused_variables)]
pub extern "C" fn probe(input: &[u8; 4096], output: &mut [u8; 4096]) {
    let input: &[u8] = black_box(input);
    let output: &mut [u8] = black_box(output);
    #[cfg(feature = "decode-bzip2")]
    decode!(input, output, decode_bzip2::Decoder::new());
    // Deflate or Deflate64.
    #[cfg(feature = "decode-deflate")]
    decode!(
        input,
        output,
        match black_box(true) {
            true => decode_deflate::Decoder::new(),
            false => decode_deflate::Decoder::deflate64(),
        }
    );
    #[cfg(feature = "decode-gzip")]
    decode!(input, output, decode_gzip::Decoder::new());
    #[cfg(feature = "decode-lz4")]
    decode!(input, output, decode_lz4::Decoder::new());
    // LZMA or LZMA2.
    #[cfg(feature = "decode-lzma")]
    if let Ok(decoder) = match black_box(true) {
        true => decode_lzma::Decoder::lzma(black_box([0; 5]), black_box(None)),
        false => decode_lzma::Decoder::lzma2(black_box(0)),
    } {
        decode!(input, output, decoder);
    }
    #[cfg(feature = "decode-lzo")]
    decode!(input, output, decode_lzo::Decoder::new());
    #[cfg(feature = "decode-xz")]
    decode!(input, output, decode_xz::Decoder::new());
    #[cfg(feature = "decode-zlib")]
    decode!(input, output, decode_zlib::Decoder::new());
    #[cfg(feature = "decode-zstd")]
    decode!(input, output, decode_zstd::Decoder::new());
    #[cfg(feature = "crc16-modbus")]
    compute!(input, compute_crc::Crc16Modbus::new());
    #[cfg(feature = "crc32")]
    compute!(input, compute_crc::Crc32::new());
    #[cfg(feature = "crc32c")]
    compute!(input, compute_crc::Crc32c::new());
    #[cfg(feature = "crc32-bzip2")]
    compute!(input, compute_crc::Crc32Bzip2::new());
    #[cfg(feature = "crc64-xz")]
    compute!(input, compute_crc::Crc64Xz::new());
    #[cfg(feature = "adler32")]
    compute!(input, compute_adler32::Adler32::new());
    #[cfg(feature = "fletcher2")]
    compute!(input, compute_fletcher::Fletcher2::new());
    #[cfg(feature = "fletcher4")]
    compute!(input, compute_fletcher::Fletcher4::new());
    #[cfg(feature = "fletcher64")]
    compute!(input, compute_fletcher::Fletcher64::new());
    #[cfg(feature = "sha256")]
    compute!(input, compute_sha256::Sha256::new());
    #[cfg(feature = "sha512")]
    compute!(input, compute_sha512::Sha512::new());
    #[cfg(feature = "sha512-256")]
    compute!(input, compute_sha512::Sha512_256::new());
    #[cfg(feature = "xxh32")]
    compute!(input, compute_xxhash::Xxh32::new(black_box(0)));
    #[cfg(feature = "xxh64")]
    compute!(input, compute_xxhash::Xxh64::new(black_box(0)));
}

# decode-xz

An xz decoder, written clean room from the `.xz` file format specification,
its branch converters ported from the LZMA SDK, both public domain, and its
SHA-256 from FIPS 180-4; dedicated to the public domain. It decodes `.xz`
files, as compressed tarballs, Debian packages, and initramfs images hold
them: streams of blocks of LZMA2, through Delta or a branch converter for
x86, PowerPC, IA-64, ARM, ARM Thumb, SPARC, ARM64, or RISC-V code, each
checked against its CRC-32, CRC-64, or SHA-256, and each stream against its
index. It never compresses. It is pure, safe Rust, builds with `#![no_std]`
and an allocator, never panics on any input, and decodes LZMA2 with
`decode-lzma`, ported likewise.

Its `Decoder` is sans-I/O: `decode` takes what input there is, fills what
room there is, and says how much of each it used, so it serves blocking,
async, and in-memory callers alike. `checkpoint` offers a copy of the
decoder between blocks, which decode alone, for resuming a long file at a
block. It is one of the `decode-*` crates: clean-room, public-domain decoders
of common codecs, each of the same shape.

```rust
use decode_xz::{Decoder, Error};

/// Decodes a whole `.xz` file in memory.
fn decompress(input: &[u8]) -> Result<Vec<u8>, Error> {
    let mut decoder = Decoder::new();
    let (mut output, mut buf, mut at) = (Vec::new(), [0; 4096], 0);
    loop {
        let (consumed, produced) = decoder.decode(&input[at..], &mut buf)?;
        at += consumed;
        output.extend_from_slice(&buf[..produced]);
        if (consumed, produced) == (0, 0) {
            break;
        }
    }
    // Checks that the file ended where a file may.
    decoder.finish()?;
    Ok(output)
}

fn main() -> Result<(), Error> {
    // `printf hello | xz -0 -C crc32`.
    let file = [
        0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00, 0x00, 0x01, 0x69, 0x22, 0xDE, 0x36,
        0x03, 0xC0, 0x09, 0x05, 0x21, 0x01, 0x0C, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x6D, 0xFF, 0x09, 0xFF, 0x01, 0x00, 0x04, 0x68, 0x65, 0x6C, 0x6C, 0x6F,
        0x00, 0x00, 0x00, 0x00, 0x86, 0xA6, 0x10, 0x36, 0x00, 0x01, 0x1D, 0x05,
        0xB8, 0x2D, 0x80, 0xAF, 0x90, 0x42, 0x99, 0x0D, 0x01, 0x00, 0x00, 0x00,
        0x00, 0x01, 0x59, 0x5A,
    ];
    assert_eq!(decompress(&file)?, b"hello");
    Ok(())
}
```

This is free and unencumbered software released into the public domain.

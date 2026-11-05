# decode-lz4

An LZ4 decoder, written clean room from LZ4's block and frame format
descriptions and dedicated to the public domain. It decodes LZ4 frames,
checking their checksums and sizes, legacy frames, as `lz4 -l` writes them
for the Linux kernel's initramfs images, and raw blocks, as SquashFS,
EROFS, ZFS, F2FS, and ReFS store them; it never compresses. It is pure,
safe Rust, its XXH32 `compute-xxhash`'s, builds with `#![no_std]` and an
allocator, never panics on any input, and bounds its memory by a block.

Its `Decoder` is sans-I/O: `decode` takes what input there is, fills what
room there is, and says how much of each it used, so it serves blocking,
async, and in-memory callers alike. `checkpoint` offers a copy of the
decoder anywhere, for resuming a long stream part way, and `decode_block`
decodes a raw block whole. It is one of the `decode-*` crates: clean-room,
public-domain decoders of common codecs, each of the same shape.

```rust
use decode_lz4::{Decoder, Error};

/// Decodes a whole LZ4 stream in memory.
fn decompress(mut input: &[u8]) -> Result<Vec<u8>, Error> {
    let mut decoder = Decoder::new();
    let (mut output, mut buf) = (Vec::new(), [0; 4096]);
    loop {
        // An empty `input` says the stream has ended.
        let (consumed, produced) = decoder.decode(input, &mut buf)?;
        input = &input[consumed..];
        output.extend_from_slice(&buf[..produced]);
        if (consumed, produced) == (0, 0) {
            break;
        }
    }
    // Checks that the stream ended where a stream may.
    decoder.finish()?;
    Ok(output)
}

fn main() -> Result<(), Error> {
    // `printf hello | lz4`
    let packed = [
        0x04, 0x22, 0x4D, 0x18, 0x64, 0x40, 0xA7, 0x05, 0x00, 0x00, 0x80,
        0x68, 0x65, 0x6C, 0x6C, 0x6F, 0x00, 0x00, 0x00, 0x00, 0xF9, 0x77,
        0x00, 0xFB,
    ];
    assert_eq!(decompress(&packed)?, b"hello");
    Ok(())
}
```

This is free and unencumbered software released into the public domain.

# decode-lzo

An LZO decoder, written clean room from a description of the LZO1X stream
format and dedicated to the public domain. It decodes lzop files, as `lzop`
writes them and the Linux kernel reads them from initramfs images, checking
their checksums, and raw LZO1X blocks, as btrfs, SquashFS, and F2FS store
them; it never compresses. It is pure, safe Rust, its Adler-32
`compute-adler32`'s and its CRC-32 `compute-crc`'s, builds with
`#![no_std]` and an allocator, never panics on any input, and bounds its
memory by a block.

Its `Decoder` is sans-I/O: `decode` takes what input there is, fills what
room there is, and says how much of each it used, so it serves blocking,
async, and in-memory callers alike. `checkpoint` offers a copy of the
decoder anywhere, for resuming a long stream part way, and `decode_block`
decodes a raw block whole. It is one of the `decode-*` crates: clean-room,
public-domain decoders of common codecs, each of the same shape.

```rust
use decode_lzo::{Error, decode_block};

fn main() -> Result<(), Error> {
    // `hello`, five bytes again from five back, and the end mark.
    let block = [22, b'h', b'e', b'l', b'l', b'o', 0x90, 0, 0x11, 0, 0];
    let mut output = [0; 32];
    let len = decode_block(&block, &mut output)?;
    assert_eq!(&output[..len], b"hellohello");
    Ok(())
}
```

This is free and unencumbered software released into the public domain.

# decode-zlib

A zlib decoder, written clean room from RFC 1950 alone and dedicated to the
public domain. It decodes zlib streams, as btrfs, SquashFS, ZFS, zisofs,
Apple's compressed files, and PNG images hold them, checking each one's
Adler-32, and leaves what follows unread. It never compresses. It is pure,
safe Rust, builds with `#![no_std]` and an allocator, never panics on any
input, and decodes Deflate with `decode-deflate`, and computes Adler-32
with `compute-adler32`, both written likewise.

Its `Decoder` is sans-I/O: `decode` takes what input there is, fills what
room there is, and says how much of each it used, so it serves blocking,
async, and in-memory callers alike. `checkpoint` offers a copy of the
decoder anywhere, for resuming a long stream part way. It is one of the
`decode-*` crates: clean-room, public-domain decoders of common codecs,
each of the same shape.

```rust
use decode_zlib::{Decoder, Error};

/// Decodes a whole zlib stream in memory.
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
    // Checks that the stream ended where a stream may.
    decoder.finish()?;
    Ok(output)
}

fn main() -> Result<(), Error> {
    // `hello`, as zlib's `compress` writes it.
    let packed = [
        0x78, 0x9C, 0xCB, 0x48, 0xCD, 0xC9, 0xC9, 0x07, 0x00, 0x06, 0x2C, 0x02,
        0x15,
    ];
    assert_eq!(decompress(&packed)?, b"hello");
    Ok(())
}
```

This is free and unencumbered software released into the public domain.

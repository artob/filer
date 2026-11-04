# decode-gzip

A gzip decoder, written clean room from RFC 1952 alone and dedicated to the
public domain. It decodes gzip streams, as `.gz` files, compressed
tarballs, and initramfs images hold them, checking each member's CRC-32
and length; it decodes members that follow one another as one stream, as
gzip does, and leaves what follows the last unread. It never compresses. It
is pure, safe Rust, builds with `#![no_std]` and an allocator, never panics
on any input, and decodes Deflate with `decode-deflate`, written likewise.

Its `Decoder` is sans-I/O: `decode` takes what input there is, fills what
room there is, and says how much of each it used, so it serves blocking,
async, and in-memory callers alike. `checkpoint` offers a copy of the
decoder anywhere, for resuming a long stream part way. It is one of the
`decode-*` crates: clean-room, public-domain decoders of common codecs,
each of the same shape.

```rust
use decode_gzip::{Decoder, Error};

/// Decodes a whole gzip stream in memory.
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
    // `printf hello | gzip -n`.
    let packed = [
        0x1F, 0x8B, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x03, 0xCB, 0x48,
        0xCD, 0xC9, 0xC9, 0x07, 0x00, 0x86, 0xA6, 0x10, 0x36, 0x05, 0x00, 0x00,
        0x00,
    ];
    assert_eq!(decompress(&packed)?, b"hello");
    Ok(())
}
```

This is free and unencumbered software released into the public domain.

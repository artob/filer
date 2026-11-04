# decode-bzip2

A bzip2 decoder, written clean room from a description of the format and
dedicated to the public domain. It decodes bzip2 streams, one after
another, checking each block against its CRC and the blocks against the
stream's, and ignores what follows the last stream, as bzip2 does; it
never compresses. It is pure, safe Rust, its CRC `compute-crc`'s, builds
with `#![no_std]` and an allocator, never panics on any input, and holds at
most a block, about 900 KiB packed, producing what it decodes to as it
is read.

Its `Decoder` is sans-I/O: `decode` takes what input there is, fills what
room there is, and says how much of each it used, so it serves blocking,
async, and in-memory callers alike. `checkpoint` offers a decoder that
goes on from between blocks, for resuming a long stream part way. It is
one of the `decode-*` crates: clean-room, public-domain decoders of
common codecs, each of the same shape.

```rust
use decode_bzip2::{Decoder, Error};

/// Decodes a whole bzip2 stream in memory.
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
    // `printf hello | bzip2`
    let packed = [
        0x42, 0x5A, 0x68, 0x39, 0x31, 0x41, 0x59, 0x26, 0x53, 0x59, 0x19,
        0x31, 0x65, 0x3D, 0x00, 0x00, 0x00, 0x81, 0x00, 0x02, 0x44, 0xA0,
        0x00, 0x21, 0x9A, 0x68, 0x33, 0x4D, 0x07, 0x33, 0x8B, 0xB9, 0x22,
        0x9C, 0x28, 0x48, 0x0C, 0x98, 0xB2, 0x9E, 0x80,
    ];
    assert_eq!(decompress(&packed)?, b"hello");
    Ok(())
}
```

This is free and unencumbered software released into the public domain.

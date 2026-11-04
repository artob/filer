# decode-zstd

A zstd decoder, written clean room from RFC 8878 alone and dedicated to
the public domain. It decodes zstd streams, frame after frame, checking
each against its content size and XXH64 checksum, and reads past
skippable frames; it never compresses. It is pure, safe Rust, its XXH64
`compute-xxhash`'s, builds with `#![no_std]` and an allocator, never panics on
any input, and bounds its memory by the frame's window, refusing windows
over 128 MiB as zstd itself does by default.

Its `Decoder` is sans-I/O: `decode` takes what input there is, fills what
room there is, and says how much of each it used, so it serves blocking,
async, and in-memory callers alike. `checkpoint` offers a decoder that
goes on from between frames, for resuming a long stream part way. It is
one of the `decode-*` crates: clean-room, public-domain decoders of
common codecs, each of the same shape.

```rust
use decode_zstd::{Decoder, Error};

/// Decodes a whole zstd stream in memory.
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
    // `printf hello | zstd`
    let packed = [
        0x28, 0xB5, 0x2F, 0xFD, 0x04, 0x58, 0x29, 0x00, 0x00, 0x68, 0x65,
        0x6C, 0x6C, 0x6F, 0xA3, 0x6D, 0x9F, 0x88,
    ];
    assert_eq!(decompress(&packed)?, b"hello");
    Ok(())
}
```

This is free and unencumbered software released into the public domain.

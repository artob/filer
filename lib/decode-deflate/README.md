# decode-deflate

A Deflate decoder, written clean room from RFC 1951 alone and dedicated to
the public domain. It decodes raw Deflate streams, as ZIP stores its
members and gzip and zlib frame them, and consumes a stream to its end and
no further, so that a container reads on from what follows; it never
compresses. It is pure, safe Rust without dependencies, builds with
`#![no_std]` and an allocator, never panics on any input, and refuses what
zlib refuses, holding at most its 32 KiB window and twice as much again.

Its `Decoder` is sans-I/O: `decode` takes what input there is, fills what
room there is, and says how much of each it used, so it serves blocking,
async, and in-memory callers alike. `checkpoint` offers a copy of the
decoder anywhere, for resuming a long stream part way. It is one of the
`decode-*` crates: clean-room, public-domain decoders of common codecs,
each of the same shape.

```rust
use decode_deflate::{Decoder, Error};

/// Decodes a whole Deflate stream in memory, and returns it and how much
/// of `input` it took, for what follows.
fn decompress(input: &[u8]) -> Result<(Vec<u8>, usize), Error> {
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
    Ok((output, at))
}

fn main() -> Result<(), Error> {
    // "hello", deflated as one block of fixed codes, then what follows.
    let packed = [0xCB, 0x48, 0xCD, 0xC9, 0xC9, 0x07, 0x00, 0xFF];
    assert_eq!(decompress(&packed)?, (b"hello".to_vec(), 7));
    Ok(())
}
```

`Decoder::deflate64` decodes Deflate64, PKWARE's variant for ZIP's method 9
and 7z, with a 64 KiB window, a longer last length, and two more distances,
behind the additive `deflate64` feature, enabled by default: a stream is one
or the other, as its container says.

This is free and unencumbered software released into the public domain.

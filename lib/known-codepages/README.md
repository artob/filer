# known-codepages

The single-byte code pages of DOS and Windows consoles, decoded by table:
437, the original IBM PC character set, and 720, 737, 775, 850, 852, 855,
857, 858, 860 to 866, 869, and 874. Formats of that era store text in the
code page of the system that wrote it, without saying which: FAT short
names, and ZIP entry names without the UTF-8 flag. Code page 437, their
nominal default, is always available; each other page is behind a feature
of its number, such as `cp850`, all of them enabled by `all`, a default
feature.

It is pure, safe Rust without dependencies, needs no allocator, builds
with `#![no_std]`, and never panics. Its tables are generated from
Python's codecs, and agree with GNU libiconv on every defined byte. It
was written for [Filer](https://github.com/artob/filer), whose FAT and
ZIP drivers decode names through it.

```rust
use known_codepages::CodePage;

/// Decodes `name` in `page`, or returns `None` if it holds a byte that the
/// page leaves undefined.
fn decode(page: CodePage, name: &[u8]) -> Option<String> {
    name.iter().map(|&byte| page.decode(byte)).collect()
}

fn main() {
    // A FAT short name, as DOS wrote it in code page 437.
    let name = decode(CodePage::Cp437, b"CAF\x90.TXT");
    assert_eq!(name.as_deref(), Some("CAFÉ.TXT"));
    // A ZIP entry name from a Russian DOS system, in code page 866, if
    // its feature is enabled.
    if let Some(page) = CodePage::from_number(866) {
        let name = decode(page, b"\x8F\xE0\xA8\xA2\xA5\xE2.TXT");
        assert_eq!(name.as_deref(), Some("Привет.TXT"));
    }
}
```

This is free and unencumbered software released into the public domain.

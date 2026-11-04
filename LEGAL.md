# Legal status of the on-disk formats implemented in this code base

This project reimplements several on-disk file-system and archive formats
from scratch and dedicates the result to the public domain (Unlicense /
CC0). This document records what is known about the intellectual-property
status of each format, which sources contributors may use, and which they
may not.

**This is not legal advice.** It is a research summary, US-centric, last
reviewed 2026-11-03. Patent expiry dates below should be re-verified on
Google Patents before relying on them. Software patents are considerably weaker
outside the United States.

A note for downstream users: a public-domain dedication carries **no patent
license**. CC0 §4(a) expressly excludes patents, and the Unlicense is silent on
them.

## Per-filesystem assessment

### FAT12 / FAT16 / FAT32 (including VFAT long names) — clear

Microsoft's long-filename patents (US 5,579,517; 5,758,352; 6,286,013 and
family) all expired between 2013 and 2015. FAT12/16 is also standardized as
ECMA-107. Microsoft's old `fatgen103` specification PDF carries a narrow
document license, so it may be read for facts but its text must not be copied.

Safe inputs: ECMA-107, `fatgen103` (facts only), the UEFI Forum's republished
FAT specification, black-box observation of real images.

### exFAT — encumbered until 2028, then clear

Microsoft published the exFAT specification in August 2019 and committed its
patents only through the Open Invention Network's Linux System Definition. That
is a defensive cross-license among OIN members for Linux packages; it is not a
public grant and does not reach a standalone public-domain library or any
non-Linux system.

Four exFAT patents are reported to remain in force: US 8,321,439 (to
2027-03-09), 9,122,695 (to 2027-05-23), 10,614,032 (to 2028-01-27) and
9,639,554 (to 2028-04-14). The first claim of 9,639,554 is reported to cover
the directory-entry model itself, so a spec-conforming reader cannot avoid it.

Policy: exFAT code may be developed, but **must not be published or released
before the last of these patents expires (verify, expected mid-2028).**

Safe inputs: Microsoft's published specification (facts only). Do not copy
the Linux `fs/exfat` driver (Samsung/Microsoft, GPL).

### NTFS — low residual risk, no official specification

Microsoft has never published the NTFS on-disk format. Everything public comes
from decades of reverse engineering: the Linux-NTFS project's *NTFS
Documentation* (Russon & Fledel; GFDL 1.3) and Brian Carrier's *File
System Forensic Analysis* are the primary references. NTFS dates to 1993
and the current on-disk version (3.1) to 2001, so core patents are long
expired. Later additions (Transactional NTFS from 2006, Windows 10-era
WOF "system compression") are more recent; this project targets NTFS 3.1
semantics plus reparse points and does not implement those later
features.

Microsoft joined OIN in 2018, and Paragon's GPL `ntfs3` driver was merged into
Linux 5.15 (2021) without legal objection. Microsoft has not asserted patents
against open NTFS implementations in over twenty years.

Safe inputs: the Linux-NTFS documentation and forensic literature (facts only);
Microsoft's [MS-FSCC] and [MS-FSA] documents for semantics (covered by the Open
Specification Promise). Do not copy `ntfs-3g` (Tuxera, GPL/commercial) or
`ntfs3` (Paragon, GPL).

### ReFS — avoid; read-only at most

ReFS is proprietary and entirely undocumented by Microsoft. It was introduced in
2012, so Microsoft's ReFS patents (block cloning, integrity streams,
mirror-accelerated parity and related features) run into the 2030s. The format
is also a moving target: the last public structural map covers version 3.4,
and later versions (through 3.14) changed page-reference sizes and root
structures.

All public knowledge comes from academic reverse engineering (Prade et al.,
ReFuzz) and forensic tools (forefst, libfsrefs). Decompiling `refs.sys` directly
raises Windows EULA concerns and must not be done for this project.

Policy: **no ReFS writer.** Any ReFS reader is best-effort, forensic-grade
only, and carries acknowledged patent exposure.

### XFS — clear

Designed 1993–94 and released under the GPL by SGI in 2000; any SGI patents
expired years ago. The format is fully documented in *XFS Algorithms & Data
Structures* (CC-BY-SA 3.0 or later: share-alike, so facts only, no
quoting). No known patent concerns.

Safe inputs: the XFS documentation (facts). Do not copy `xfsprogs` or the
kernel driver (GPL).

### ext4 — clear

Thoroughly documented in the kernel's *ext4 Data Structures and Algorithms*
and by `e2fsprogs`. No known patents; extents, HTree and journaling are old and
rich in prior art.

Safe inputs: kernel documentation (facts). Do not copy `e2fsprogs` (GPL/LGPL)
or the kernel driver.

### Minix — clear

Andrew Tanenbaum designed the Minix file system in 1987 for MINIX 1.0,
modeled on Seventh Edition Unix's: a superblock, inode and zone bitmaps,
an inode table, and fixed-size directory entries. Version 2 (32-bit zone
numbers) existed by 1996, when Linux gained it, and version 3 (60-byte
names, larger blocks) came with MINIX 3 in 2005; Linux has read all three
since 2.6.21 (2007), and the 30-byte-name variants of versions 1 and 2
are Linux's own. No patent is known on any of it, and none from the
1980s could still be in force.

MINIX was proprietary, source-available software until April 2000, when
Prentice Hall let Tanenbaum relicense every version under a BSD license;
MINIX 3 carries a BSD-3-Clause variant with a fourth clause requiring
written permission for any deviation. So the MINIX 3 `mfs` server is a
permissively licensed reference, rare on this list, but copying from it
would still mean carrying its notice, which a public-domain file cannot.

Safe inputs: Tanenbaum and Woodhull's *Operating Systems: Design and
Implementation* (facts only: the prose and the code it prints are
copyrighted), the MINIX 3, Linux and `util-linux` headers for facts (magic
numbers, layouts), and images made with `mkfs.minix`. Do not copy the
Linux `fs/minix` driver or `util-linux`'s `mkfs.minix` and `fsck.minix`
(GPL), or MINIX 3's `mfs` (BSD, notice required).

### UFS (BSD FFS) — clear

The Berkeley Fast File System shipped in 4.2BSD (1983) and was described
by McKusick, Joy, Leffler and Fabry in 1984; the UFS1 layout read today
is 4.4BSD's (1993). UFS2 (64-bit block pointers, 256-byte inodes,
extended attributes) shipped in FreeBSD 5.0 (2003) and is described in
McKusick's BSDCon 2003 paper. Soft updates (1994) are an in-memory
technique, and soft updates journaling (SU+J, 2010) adds only a journal
file, so neither changes what a reader must parse. No patent is known on
any of it. The one patent family on UFS, Sun's UFS logging (US 5,870,757
and its division 6,021,414, priority 1995-09-11), covered the Solaris
variant's journal, which a reader may ignore, and expired on 2015-09-11.

The BSD code is permissively licensed: the Regents dropped the
advertising clause in 1999, so FreeBSD's, NetBSD's and OpenBSD's `sys/ufs`
headers are BSD-3-Clause (the Regents, 1982–93). The Solaris variant
lives in illumos under the CDDL; Apple's, of NeXTSTEP descent, was
dropped in OS X 10.7 and is under the APSL; Linux's `fs/ufs` is GPL and
reads every variant its `ufstype` option names (`44bsd`, `ufs2`, `sun`,
`sunx86`, `hp`, `nextstep`, `openstep`, `old`). "UFS" also names JEDEC's
Universal Flash Storage, an unrelated interface standard; the collision
affects only how the crate describes itself.

Safe inputs: the 1984 and 2003 papers, *The Design and Implementation of
the FreeBSD Operating System*, Carrier's *File System Forensic Analysis*
(its UFS1 and UFS2 chapters), the Linux `ufs` admin guide, and the BSD
headers for facts (the magic numbers 0x011954 and 0x19540119, superblock
offsets, layouts). Do not copy any BSD `ufs` code (notice required),
illumos (CDDL), Darwin's `xnu` (APSL), Linux `fs/ufs` (GPL) or The Sleuth
Kit (IBM Public License / CPL).

### ZFS — medium risk, declining

Sun filed roughly 56 patents on ZFS, mostly between 2001 and 2005, so the bulk
expired between 2021 and 2026. The CDDL's patent grant protects users of the
*covered software* (OpenZFS itself); a clean-room reimplementation is not
covered software and receives nothing from that grant.

NetApp's WAFL patents that underlay its 2007 suit against Sun (US 5,819,292;
6,857,001; 7,174,352) have expired, and that litigation was dismissed in 2010.

Residual exposure: Oracle's post-2010 patents on Oracle ZFS (relevant only where
on-disk features overlap with OpenZFS), and newer OpenZFS features (native
encryption, RAIDZ expansion, fast dedup) contributed by companies that may hold
patents. Oracle has a history of IP litigation.

Safe inputs: Sun's 2006 *ZFS On-Disk Specification* draft, OpenZFS
documentation, conference talks. Do not copy any OpenZFS code (CDDL).

### Btrfs — low risk

GPL since 2007. No Btrfs-specific patents are known, and anything from the
early Oracle years would expire around 2027–28 regardless. The copy-on-write
B-tree design derives from Ohad Rodeh's 2006 IBM paper; any IBM patents on
that technique are expiring around now.

Safe inputs: the btrfs wiki / readthedocs format documentation. Do not copy
`btrfs-progs` or the kernel driver (GPL).

### bcachefs — low legal risk, high format churn

GPL-2.0, essentially a single-author project, no known patents. The risk is
engineering rather than legal: merged in Linux 6.7, marked "externally
maintained" in August 2025, removed from the tree in 6.18, now shipped as a
DKMS module, and still self-described as experimental as of mid-2026. The
on-disk format revs frequently; this implementation must track format version
numbers carefully.

Safe inputs: *bcachefs: Principles of Operation*. Do not copy `bcachefs-tools`
or the kernel module (GPL).

### APFS — documented, but patented

Apple published the *Apple File System Reference* in 2018 (revised 2020); it
explicitly contemplates implementations on other platforms. It grants no patent
license. APFS shipped in 2017, so Apple's patents (space sharing, snapshots,
encryption tiers) run to roughly 2036–37. Apple has tolerated third-party
implementations (libfsapfs, Paragon's read-only SDK, linux-apfs-rw) for years.

The reference remains incomplete around Fusion drives, T2/sealed volumes and
some compression methods.

Policy: read-only support only. A writer is both legally and practically
riskier and is out of scope.

Safe inputs: Apple's reference (facts and identifiers only; do not copy prose).
Do not copy libfsapfs (LGPLv3) or linux-apfs-rw (GPL).

### UDF — clear

ECMA-167 is free; OSTA's UDF 2.60 specification is freely downloadable. Both
date to the 1990s and any patents have expired. "UDF" is an OSTA trademark,
which affects only how the project describes itself.

### ISO 9660 — clear

Use ECMA-119 (free, textually identical) rather than the paid ISO text.
Rock Ridge / SUSP (IEEE P1281/P1282 drafts), Joliet (Microsoft, 1995) and
El Torito are all freely published and unpatented.

### SquashFS — clear

No patents. The format exists only as GPL code plus unofficial documentation
(the "SquashFS binary format" write-up on GitHub, kernel docs). Clean-room
readers already exist (squashfuse, BSD-3; squashfs-tools-ng, LGPL).

Compression back-ends: zlib (zlib license), LZ4 (BSD), zstd (BSD/GPL dual) and
xz/LZMA (public domain / 0BSD) are all acceptable to reimplement or study. LZO
is the exception: the reference implementation is GPL, so the decoder must be
written from the algorithm description or by studying lzokay (MIT), never from
minilzo.

## Per-archive assessment

The archive drivers already built (tar, ZIP, ar, cpio) are not assessed
here yet; the two reserved ones are.

### 7z — clear

7z is 7-Zip's native archive format (Igor Pavlov, 1999 onward). Its
description (`7zFormat.txt`), its method table (`Methods.txt`) and a C
decoder for it (`7zDec.c` and company) ship in the LZMA SDK, which Pavlov
placed in the public domain in 2008: the one reference implementation on
this list that could be copied outright, though this project ports rather
than copies (100% Rust). The dedication covers the LZMA, LZMA2 and xz
decoders, the BCJ, BCJ2, Delta and branch filters, PPMd var. H (Dmitry
Shkarin's own public-domain code) and SHA-256 (Wei Dai's); for
jurisdictions that do not recognize a dedication, the SDK's text also
grants everyone permission to copy, modify, sell and distribute it. 7-Zip
itself is LGPL-2.1-or-later, its RAR decoder under the unRAR restriction
and its zstd and LZFSE decoders BSD-3-Clause, and must not be copied. No
patent on LZMA is known; the xz project's FAQ says the same, with the
usual caveat about unknown third-party patents.

Other methods a 7z archive may use: Deflate (PKWARE's patent US 5,051,745
expired in 2008; RFC 1951), Deflate64 (a PKWARE trademark, undocumented
by PKWARE but reimplemented by 7-Zip and Info-ZIP; no patent known),
BZip2 (Burrows–Wheeler, unpatented; bzip2's BSD-style license), zstd
(BSD) and 7zAES (AES-256 with a SHA-256 key schedule, unencumbered, and
`Unsupported` for now anyway).

Safe inputs: the LZMA SDK's documents and code, RFC 1951, PKWARE's
APPNOTE for Deflate (facts), and bzip2's source (facts). Do not copy 7-Zip
or p7zip (LGPL).

### RAR — no patents known; decompression only, written clean room

RAR is Eugene Roshal's proprietary format (1993), with three compression
generations still in use: RAR 2 (1996), RAR 3 (2002: LZSS with Huffman
tables, a filter VM for executables and audio, and PPMd var. H for text)
and RAR5 (2013: a new container and LZSS, no PPMd), revised again in
RAR 7.0 (2024). Copyright in RAR and `unrar` is held by Alexander Roshal,
with win.rar GmbH as publisher; the names RAR and WinRAR are theirs, and
this project uses them only to name the format. No patent on the format
or its algorithms is known.

The encumbrance is copyright, not patent. The only complete description
of the decompression algorithms is the `unrar` source, whose license
allows its use in any software that handles RAR archives but forbids
re-creating the RAR compression algorithm or developing a RAR-compatible
archiver, and requires modified copies to carry that restriction. That
makes anything derived from it non-free (Debian and Fedora both say so)
and incompatible with a public-domain dedication. RARLAB's *RAR 5.0
archive format* technote documents the container (headers, records,
flags) and states no terms, but refers to `unrar` for the algorithms; an
older `technote.txt`, once shipped with WinRAR, did the same for RAR 3's
container.

Policy: a reader only, as for every archive, and one written without
`unrar`: the decoder is implemented from a written description of the
algorithms, never from the `unrar` source. Where no such description
exists, writing one from libarchive's readers (RAR 2/3 by Andres Mejia,
2011; RAR5 by Grzegorz Antoniak, 2018; both BSD-2-Clause and carrying no
`unrar` notice) and then coding from it is the procedure to record in
`DESIGN.md` before any code (TODO RAR-01). PPMd var. H is the
public-domain code in the LZMA SDK. AES-encrypted archives stay
`Unsupported`.

Safe inputs: RARLAB's technotes and libarchive (facts). Do not copy
`unrar` (its own license), 7-Zip's RAR decoder (the same restriction),
The Unarchiver or unarr (LGPL), or libarchive (BSD, notice required);
newer Rust decoders of unstated provenance are off limits too.

## Decoders

Archives compress with a handful of algorithms, and `filer-pack` decodes
each behind a feature of its own (`DESIGN.md` §17.7, D59). Each decoder
is pure Rust and builds with `#![no_std]` (D45), and is either a crate
under a permissive license, which stays its own (nothing of it is copied
into this code base), or written here from a description of its format.
Assessed 2026-11-03: re-check each crate's version, license and features
when adopting it.

The crates are interim (`DESIGN.md` D61): the aim is a clean-room,
public-domain decoder of each format, written here as a `decode-*` crate
of its own (D62), which replaces a crate once the crate becomes an
inconvenience or a blocker, rather than waiting on changes upstream. The
sources stay those assessed below: RFC 1951 and 1952 for Deflate and
gzip; RFC 8878 for zstd, and for its XXH64 checksum the xxHash
specification (Yann Collet, whose notice lets anyone copy and distribute
it), never zstd's or xxHash's own code; the LZMA SDK and the `.xz` file
format specification, both public domain, for LZMA, LZMA2, and xz; and
Joe Tsai's description for bzip2, as `Bzip2` already is. The checksums
and hashes, `compute-*` crates (D64), come likewise from their
definitions: each CRC's parameters, facts as the catalogue of CRCs lists
them, and RFC 3720's test vectors; FIPS 180-4 for SHA-256 and SHA-512, a
United States government work; RFC 7693 and the BLAKE2 specification for
BLAKE2, and the BLAKE3 specification, the reference code of neither
read; Fletcher's checksums as ZFS's and APFS's documentation define
their variants; the xxHash specification for XXH64; and RFC 1950 for
Adler-32. A crate's code is never read as the source of its replacement.

- **Deflate and gzip: written here.** `decode-deflate` (D62), written
  from RFC 1951 alone, decodes ZIP's raw Deflate, behind `deflate`, in
  place of `miniz_oxide` (MIT, Zlib or Apache-2.0) since 2026-11-04
  (DEC-02), which stays only to write tests' data. gzip wraps Deflate in
  a header and a trailer holding a CRC-32 and the length, which
  `decode-gzip` checks: written from RFC 1952, not from zlib or
  `flate2`; zlib's stream wraps it in two bytes and an Adler-32, which
  `decode-zlib` checks, written from RFC 1950 (DEC-06). `zlib-rs`, a port of zlib (Zlib license), is the reference
  of both crates' tests for zlib's behavior, run, never read. Deflate64,
  which PKWARE names but does not document, is decoded from how it is
  known to differ from Deflate (a 64 KiB window, length code 285, and
  distance codes 30 and 31), checked against 7-Zip's output and the
  `deflate64` crate (MIT), run as a black box; no patent on it is known.
  Deflate's patent (US 5,051,745) expired in 2008.
- **zstd: written here.** `decode-zstd`, which `filer-pack`'s `Zstd`
  adapts, is written from RFC 8878 alone, and its XXH64 checksum from
  the xxHash specification (D61, D62, 2026-11-04);
  zstd's and xxHash's own code (BSD-3-Clause or GPL-2.0, and
  BSD-2-Clause) is not copied. It replaced `ruzstd` 0.9 (MIT), which
  decoded zstd here before it, and which had `unsafe` code and internal
  panics, and a decoder state that could not be copied for checkpoints
  within frames. It was checked against `ruzstd` on zstd's own output
  and mutations of it until then, and is now checked by the frames'
  checksums and its fuzzers. zstd 1.0 came with an explicit patent
  grant, dropped in 1.3.1 (2017) when the license became BSD and GPLv2;
  no patent is known to be asserted against implementations, nor to
  cover XXH64.
- **LZMA, LZMA2, and xz: written here.** `decode-lzma` (D62) ports the
  LZMA SDK's specification, reference decoder, and LZMA2 decoder (public
  domain, see 7z), since 2026-11-04 (DEC-04); `decode-xz` is written from
  the `.xz` file format specification (public domain), its branch
  converters ported from the LZMA SDK, 9.20's and 26.05's, and its
  SHA-256 from FIPS 180-4, since 2026-11-04 (DEC-05). They replace
  `lzma-rust2` 0.25 (Apache-2.0; a port of XZ for Java), whose decoder
  and encoder are now only their tests' oracle and source of streams,
  run, never read. No patent is known.
- **bzip2: written here.** No crate fits. `bzip2` 0.6 is std only; its
  pure-Rust backend, `libbz2-rs-sys` 0.2 (bzip2-1.0.6 license), builds
  with `#![no_std]` but offers only libbzip2's C interface, whose raw
  pointers would take `unsafe` code that `filer-pack` forbids; and
  `bzip2-rs` 0.1 (MIT or Apache-2.0) is std only and unmaintained since
  2021. A decoder is small (Huffman tables, move-to-front, the inverse
  Burrows–Wheeler transform, and CRC-32s per block and stream):
  `decode-bzip2` (D62), which `filer-pack`'s `Bzip2` adapts, is written
  from Joe Tsai's *bzip2 format* (2016), an independent
  description reverse-engineered from bzip2 1.0.6, as no official one
  exists, with bzip2's own source (BSD-style, notice required) and Wuffs'
  decoder (Apache-2.0) read for facts only. No patent applies: the
  Burrows–Wheeler transform is unpatented, and bzip2 dropped bzip's
  arithmetic coding for Huffman coding over patent concerns.

## Summary

| Format    | Patents                          | Format documentation      | Verdict                  |
|-----------|----------------------------------|---------------------------|--------------------------|
| FAT       | expired                          | ECMA-107, fatgen103       | clear                    |
| exFAT     | live until ~Apr 2028             | official spec             | do not release until expiry |
| NTFS      | core expired                     | reverse-engineered only   | low risk                 |
| ReFS      | live into the 2030s              | none; reverse-engineered  | avoid; read-only at most |
| XFS       | expired                          | official                  | clear                    |
| ext4      | none known                       | official                  | clear                    |
| Minix     | none known                       | textbook; BSD source      | clear                    |
| UFS       | expired (Sun logging, 2015)      | papers; BSD headers       | clear                    |
| ZFS       | mostly expired, some residual    | 2006 spec + docs          | medium-low               |
| Btrfs     | none known                       | wiki                      | low                      |
| bcachefs  | none known                       | Principles of Operation   | low (format churn)       |
| APFS      | live until ~2036                 | official (partial)        | read-only only           |
| UDF       | expired                          | ECMA-167 / OSTA           | clear                    |
| ISO 9660  | none                             | ECMA-119                  | clear                    |
| SquashFS  | none                             | unofficial                | clear                    |
| 7z        | none known                       | public-domain SDK         | clear                    |
| RAR       | none known                       | container only; code      | reader only, clean room  |

## Rules for contributors: acceptable and unacceptable inputs

**Acceptable:**

- Standards-body texts (ECMA, IEEE, OSTA, UEFI Forum), read for facts.
- Vendor-published specifications (Microsoft exFAT, Apple APFS, Sun ZFS
  on-disk spec, Microsoft [MS-FSCC]/[MS-FSA]), read for facts and identifiers.
- Peer-reviewed forensic and systems papers.
- The Linux kernel's own documentation, `Documentation/filesystems/` in
  its tree and rendered at
  <https://www.kernel.org/doc/html/latest/filesystems/>: **allowed, as a
  factual source only.** It is part of the kernel and under the kernel's
  GPL-2.0 (its file-system documents carry that SPDX tag; the few without
  one fall under the tree's `COPYING`), so its prose, tables and examples
  may no more be copied or closely paraphrased than the drivers it
  describes, while the facts it states (layouts, field names, constants,
  semantics) are free to use. The same holds for the kernel's admin guide
  (`ufs`, `vfat`) and for tool documentation (`e2fsprogs`, `xfsprogs`,
  `util-linux`), under their own licenses. Why this keeps the clean room
  is explained below.
- Your own black-box observation of real images produced by the reference
  implementation.
- Field names, magic numbers, offsets, constants and struct layouts: these are
  facts, not copyrightable expression.

**Unacceptable:**

- Copying, translating or closely paraphrasing code from any existing driver or
  tool. Nearly every existing implementation of these formats is GPL, LGPL
  or CDDL, and copyleft contamination would void the public-domain
  dedication. The permissive exceptions (the BSD `ufs` code, MINIX 3's
  `mfs`, libarchive) still require their notices to travel with copied
  code, which a public-domain file cannot carry, so they too are sources
  of facts only. The LZMA SDK, itself public domain, is the one source
  whose code could be taken; this project ports it rather than copies it.
- Copying prose from specification documents into comments, docs or this file.
- Decompiling or disassembling proprietary drivers (`refs.sys`, `ntfs.sys`,
  `exfat.sys`, Apple's `apfs.kext`). Use the published reverse-engineering
  literature instead.
- Submitting code generated by a tool that has reproduced any of the above.

### Documentation and the clean room

The clean-room drivers (`DESIGN.md`, D52) are written from documentation
that is itself copyleft: the kernel's ext4, SquashFS and other documents
(GPL-2.0), the *XFS Algorithms & Data Structures* book (CC-BY-SA 3.0 or
later) and the Linux-NTFS *NTFS Documentation* (GFDL 1.3). Reading them
for facts is compatible with the clean room, for three reasons.

- A clean room separates the implementers from the *code's* expression,
  not from descriptions of the format. Its classic form is a written
  specification, prepared by people who saw the original and implemented
  by people who did not. These documents are that specification, written
  by third parties: reading them is the method, not a breach of it.
- A document's license governs its expression only: the prose, tables and
  diagrams as written. It cannot reach the facts they state. Layouts,
  offsets, constants and how an algorithm works are unprotected facts
  (*Feist*); names are short phrases, unprotected too; and elements
  dictated by compatibility are filtered out of any infringement
  comparison (*Altai*). An implementation that uses the facts is not a
  derivative work of the document, so no copyleft attaches.
- Interoperability case law reaches further than this needs: *Sega v.
  Accolade* and *Sony v. Connectix* held that even reading the original
  code to learn a format was fair use. Reading a document written to
  describe the format is well inside that line.

Where the room leaks, and the rules that close it:

- Code excerpts and pseudo-code inside a document are the code. Where a
  document reproduces a function, or points at one as the only exact
  definition (as the ext4 document does for its directory hashes), skip
  it and build from a published algorithm instead, as `filer-ext4` does
  from RFC 1320 and TEA.
- Paraphrase in comments and rustdoc is the real exposure: a document's
  sentence, or a table's description column, drifting into ours. The rule
  against copying or closely paraphrasing prose is what keeps the output
  clean, not the choice of document.
- Layout tables are facts; their descriptive text is not. Transcribe
  offsets, types and field names, and rewrite every explanation in your
  own words.

No choice of input can cure a live patent. That is why exFAT and ReFS are the
genuine problems in this list; everything else, RAR included, is primarily
a matter of copyright hygiene.

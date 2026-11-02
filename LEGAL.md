# Legal status of the file-system formats implemented in this code base

This project reimplements several on-disk file-system formats from scratch and
dedicates the result to the public domain (Unlicense / CC0). This document
records what is known about the intellectual-property status of each format,
which sources contributors may use, and which they may not.

**This is not legal advice.** It is a research summary, US-centric, last
reviewed 2026-11-02. Patent expiry dates below should be re-verified on
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
Documentation* (Russon & Fledel) and Brian Carrier's *File System Forensic
Analysis* are the primary references. NTFS dates to 1993 and the current on-disk
version (3.1) to 2001, so core patents are long expired. Later additions
(Transactional NTFS from 2006, Windows 10-era WOF "system compression") are
more recent; this project targets NTFS 3.1 semantics plus reparse points and
does not implement those later features.

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
Structures* (check its Creative Commons terms before quoting). No known patent
concerns.

Safe inputs: the XFS documentation (facts). Do not copy `xfsprogs` or the
kernel driver (GPL).

### ext4 — clear

Thoroughly documented in the kernel's *ext4 Data Structures and Algorithms*
and by `e2fsprogs`. No known patents; extents, HTree and journaling are old and
rich in prior art.

Safe inputs: kernel documentation (facts). Do not copy `e2fsprogs` (GPL/LGPL)
or the kernel driver.

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

## Summary

| Format    | Patents                          | Format documentation      | Verdict                  |
|-----------|----------------------------------|---------------------------|--------------------------|
| FAT       | expired                          | ECMA-107, fatgen103       | clear                    |
| exFAT     | live until ~Apr 2028             | official spec             | do not release until expiry |
| NTFS      | core expired                     | reverse-engineered only   | low risk                 |
| ReFS      | live into the 2030s              | none; reverse-engineered  | avoid; read-only at most |
| XFS       | expired                          | official                  | clear                    |
| ext4      | none known                       | official                  | clear                    |
| ZFS       | mostly expired, some residual    | 2006 spec + docs          | medium-low               |
| Btrfs     | none known                       | wiki                      | low                      |
| bcachefs  | none known                       | Principles of Operation   | low (format churn)       |
| APFS      | live until ~2036                 | official (partial)        | read-only only           |
| UDF       | expired                          | ECMA-167 / OSTA           | clear                    |
| ISO 9660  | none                             | ECMA-119                  | clear                    |
| SquashFS  | none                             | unofficial                | clear                    |

## Rules for contributors: acceptable and unacceptable inputs

**Acceptable:**

- Standards-body texts (ECMA, IEEE, OSTA, UEFI Forum), read for facts.
- Vendor-published specifications (Microsoft exFAT, Apple APFS, Sun ZFS
  on-disk spec, Microsoft [MS-FSCC]/[MS-FSA]), read for facts and identifiers.
- Peer-reviewed forensic and systems papers.
- Kernel and tool documentation, used as a factual source only.
- Your own black-box observation of real images produced by the reference
  implementation.
- Field names, magic numbers, offsets, constants and struct layouts: these are
  facts, not copyrightable expression.

**Unacceptable:**

- Copying, translating or closely paraphrasing code from any existing driver or
  tool. Every existing implementation of these formats is GPL, LGPL or CDDL;
  copyleft contamination would void the public-domain dedication.
- Copying prose from specification documents into comments, docs or this file.
- Decompiling or disassembling proprietary drivers (`refs.sys`, `ntfs.sys`,
  `exfat.sys`, Apple's `apfs.kext`). Use the published reverse-engineering
  literature instead.
- Submitting code generated by a tool that has reproduced any of the above.

No choice of input can cure a live patent. That is why exFAT and ReFS are the
genuine problems in this list; everything else is primarily a matter of
copyright hygiene.

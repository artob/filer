# Filer.rs design

**Status:** draft design; empty crate skeletons only (revised 2026-10-31).
**Crates:** `filer-api` for the core abstractions, and `filer-*` crates for
individual drivers, middleware, adapters, and tools (for example
`filer-posix`, the `rustix`-based POSIX driver, and `filer-cli`, the
`filer` command).

This document records *why* Filer.rs is shaped the way it is. It is written
for whoever implements it, very likely a future session of the model that
wrote it, so it favors completeness and stated rationale over brevity. It
began as a design for a `dogma-fs` crate inside Dogma.rs; the project is now
standalone but keeps Dogma.rs's engineering conventions (§3).

It is not user documentation. Per `AGENTS.md`, user-facing explanations
belong in rustdoc on the relevant modules and types; work items belong in
[`TODO.md`](TODO.md). When the code and this document disagree, fix one of
them in the same change and record the outcome in the
[decision log](#28-decision-log).

Conventions used here: "must" marks a design invariant, "should" marks a
strong default that needs a recorded reason to break, and **VERIFY** marks a
platform or ecosystem fact that was believed true when written but must be
re-checked before code depends on it (each has a matching `TODO.md` item).
Facts checked against sources are marked with the date they were checked.

## Contents

1. [Summary](#1-summary)
2. [Goals and non-goals](#2-goals-and-non-goals)
3. [Project conventions](#3-project-conventions)
4. [Architecture and crate map](#4-architecture-and-crate-map)
5. [Names and paths](#5-names-and-paths)
6. [Capabilities and resolution](#6-capabilities-and-resolution)
7. [Async model](#7-async-model)
8. [Handles, files, and opening](#8-handles-files-and-opening)
9. [Directory enumeration](#9-directory-enumeration)
10. [Metadata](#10-metadata)
11. [Extended attributes and file flags](#11-extended-attributes-and-file-flags)
12. [Copies, clones, and sparse files](#12-copies-clones-and-sparse-files)
13. [Durability](#13-durability)
14. [Atomic replacement](#14-atomic-replacement)
15. [Transactions](#15-transactions)
16. [The driver boundary](#16-the-driver-boundary)
17. [Drivers](#17-drivers)
18. [Middleware](#18-middleware)
19. [Errors](#19-errors)
20. [Walking and recursive operations](#20-walking-and-recursive-operations)
21. [Platform notes](#21-platform-notes)
22. [Workspace layout and features](#22-workspace-layout-and-features)
23. [Dependencies](#23-dependencies)
24. [Testing strategy](#24-testing-strategy)
25. [Observability](#25-observability)
26. [Excluded on purpose](#26-excluded-on-purpose)
27. [Prior art](#27-prior-art)
28. [Decision log](#28-decision-log)
29. [Open questions](#29-open-questions)

Appendices: [A. Glossary](#appendix-a-glossary),
[B. Primitive mapping](#appendix-b-primitive-mapping).

## 1. Summary

Filer is an async, capability-based filesystem API. Six ideas carry the
whole design:

1. **Authority is a handle.** An open directory handle (`Dir`) is the unit of
   authority. There is no free function that takes a path and no current
   working directory. Ambient access enters only through one explicit,
   greppable token.
2. **Names are UTF-8 by type, but enumeration is total.** Every name a caller
   can type is valid UTF-8. Names found on disk that are not UTF-8 are
   surfaced as opaque values that can be opened, renamed, and removed, but
   never created or forged.
3. **Operations own their buffers.** Every I/O operation takes its buffers by
   value and returns them, so completion-based kernels (io_uring, IOCP) fit
   without unsoundness. Futures are `'static`; once submitted, an operation
   runs to completion and dropping its future only detaches it.
4. **Operations are data at the driver boundary.** The ergonomic frontend is
   typed, but backends implement one `submit(Op)` method. That single seam
   makes VFS layering, middleware, per-operation fallback, batching, record
   and replay, and fault injection cheap.
5. **Guarantees are reported, not emulated.** Whether a resolution was
   kernel-confined, whether a copy was a clone, whether a commit was
   atomic or merely crash-atomic: each comes back in a type. Nothing degrades
   silently unless the caller opted in.
6. **Crash consistency is a primitive and is testable.** Atomic replacement,
   transactions, and race-free recursive operations are library features,
   and an in-memory driver models power loss so that application code can be
   property-tested against it.

## 2. Goals and non-goals

### Goals

- Async throughout, runtime-agnostic, with a portable blocking-thread-pool
  driver as the default and an io_uring driver where available.
- Capability-based access: every operation resolves relative to a handle and
  is confined beneath it, kernel-enforced where the OS allows.
- UTF-8 names only in the API surface; total over what is actually on disk.
- First-class extended metadata (`statx`-class), extended attributes, file
  flags, reflinks, server-side copies, and sparse files, each with honest
  capability reporting.
- Durable, crash-consistent primitives: explicit durability levels, atomic
  replacement, and transactions with an explicit atomicity guarantee and
  best-effort rollback.
- Pluggable drivers, including virtual ones: in-memory, mount tables,
  overlays, read-only archives, and fault injection.
- Builds as `#![no_std]` (with allocation) for the lexical and virtual parts,
  per project policy; OS access requires `std`.

### Non-goals

- Being a drop-in replacement for `std::fs`. Code that wants ambient,
  path-based, synchronous access should keep using `std::fs`.
- Creating or typing non-UTF-8 names. Legacy names are tolerated, not
  produced. Programs that must manipulate them should use legacy APIs.
- Object-storage abstraction (S3 and friends). Apache OpenDAL does that well;
  object stores lack rename, directories, and the semantics this crate is
  about. A future driver may adapt one, but it shapes nothing here.
- Memory mapping in the async core (see [§26](#26-excluded-on-purpose)).
- Emulating POSIX permissions, ownership, or ACLs where they do not exist.
- Text encodings, line endings, user and group name lookup.
- Locks as a correctness mechanism. Advisory locks exist for cooperative
  coordination only.

## 3. Project conventions

These conventions are carried over from Dogma.rs, adapted where a
multi-crate filesystem project needs more. They will be restated tersely in
`AGENTS.md`, which is written for smaller models with small context
windows; this section holds the rationale.

### 3.1 Workspace and toolchain

- Crates live in `lib/<crate>/`. The root `Cargo.toml` owns shared package
  metadata, `[workspace.dependencies]` (always `default-features = false`,
  enabled per crate), and `[workspace.lints]`; every crate sets `[lints]
  workspace = true`.
- Rust 2024 edition, resolver 3, MSRV 1.97, enforced everywhere it is
  stated (manifest, README badge, CI matrix, `AGENTS.md`).
- A `VERSION` file holds the single project version. Package versions and
  internal dependency versions stay aligned with it; internal dependencies
  keep default features disabled.
- Versions start at 0.0.0. Releases stay at 0.0.x while the API is
  extremely experimental and unstable: under Cargo's rules every 0.0.x
  release is incompatible with every other, which states that honestly
  (D29). 0.1.0 waits for the criteria at the end of `TODO.md`.
- License: Unlicense, as in Dogma.rs (carried over; see Q10).
- Repository: <https://github.com/artob/filer> (D31).
- Support crates that are not Filer-specific are named for what they are,
  not `filer-*`, and are written to be spun out: `known-codepages` (DOS
  and Windows code pages, used by `filer-fat` and later `filer-zip`) will
  move to <https://github.com/it-is-known/known-codepages> (D49); and
  the `decode-*` crates, our clean-room, public-domain decoders, such as
  `decode-zstd` and `decode-bzip2` (D62); and the `compute-*` crates, our
  checksums and hashes, such as `compute-crc` (D64).
- Root files: `AGENTS.md`, `README.md`, `CHANGES.md` (Keep a Changelog,
  Semantic Versioning, **Breaking** markers), `TODO.md`, `DESIGN.md`,
  `VERSION`, `UNLICENSE`, `AUTHORS`, `CREDITS.md`, `.rustfmt.toml` (same
  settings as Dogma.rs), and `.github/workflows/ci.yaml`.

### 3.2 `no_std` and features

- Every crate is always buildable with `#![no_std]` and exports an explicit
  `std` feature. Crates that cannot do anything useful without allocation
  (all of them, in practice) use `extern crate alloc` unconditionally and
  have no `alloc` feature. `extern crate std` appears only under `std`.
- OS driver crates (`filer-posix`, `filer-wasi`, `filer-windows`,
  `filer-uring`) export nothing without `std`, and nothing on targets they
  do not support (`cfg(unix)`, `cfg(target_os = "wasi")`, `cfg(windows)`,
  Linux). They still build there, as empty shells, so workspace-wide
  feature checks stay uniform. Note that WASI targets are not `cfg(unix)`.
- `filer-cli` is the one binary crate. Its `[[bin]]` has
  `required-features = ["std"]`, so `no_std` builds of the package skip it,
  and it depends on each OS driver only under that driver's `cfg`.
- `[features]` starts with `default = ["all", "std"]`. `all` collects the
  default non-`std` features and must build for `thumbv7em-none-eabihf` in
  every crate. Every feature must build alone on the host. All features
  must build for `wasm32-unknown-unknown`, `wasm32-wasip1`, and
  `wasm32-wasip2`, so OS dependencies are target-gated
  (`[target.'cfg(...)'.dependencies]`) and OS modules are `cfg`'d out.
- APIs exempt from semver guarantees go behind an `unstable` feature;
  whole experimental crates use `publish = false` until their gates pass
  (D16).

### 3.3 Unsafe code (D11)

- The workspace lint table sets `unsafe_code = "deny"`,
  `unsafe_op_in_unsafe_fn = "deny"`, and Clippy's
  `undocumented_unsafe_blocks = "deny"`.
- Every crate without `unsafe` (currently all but `filer-windows`,
  including the empty OS driver shells) also declares
  `#![forbid(unsafe_code)]` at the crate root, marking it unsafe-free in a
  way no module can override.
- A driver crate that comes to need `unsafe` (expected: `filer-windows`,
  `filer-uring`, and `filer-posix` only for gaps in `rustix`, see §17.1)
  switches its crate root from `forbid` to `deny` in the same change and
  confines all `unsafe` to one private `sys` module (or a small `sys/`
  tree) carrying `#[allow(unsafe_code)]`. Every `unsafe` block has a
  `// SAFETY:` comment; every `unsafe fn` documents its contract. The rest
  of the crate is safe code over `sys`'s safe wrappers.
- The README lists which crates contain `unsafe` and why.
- Gaps in safe dependencies are fixed upstream first (for example, PRs to
  `rustix`); local `unsafe` is a stopgap with a `TODO.md` item to remove it.

### 3.4 Code style and documentation

- Refer to `core`, `alloc`, and `std` items by qualified path or explicit,
  least-power imports (`core::error::Error`, `alloc::sync::Arc`). Sketches
  in this document follow that rule.
- Flat, type-focused source modules (`mod rights; pub use rights::*;`).
- Lints: `missing_docs` and `rust_2018_idioms` warn,
  `clippy::must_use_candidate` warns, and CI runs Clippy with `-D
  warnings`. Rustdoc coverage of every public item is a goal; brevity and
  quality matter. Run `cargo doc` as the last step after changing a crate.
- Error enums are `#[non_exhaustive]`.
- Robustness principles (D47), for all code: async first (anything that
  may touch storage is async); no panics on any input (no `unwrap`,
  `expect`, `panic!`, unchecked indexing, or unchecked arithmetic in
  library code, which every library crate's root denies by Clippy lint);
  illegal states unrepresentable (validated newtypes and enums
  constructed only by parsing, rather than checked flags); and no error
  ever ignored (no `let _ =` on a `Result`, no defaulting away a
  failure). The few panics left are allowed where they stand, each with
  its reason: a builder's on invalid arguments, a check evaluated at
  compile time, a driver breaking its contract, or ids running out.
  `filer-spec` is exempt, as its assertions are how it reports failures,
  and so are tests, binaries, and examples.
- Builders (fixtures such as `HeapBuilder`, and later configuration
  builders) maximize developer usability: path arguments take
  `P: TryInto<RelPathBuf>` so plain strings work besides `rel!` literals,
  and invalid input panics under `#[track_caller]`. Operations and driver
  APIs stay strictly typed, so drivers never re-validate.
- Markdown and comments wrap at 80 columns where feasible.
- The README is curated: longer is not better. Documentation that wants to
  go there usually belongs in rustdoc.

### 3.5 Process

- One narrowly scoped, atomic change per request; split larger goals in
  `TODO.md` first. Remove completed items after verification.
- Add regression coverage with behavior changes; behavior tests live in the
  crate that owns the behavior.
- User-visible changes get a `CHANGES.md` entry under "Unreleased".
- Commits only when explicitly requested.
- Checks, from the repository root:

```sh
cargo fmt --all -- --check
cargo test --workspace
cargo test --workspace --all-features
cargo check --workspace --no-default-features
cargo check --workspace --no-default-features --features all
cargo doc --workspace --all-features --no-deps
```

Also check affected crates individually with default features disabled,
because workspace feature unification can hide missing gates.

### 3.6 Continuous integration

Carried over from Dogma.rs: formatting; Clippy with all features; docs with
`-D warnings`, plus a nightly docs.rs-settings build (which is also how
docs are read locally: from each `#[cfg]`, rustdoc infers a badge naming
the feature or platform an item needs, so `doc(cfg)` is never written by
hand); packaged doctests
(`cargo package --workspace`, then each package's doctests with sibling
packages patched in, under default, all, none, and `all`-only features);
feature checks (no defaults, `all`, every feature alone, `all` on
`thumbv7em-none-eabihf`, all features on `wasm32-unknown-unknown`); minimal
direct dependency versions; build and test on Ubuntu and Windows at MSRV and
stable.

New for Filer.rs: a macOS job (APFS clones, `F_FULLFSYNC`, no `openat2`),
all-feature checks on `wasm32-wasip1` and `wasm32-wasip2` (for
`filer-wasi`), filesystem fixtures on Linux (§24), and the specification
suite run against every driver. The packaged-doctest job skips `filer-cli`,
which has no library and so no doctests.

### 3.7 Pure Rust (D45)

Filer is a 100% Rust implementation. No dependency, direct or
transitive, may compile or bundle C, C++, or other non-Rust code: no
`cc`, `cmake`, `pkg-config`, or `bindgen` in the build, and no `-sys`
crates wrapping C libraries (zlib, zstd, bzip2, liblzma, libarchive,
libfuse, and the like). Where a pure-Rust alternative is missing, the
feature waits or is written here.

Declaring the operating system's own interfaces to Rust is not C code and
remains allowed where the OS offers no other stable ABI: `libc` bindings
on macOS and the BSDs (their system call interface is private), and
`windows-sys` on Windows. On Linux, `rustix` calls the kernel directly,
with no C library at all.

Consequences: compression codecs are pure-Rust decoders (Deflate, zstd,
LZMA, xz, and bzip2 written here, as `LEGAL.md` assesses them), which
suffices since archive drivers only read; `filer-fuse` must use a FUSE
mount path that needs no `libfuse` (**VERIFY** `fuser` without its
`libfuse` feature). CI fails if `cc`, `cmake`, `pkg-config`, or
`bindgen` enters the dependency tree on any target.

**Fuzzing tooling (D50).** The one exemption: `cargo-fuzz` targets, in a
`fuzz/` crate inside the crate they fuzz, may depend on `libfuzzer-sys`,
which compiles LLVM's C++ libFuzzer, as coverage guidance has no
maintained pure-Rust equivalent. Such a crate has its own `[workspace]`,
so it is outside the repository's workspace and its C-code check, is left
out of the packaged crate, and is never a dependency of a published
crate. A pure-Rust fuzz test in `tests/` remains the baseline, running
with every `cargo test`.

### 3.8 The umbrella crate

Following Dogma.rs, an umbrella crate re-exports `filer-api` at the top
level and component crates as modules (`posix`, `windows`, `heap`, ...),
forwarding features; component crates never depend on it, and umbrella
tests only check re-exports and forwarding. Its name is open: `filer` on
crates.io is an unrelated 0.0.0 placeholder (Q11).

## 4. Architecture and crate map

```text
            application code
                   |
  +----------------v-----------------+
  | Frontend (typed, rights-checked) |  filer-api: Dir, File<M>, Open<M>,
  |                                  |  ReadDir, AtomicWriter, Walker
  |                                  |  (+ filer-tx: Transaction)
  +----------------+-----------------+
                   | Op in, Completion out (owned buffers, 'static)
  +----------------v-----------------+
  | Middleware                       |  filer-tracing, filer-faults,
  |                                  |  filer-quota, filer-replay
  +----------------+-----------------+
                   |
  +----------------v-----------------+
  | Routing                          |  filer-vfs, filer-overlay,
  |                                  |  filer-api's Hybrid combinator
  +----------------+-----------------+
                   |
  +----------------v-----------------+
  | Concrete drivers                 |  filer-posix, filer-windows,
  |                                  |  filer-uring, filer-heap,
  |                                  |  filer-tar, filer-zip
  +----------------------------------+
```

- The **frontend** is what users touch. It validates names, enforces rights,
  chooses coarse operations and chains, and turns completions back into
  typed results.
- The **driver boundary** is a single method taking an `Op` value. Every
  layer below the frontend is "a driver", including middleware and routers,
  so layers compose by wrapping.
- **Middleware** matches the operations it cares about and forwards the rest,
  including operations added after it was written (the enum is
  `#[non_exhaustive]`).
- **Routing drivers** dispatch by path prefix (`filer-vfs`), by layer
  (`filer-overlay`), or by operation kind (`Hybrid`).
- **Concrete drivers** talk to an OS or hold state in memory.

### 4.1 Crates

| Crate | Role | `unsafe` |
| --- | --- | --- |
| `filer-api` | types, paths, `Op`, `Driver`, frontend, algorithms | no |
| `filer-posix` | `rustix` driver: Linux, macOS, BSDs | stopgaps only |
| `filer-wasi` | WASI driver over preopens | no (§17.8) |
| `filer-windows` | `windows-sys` driver | yes, in `sys` |
| `filer-whatwg` | WHATWG File System driver (browsers, OPFS) | no (§17.11) |
| `filer-uring` | io_uring driver over `filer-posix` | yes, in `sys` |
| `filer-heap` | in-memory driver with a crash model | no |
| `filer-vfs` | mount-table routing driver | no |
| `filer-overlay` | overlay routing driver | no |
| `filer-tar`, `filer-zip` | read-only archive drivers | no |
| `filer-ar`, `-cpio`, `-rar`, `-7z` | read-only archive drivers | no |
| `filer-archive` | any archive format, by content or name, over those drivers | no |
| `filer-pack` | shared machinery of archive drivers (§17.7, D59) | no |
| `filer-volume` | any file system, by content, over their drivers (§17.18) | no |
| `filer-tracing` | tracing middleware | no |
| `filer-faults` | fault-injection middleware | no |
| `filer-quota` | quota middleware | no |
| `filer-replay` | record and replay middleware | no |
| `filer-tx` | transactions (experimental) | no |
| `filer-tokio` | Tokio spawner and I/O trait adapters | no |
| `filer-spec` | executable driver specification, as a library | no |
| `filer-fuse` | FUSE export of any driver | no |
| `filer-uefi` | UEFI driver (planned, §17.12) | TBD |
| `filer-9p`, `-smb`, `-nfs`, `-sftp` | remote drivers (planned) | no |
| `filer-fat` | FAT12/16/32 driver (§17.13) | no |
| `filer-fs` | shared machinery of file system drivers (§17.17) | no |
| `filer-msft` | Microsoft's on-disk conventions (§17.17, D60) | no |
| `filer-exfat` and others | on-disk formats (planned) | no |
| `filer-system` | the native driver for the target, plus the API | no |
| `filer-cli` | the `filer` binary over the native driver | no |
| umbrella (Q11) | re-exports and feature forwarding | no |

Existing so far (2026-11-03): `filer-api`, `filer-heap`, `filer-posix`,
`filer-windows`, `filer-system`, `filer-spec`, `filer-fs`, `filer-msft`,
`known-codepages`, `compute-adler32`, `compute-crc`, `compute-fletcher`,
`compute-sha256`, `compute-sha512`, `compute-xxhash`, `decode-bzip2`,
`decode-deflate`, `decode-gzip`, `decode-lz4`, `decode-lzma`,
`decode-lzo`, `decode-xz`, `decode-zlib`, `decode-zstd`, `filer-pack`,
the read-only `filer-tar`, `filer-zip`, `filer-ar`, `filer-cpio`, and
`filer-archive` over them, `filer-cdfs`, `filer-ext4`, `filer-fat`,
`filer-ntfs`, `filer-squashfs`, `filer-udf`, `filer-xfs`, and
`filer-volume` over them, and as empty skeletons `filer-wasi`,
`filer-whatwg`, and `filer-cli` (D30, D38, D41). Stubs reserving names
live in `tmp/` (D58): `filer-rar` and `filer-7z`, and the file systems
`filer-apfs`, `filer-btrfs`, `filer-exfat`, `filer-minix`, `filer-refs`,
`filer-ufs`, and `filer-zfs`; and `compute-blake2` and `compute-blake3`
(D64). The rest are created when their first `TODO.md` item starts.

Placement rule (D20): `filer-api` holds the abstractions plus combinators
that perform no I/O and hold no filesystem state (`DynDriver`, `Hybrid`,
`ReadOnly`, the `Layer` trait), and generic algorithms written purely
against the `Driver` trait (atomic replacement, walking, copying, the
userspace resolver). Anything with its own semantics, state, or I/O is its
own crate.

Main type per crate: `filer_system::System`, `filer_posix::Posix`,
`filer_windows::Windows`,
`filer_uring::Uring`, `filer_heap::Heap`, `filer_vfs::Vfs`,
`filer_overlay::Overlay`, `filer_tar::Tar`, `filer_zip::Zip`.

### 4.2 Dependency graph

```text
filer-api <--+-- filer-posix <---- filer-uring
             +-- filer-wasi
             +-- filer-windows
             +-- filer-heap
             +-- filer-vfs, filer-overlay
             +-- filer-pack <-- filer-tar, filer-zip, filer-ar, filer-cpio,
             |                  and filer-archive over them (D59)
             +-- filer-fs <---- filer-fat, filer-ext4, filer-ntfs, filer-xfs,
             |                  and the stubs in tmp/ (D58)
             +-- filer-msft <-- filer-fat, filer-ntfs, filer-windows,
             |                  filer-zip, filer-7z, filer-rar, and the
             |                  stubs needing them (D60)
             +-- filer-tracing, filer-faults, filer-quota, filer-replay
             +-- filer-tx, filer-tokio, filer-fuse
             +-- filer-spec          (dev-dependency of every driver)
```

`filer-cli` depends on `filer-api` and on `filer-posix`, `filer-wasi`, and
`filer-windows`, each under its own target `cfg`; it is a leaf that nothing
depends on, and unlike the umbrella it is a tool, not a library.

`filer-uring` depends on `filer-posix` because it reuses its handle payloads
and falls back to it for operations io_uring cannot do (§16.5).
`filer-heap` is a dev-dependency of crates whose tests need a crash model
(`filer-tx`, `filer-replay`).

## 5. Names and paths

### 5.1 Types and where they live

Path types start in `filer-api` (D3), in a self-contained `filer_api::path`
module that depends only on `core`, `alloc`, and optional `serde`, so that
it can later move into Dogma.rs's `dogma-path` with `filer-api` re-exporting
it, without breaking users.

| Type | Module | Meaning |
| --- | --- | --- |
| `Name` | `filer_api::path` | one valid component |
| `NameBuf` | `filer_api::path` | owned `Name` |
| `RelPath` | `filer_api::path` | zero or more `Name`s joined by `/` |
| `RelPathBuf` | `filer_api::path` | owned `RelPath` |
| `ForeignName` | `filer_api` | a non-UTF-8 name found on disk |
| `EntryName` | `filer_api` | `Name` or `ForeignName`, from enumeration |
| `LinkTarget` | `filer_api` | UTF-8 or foreign symlink target |

`ForeignName`, `EntryName`, and `LinkTarget` stay outside `path` because
they only arise from filesystem enumeration and carry driver provenance
(see [§5.5](#55-foreign-names)); they would not move with the lexical types.

### 5.2 Validation rules

A `Name` must be:

- non-empty valid UTF-8;
- free of `/` and NUL;
- neither `.` nor `..`.

Nothing else is rejected lexically. Control characters, backslashes,
colons, and trailing dots are legal on some filesystems; whether a name is
acceptable *on a given directory* is a driver question answered by
[`NamingRules`](#56-naming-rules-per-directory) and reported as
`ErrorKind::InvalidNameForTarget`. There is deliberately no lexical length
limit: limits differ per filesystem and per unit (bytes versus UTF-16).

A `RelPath` is zero or more `Name`s joined by single `/` characters:

- the empty path `""` means "the directory itself";
- no leading or trailing `/`, no empty components;
- every component is a valid `Name` (so no `.` and no `..`).

Banning `..` is deliberate. In a capability world the way up is a handle
you already hold. Symlinks containing `..` are still followed during
resolution, under kernel-enforced confinement where available.
`RelPathBuf::normalize_lexically(s, separators)` reads strings typed by
users leniently: it drops `.` components and redundant separators, and
with `Separators::SlashOrBackslash` reads `\` as one too, but refuses
`..`, roots (`/`, `\`, `\\server`), and drive prefixes (`C:`), as
`NormalizeError`: drive prefixes even with `Separators::Slash`, since
`C:` names a drive on Windows.

The separator in `RelPath` is always `/`, on every platform. Drivers
translate.

### 5.3 Representation

The idiomatic shape is an unsized newtype: `#[repr(transparent)] struct
RelPath(str)`, borrowed as `&RelPath`, owned as `RelPathBuf` with `Deref`,
`Borrow`, and `ToOwned` (so `Cow<'_, RelPath>` works). Building `&RelPath`
from `&str` needs a pointer cast, and `filer-api` forbids `unsafe` (§3.3).

Decision (D4): use `ref-cast`. `#[derive(RefCast)]` generates the cast, and
its derive deliberately emits the `unsafe` block with a macro-generated span
so that a caller's `forbid(unsafe_code)` accepts the expansion (confirmed by
reading the `ref-cast-impl` 1.0.27 source, 2026-10-31).

Verified 2026-11-01 with a `no_std` scratch crate carrying the workspace
lints and `#![forbid(unsafe_code)]`, on Rust 1.97.0 and 1.99.1, with
`ref-cast` 1.0.27 and `default-features = false`:

- `#[derive(RefCast, RefCastCustom)]` on `#[repr(transparent)] struct
  RelPath(str)` compiles; Clippy (including `undocumented_unsafe_blocks`)
  is clean. `#[ref_cast_custom]` needs the `RefCastCustom` derive; `RefCast`
  alone fails with `RefCastCustom<str>` not satisfied.
- A private `#[ref_cast_custom] const fn from_str_unchecked(&str) -> &Self`
  works in `const` context, so `rel!` expands to a `const` item calling a
  `const fn` validator that panics on invalid input. An invalid literal is
  a compile error (`E0080: evaluation panicked`).
- `Deref`, `Borrow`, `ToOwned`, and `Cow<'static, RelPath>` work, and the
  crate builds for `thumbv7em-none-eabihf`.
- `forbid` still rejects a hand-written `unsafe` block in the same crate.

Rejected fallback, kept for the record: borrowed views
(`#[derive(Clone, Copy)] struct RelPath<'a>(&'a str)`), which are safe but
lose `Deref`, `Cow<RelPath>`, and `&RelPath` ergonomics.

Owned forms use `alloc::boxed::Box<str>` or `String`
internally, and `Op` payloads use owned forms so that futures stay
`'static`.

### 5.4 Literal macros

`filer_api::rel!("config/app.toml")` and `filer_api::name!("app.toml")`
validate at compile time via `const fn` validators and produce
`&'static RelPath` / `&'static Name`. Invalid literals are compile errors.

### 5.5 Foreign names

UTF-8-only cannot mean the disk is UTF-8-only. Enumeration must be total:

- A listing that silently skips undecodable names produces backups with
  holes.
- A recursive delete that cannot see an entry fails with "directory not
  empty" for no visible reason.

WASI chose to make such files inaccessible; `cap-std`'s `fs_utf8` chose to
encode them into UTF-8 with ARF strings. Filer chooses opaque values:

```rust
/// A name found on disk that is not valid UTF-8 (or, on Windows, not
/// well-formed UTF-16). It has no public constructor.
pub struct ForeignName {
    raw: alloc::boxed::Box<[u8]>, // WTF-8 on Windows, raw bytes elsewhere
    origin: DriverId,             // the driver instance that produced it
}

pub enum EntryName {
    Utf8(path::NameBuf),
    Foreign(ForeignName),
}

pub enum LinkTarget {
    Utf8(alloc::string::String),
    Foreign(alloc::boxed::Box<[u8]>),  // never accepted as input
}
```

Drivers live in other crates, so "no public constructor" is enforced by a
credential rather than by visibility (D32). Each driver instance holds a
private `DriverKey`, which is neither `Clone` nor `Copy`. The key mints the
instance's unique `DriverId` (a `usize` counter, since
`thumbv7em-none-eabihf` lacks 64-bit atomics) and is the only constructor
of `ForeignName`s carrying that id: `key.entry_name(raw: &[u8])` returns
`EntryName::Utf8` for valid UTF-8 names and `EntryName::Foreign` otherwise,
rejecting empty names, `/`, NUL, `.`, and `..` with a `NameError`. Anyone
can create a key, but names from it carry an id no real driver accepts.
Thus a `ForeignName`'s bytes are never valid UTF-8. `LinkTarget` needs no
such protection, since it is only ever output, so it has a public
`from_bytes`.

Rules:

- Enumeration yields `EntryName`.
- `open`, `remove`, `rename` (as the *source*), `stat`, and xattr reads
  accept an `EntryName`. A `ForeignName` is only accepted by a handle from
  the same driver instance that produced it; otherwise
  `ErrorKind::ForeignNameMismatch`.
- A `ForeignName` can never be a creation target or a rename *destination*.
- Symlink targets follow the same rule: `read_link` returns a `LinkTarget`
  (`Utf8` or `Foreign`), and `symlink` only accepts UTF-8 targets.
- `Display` is an escaped, lossy rendering (`\xNN` for invalid bytes),
  documented as non-round-trippable.
- No `serde` implementation. Persisting a foreign name is out of scope.
- Test-only construction is offered by `filer-heap`, not by the type.

With Dogma.rs interop (§5.7), `EntryName` would implement
`dogma_traits::MaybeNamed`, returning the UTF-8 name or `None` for foreign
names.

### 5.6 Naming rules per directory

The API never normalizes or case-folds names, but it reports how a directory
will treat them, because this varies per directory, not per platform:

- ext4 casefolding is a per-directory attribute;
- APFS is normalization-insensitive but normalization-preserving;
- NTFS is case-insensitive by default with per-directory case sensitivity;
- name length limits are in bytes on ext4 (255) but in UTF-16 units on NTFS
  (255), so a 100-character CJK name (about 300 UTF-8 bytes) fits on NTFS
  but not on ext4.

```rust
#[non_exhaustive]
pub struct NamingRules {
    pub case: CaseSensitivity,          // Sensitive | Insensitive | Unknown
    pub normalization: Normalization,   // Preserving | Insensitive | Nfd ...
    pub max_name_bytes: Option<u32>,
    pub max_name_utf16: Option<u32>,
    pub reserved: ReservedNames,        // e.g. Windows device names
    pub forbidden_chars: &'static [char],
}

impl NamingRules {
    /// Would `a` and `b` refer to the same entry in this directory?
    pub fn collides(&self, a: &Name, b: &Name) -> Collision; // Yes|No|Maybe
}
```

A separate `portability` lint (`Name::portability_issues()`) reports names
that would be invalid or colliding on *any* mainstream filesystem. Git's
CVE-2014-9390 (`.Git` versus `.git` on case-insensitive volumes, plus HFS+
ignorable code points) is the canonical failure of assuming byte-equal means
same file.

### 5.7 Relationship to Dogma.rs

`dogma-path` currently owns one type, `AncestorPath` (`../`, `../../`, ...),
which is exactly what a confined `RelPath` forbids. Its `std` methods
(`is_dir`, `exists`, `try_exists`) resolve against the process's current
directory, which makes them ambient-authority operations. If Filer.rs grows
Dogma.rs interop, the clean integration point is the ambient boundary only:
`Dir::open_ambient_ancestor(AncestorPath, AmbientAuthority)`, behind an
optional `dogma-path` feature. A `Dir` never resolves an `AncestorPath`.

Likewise, `dogma-uri` converts `file:` IRIs to native paths; a
`Dir::open_ambient_iri` would belong at the same boundary. Both are
low-priority interop, and the reverse direction (`dogma-path` adopting
`filer_api::path`) is the D3 follow-up.

## 6. Capabilities and resolution

### 6.1 No ambient authority

There is no free function that takes a path and no notion of a current
directory. Roots are obtained through an `AmbientAuthority` token:

```rust
/// Proof that the caller deliberately reaches into the ambient namespace.
/// Greppable, lintable (`clippy::disallowed_methods`), and deniable.
#[derive(Clone, Copy, Debug)]
pub struct AmbientAuthority(());

#[cfg(feature = "std")]
pub fn ambient_authority() -> AmbientAuthority { AmbientAuthority(()) }

impl<D: Driver> Dir<D> {
    #[cfg(feature = "std")]
    pub async fn open_ambient(
        driver: D,
        path: &std::path::Path,   // the one place native paths appear
        authority: AmbientAuthority,
    ) -> Result<Self>;
}
```

This mirrors `cap-std`. A code review can find every place authority enters
the program, and a library cannot conjure access it was not handed.
Preopened directories from a WASI host or file descriptors inherited from a
supervisor become `Dir`s through `Dir::from_owned_fd` /
`Dir::from_owned_handle`, which also require `AmbientAuthority`.

### 6.2 Rights

Rights are coarse, on purpose. WASI preview 1 gave every descriptor separate
base and inheriting rights with dozens of bits; WASI 0.2 replaced them with
a handful of flags because the fine grain was rarely used correctly.

```rust
bitflags::bitflags! {
    pub struct Rights: u8 {
        /// Read file contents; list a directory; read metadata and xattrs.
        const READ   = 1 << 0;
        /// Write file contents, truncate, allocate, punch holes.
        const WRITE  = 1 << 1;
        /// Create, remove, rename, and link entries in a directory.
        const MUTATE = 1 << 2;
        /// Change permissions, times, ownership, xattrs, and flags.
        const META   = 1 << 3;
    }
}
```

Operation requirements (on the handle the operation is issued against):

| Operation | Rights |
| --- | --- |
| open file for reading, `read_dir`, `stat`, read xattrs | `READ` |
| open file for writing | `WRITE` on the result; see below |
| create, remove, rename, link, symlink, mkdir | `MUTATE` |
| set perms, times, owner, xattrs, flags | `META` |
| clone into a file | `WRITE` on the destination |
| sync | any |

Opening a file for writing through a `Dir` requires the `Dir` to carry
`WRITE` (so the right is inherited), and creating it additionally requires
`MUTATE`. The frontend checks rights before submitting an op; drivers never
see an op the caller lacked rights for.

### 6.3 Attenuation and inheritance

- `Dir::restrict(Rights) -> Dir` only removes rights; amplification is
  impossible by construction.
- A child `Dir` opened through a parent gets at most the parent's rights.
- A `File` opened through a `Dir` gets at most the parent's `READ`, `WRITE`,
  and `META`; `MUTATE` is meaningless on files and is dropped.
- Rights are carried by the frontend handle, not the driver handle, so two
  `Dir` values may share one OS descriptor with different rights.

### 6.4 Resolution policy

```rust
pub enum Symlinks {
    /// Refuse to traverse any symlink (RESOLVE_NO_SYMLINKS).
    Deny,
    /// Follow symlinks, but fail if resolution would leave the handle's
    /// subtree (RESOLVE_BENEATH). The default.
    Beneath,
    /// Follow symlinks, treating absolute targets and `..` at the top as
    /// referring to this handle (RESOLVE_IN_ROOT), chroot-style.
    InRoot,
}

pub struct Resolve {
    pub symlinks: Symlinks,
    pub cross_mounts: bool, // false = RESOLVE_NO_XDEV
}
```

Magic links (`/proc/self/fd/*` and friends) are never followed during user
path resolution. Each `Dir` carries a default `Resolve`; individual
operations may tighten it, never loosen it.

### 6.5 Enforcement per platform

| Platform | Mechanism | `Enforcement` |
| --- | --- | --- |
| Linux 5.6+ | `openat2` + `RESOLVE_*` | `Kernel` |
| FreeBSD 13+ | `O_RESOLVE_BENEATH` | `Kernel` |
| older Linux, macOS, other Unix | component walk, `O_NOFOLLOW` | `Userspace` |
| Windows | component walk relative to handles | `Userspace` |
| virtual drivers | the driver's own resolver | `Virtual` |

The userspace walk resolves one component at a time relative to the previous
handle, reading and re-resolving symlinks itself with a hop limit (40, like
Linux `MAXSYMLINKS`). It is slower but still race-free, because no step
re-parses a string from the root. `Features::enforcement` reports which
mechanism applies, per directory.

As built (2026-11-01), the walk is the `filer_api::Resolver` trait: drivers
implement four primitives (`lookup` one name without following, `kind`,
`read_link`, and optionally `crosses_mount`), and the provided `resolve`
method walks a byte path (so non-UTF-8 targets and foreign names work)
under a `Resolve` policy, with `follow_final` choosing whether a final
symlink is followed. It keeps the chain of directories it entered; `..`
pops that chain, so it cannot reach anything outside it. Confinement is
relative to the start handle: `..` above it or an absolute target is
`EscapeBlocked` under `Beneath`, and clamps to or restarts from it under
`InRoot`; any symlink to follow is `SymlinkLoop` under `Deny` (as
`RESOLVE_NO_SYMLINKS` gives `ELOOP`), as is a 41st hop
(`MAX_SYMLINK_HOPS`). A driver with kernel confinement can override
`resolve`.

Being async first, `Resolver`'s primitives return `Send` futures, for
drivers whose lookups await I/O (a filesystem image over `ReadAt`,
network protocols, the browser's file system API). Drivers whose
primitives block (system calls, memory accesses) implement
`BlockingResolver` instead, whose synchronous primitives fit every
executor: a thread-pool driver runs the whole walk as one job (D48). Both
traits drive one private state machine, `Walk`, which holds the policy and
the chain of directories and does no I/O, so the confinement logic exists
once and both are tested against each other.

On Linux, `openat2` can fail with `EAGAIN` under `RESOLVE_*` when a rename
races the lookup; the driver retries a bounded number of times, then falls
back to the userspace walk. `ENOSYS` (old kernel, seccomp filter) selects the
userspace walk permanently for that driver instance.

### 6.6 Handle-based operations after open

After open, everything goes through the handle: metadata (`fstat`/`statx`
with `AT_EMPTY_PATH`), permissions (`fchmod`), xattrs (`fgetxattr`), sync,
truncation. No operation re-resolves a path to act on something already
open. Check-then-open patterns are impossible to express.

Operations on an *entry* rather than an open file (rename, remove, stat
without following) take a `(&Dir, &Name)` pair, never a multi-component
path: the frontend resolves the parent first, then issues the single-name
operation against the parent handle. This keeps every mutating syscall a
`*at` call on a confined handle.

### 6.7 Limits of in-process enforcement

Rights are enforced by the library. In safe Rust that is a strong
bug-prevention boundary, but it is not a sandbox against hostile native code
in the same process. The kernels that offer capability-style restriction
will be supported within `filer-posix` (§17.1, D39), so that what the
library enforces can also be enforced by the OS:

- **Capsicum** (FreeBSD): `cap_rights_limit` attaches rights to a
  descriptor, so a `Dir` restricted to `READ` can be read-only to the
  kernel too, and `cap_enter` closes the ambient namespace entirely. The
  closest match to this design anywhere.
- **Landlock** (Linux 5.13+): a process-wide ruleset, derivable from the
  `Dir`s and rights a program holds.
- **`unveil` and `pledge`** (OpenBSD): path visibility and system-call
  restriction, also process-wide.

These live in `filer-posix` because each is a Unix kernel feature over the
same descriptors, as further restriction of handles that driver already
manages. A separate `filer-sandbox` crate abstracting over all three was
considered but is not committed to. Windows AppContainer and the macOS App
Sandbox are usually imposed by packaging rather than by a library, and are
out of scope.

## 7. Async model

### 7.1 Owned buffers

With io_uring, IOCP, and Windows IoRing, the kernel writes into memory after
submission. The `AsyncRead` shape, which borrows `&mut [u8]` across an
`.await`, is unsound the moment a future is dropped mid-flight. So every
operation takes its buffers by value and returns them, the shape that
tokio-uring, monoio, glommio, and compio converged on independently.

Because `filer-api` forbids `unsafe`, the buffer traits are **safe and
sealed**: only
types whose heap storage does not move when the owning value moves may
implement them, and the crate guarantees that by being the only implementer.

```rust
mod sealed { pub trait Sealed {} }

/// A buffer whose contents can be written to a file.
pub trait IoBuf: sealed::Sealed + Send + 'static {
    fn as_bytes(&self) -> &[u8];
}

/// A buffer that a read can fill, from `len()` up to `capacity()`.
pub trait IoBufMut: IoBuf {
    fn capacity(&self) -> usize;
    /// Zero-fills the spare capacity, lets `read` fill it, and appends the
    /// count it returns; on error the contents are unchanged.
    fn fill(&mut self, read: &mut dyn FnMut(&mut [u8]) -> Result<usize>)
        -> Result<usize>;
}

/// The result of an operation, returning its buffer whatever happened.
pub struct BufResult<T, B>(pub Result<T>, pub B);
```

`fill` replaced an earlier `spare_zeroed` plus `commit` pair (D33): with
`Vec<u8>`, safe code can only expose spare capacity by resizing into it,
after which `commit` cannot know where the old contents ended. One call
that resizes, reads, and truncates is safe, keeps the storage in place,
and is dyn-compatible for erased buffers (§16.1).

Implementations: `alloc::vec::Vec<u8>`, `alloc::boxed::Box<[u8]>` (write
only, since a boxed slice has no spare capacity),
`alloc::string::String` (write only), `&'static [u8]` (write only),
`bytes::Bytes` and `bytes::BytesMut` (feature `bytes`), `AlignedBuf` (for
direct I/O, see [§10](#10-metadata)), `PooledBuf` (driver-registered), and
`Slice<B>` (a sub-range view, like tokio-uring's `slice`).

Safe drivers zero-fill spare capacity before reading. That memset is
the price of keeping `filer-api` and most drivers free of `unsafe`; it is
cheap next to a syscall and invisible next to a device. A driver crate that
contains `unsafe` (`filer-uring`, D11) may read into uninitialized memory
through a crate-private extension trait in its `sys` module.
`core::io::BorrowedBuf` is still unstable (`core_io_borrowed_buf`) at 1.97
and at 1.99.1 (checked 2026-11-01); revisit if it stabilizes.

### 7.2 `'static` futures and cancellation

Futures returned by the frontend own their handles (refcounted) and buffers,
and are `'static`. That allows one cancellation story that every driver can
honor:

- An operation is **submitted on first poll** (Rust futures are lazy; an
  unpolled future does nothing).
- Once submitted, it **runs to completion**. Dropping the future detaches it:
  the result is discarded and buffers are released by the driver when the
  kernel is done with them.
- A thread pool cannot cancel a blocking call anyway, and for mutating
  operations "maybe it happened" is the worst possible semantics.
- Drivers may opportunistically cancel pure reads (for example with
  `IORING_OP_ASYNC_CANCEL`) as an optimization, never as a semantic.

Documentation must state plainly: dropping a mutating future does not undo
or prevent the mutation.

As built (2026-11-01, D36): the `'static` futures live at the driver
boundary (§16.3). Frontend methods (`Dir`, `File`) are plain `async fn`s
borrowing `&self`: with generic path arguments (`P: TryInto<RelPathBuf>`),
a returned `impl Future` must capture `P`, so `open_file("x")` would yield
a future tied to the string's lifetime unless every call were boxed. The
semantics above are unaffected, because each frontend method awaits a
driver future that is `'static` and runs to completion; to spawn frontend
work, move a cheap clone of the `Dir` or `File` into the task.

### 7.3 Granularity and chains

The default thread-pool executor makes granularity a design constraint:
`tokio::fs` pays one thread hop per call, which is why metadata-heavy work
on it is slow. The frontend therefore offers coarse operations that execute
in one hop:

- `read_to_end`, `write_atomic`, `read_to_string` (UTF-8 validated);
- directory streams that pull large `getdents64` batches per hop;
- a walker that a driver may run natively on a worker (the optional coarse
  `Op::Walk`), streaming batches back, with the generic `filer-api` walker
  as the fallback when a driver returns `Unsupported`;
- `copy_tree`, `remove_tree`.

And it offers **chains**: sequences of ops submitted together, where later
ops refer to earlier results through placeholder handles.

```rust
let chain = Chain::new()
    .push(Op::open_tmpfile(dir.handle(), perms))        // -> Pending(0)
    .push(Op::write(Pending(0), buf, 0))
    .push(Op::sync(Pending(0), Durability::Data))
    .push(Op::link(Pending(0), (dir.handle(), name)))
    .push(Op::sync(dir.handle(), Durability::Full));
let completions = driver.submit_chain(chain).await;
```

Semantics match `IOSQE_IO_LINK`: ops run in order; the first failure cancels
the rest, which complete with `ErrorKind::Canceled`. On io_uring a chain
maps to linked SQEs with direct descriptors standing in for placeholders; on
the thread pool it is one blocking task.

### 7.4 `Send` and local executors

Work-stealing runtimes need `Send` futures; thread-per-core io_uring
runtimes are naturally `!Send`. The core `Driver` trait requires `Send`
futures, because that is what the default driver and most users need. A
`LocalDriver` twin, generated with `trait-variant` or written by hand, can
follow once there is a concrete `!Send` driver. Return type notation
(`D: Driver<submit(..): Send>`), which could replace the twin, is still
experimental at 1.97 and at 1.99.1 (checked 2026-11-01).

### 7.5 Runtime independence

No public trait names a runtime. Drivers that run blocking calls take a
`Spawner`, defined in `filer-api`:

```rust
pub trait Spawner: Send + Sync + 'static {
    fn spawn_blocking(&self, job: alloc::boxed::Box<dyn FnOnce() + Send>);
}
```

Completion is signaled through a small internal oneshot channel in
`filer-api`, so no runtime's channel types leak. Implementations:

- `filer_api::DefaultPool` (under `std`): a small built-in pool with bounded
  threads, an idle timeout, and named threads (`filer-N`). It does no I/O
  itself, so it belongs in `filer-api`, shared by `filer-posix` and
  `filer-windows`.
- `filer_tokio::TokioSpawner`: delegates to `tokio::task::spawn_blocking`.
- Others (smol's `blocking`) as small adapter crates if requested.

As built (2026-11-01): `Spawner::spawn_blocking(Job)` with `Job =
Box<dyn FnOnce() + Send>`; under `std`, `run_on(Arc<dyn Spawner>, job)`
is a `'static` future that submits the job on first poll and resolves to
`Some(result)`, or `None` if the job panicked or was dropped unrun (the
oneshot is a `Mutex` slot whose sender closes it on drop). `DefaultPool`
starts named `filer-N` threads on demand up to a maximum (64 by default),
queues jobs beyond it, retires threads idle past a timeout (10 seconds),
contains panics so threads survive, and runs the queue inline only if no
thread can be started at all.

### 7.6 `no_std`

`core::future::Future`, `core::task`, and `alloc` are enough for the types,
the `Op` enum, the `Driver` trait, the frontend, the generic algorithms,
`filer-heap`, the routing drivers, and most middleware. Only OS drivers,
`AmbientAuthority` constructors, `DefaultPool`, and `std::io::Error`
interop require `std`.

### 7.7 Locks inside Filer.rs

Two different needs, two different tools (D21):

- **State locks that are never held across an `.await`.** `filer-heap`'s
  inode table, driver caches (features per mount), handle tables. These
  guard short, synchronous critical sections, for which a synchronous mutex
  is both correct and fastest: `std::sync::Mutex` under `std`,
  `spin::Mutex` without it. Tokio's own documentation recommends the
  standard mutex for exactly this case.
- **Locks whose guard must live across an `.await`.** In-process
  serialization of transactions on one root in `filer-tx`, quota
  reservations in `filer-quota`, per-path ordering in a future middleware.
  These need an async-aware lock, and it must be runtime-agnostic and
  `no_std`-capable: `async-lock` (smol-rs; optional `std` feature; MSRV
  1.85; checked 2026-10-31).

Tokio's `tokio::sync::Mutex` is therefore not used in any crate except
`filer-tokio`: it would tie core crates to one runtime, it is not
`no_std`, and for short critical sections it is slower than a synchronous
lock. A lock is never a correctness mechanism *between processes*; that is
what OS advisory locks are for (§15.4).

## 8. Handles, files, and opening

### 8.1 File typestate

```rust
pub struct File<M: Mode = ReadOnly, D: Driver = DynDriver> { /* ... */ }

pub enum ReadOnly {}
pub enum WriteOnly {}
pub enum ReadWrite {}
pub enum Append {}

impl<M: Readable, D: Driver> File<M, D> {
    pub async fn read_at<B: IoBufMut>(&self, buf: B, off: u64)
        -> BufResult<usize, B>;
    pub async fn read_exact_at<B: IoBufMut>(&self, buf: B, off: u64)
        -> BufResult<(), B>;
}

impl<M: Writable, D: Driver> File<M, D> {
    pub async fn write_all_at<B: IoBuf>(&self, buf: B, off: u64)
        -> BufResult<(), B>;
    pub async fn set_len(&self, len: u64) -> Result<()>;
    pub async fn allocate(&self, range: core::ops::Range<u64>) -> Result<()>;
    pub async fn punch_hole(&self, range: core::ops::Range<u64>)
        -> Result<()>;
}

impl<D: Driver> File<Append, D> {
    pub async fn append<B: IoBuf>(&self, buf: B) -> BufResult<(), B>;
}

impl<M: Mode, D: Driver> File<M, D> {
    pub async fn metadata(&self, want: MetaMask) -> Result<Metadata>;
    pub async fn sync(&self, level: Durability) -> Result<()>;
    /// Surfaces deferred errors (NFS, quotas). Drop closes in the
    /// background and reports failures through the driver's error hook.
    pub async fn close(self) -> Result<()>;
}
```

### 8.2 Positional I/O only

There is no implicit cursor. A shared seek position is hidden mutable
state: it makes handles non-shareable and means nothing to completion-based
I/O, which always passes an offset. Without it, `File` is `Sync` and can
serve concurrent reads. A cursor is a trivial adapter (`Cursor<File<M>>`),
and that adapter is where `futures-io` / `tokio::io` `AsyncRead` and
`AsyncWrite` compatibility lives, with an internal buffer copy
(`filer-api`'s `futures-io` feature, and `filer-tokio`).

`Append` is a separate mode because `O_APPEND` writes ignore offsets.

### 8.3 Opening

```rust
let f = dir
    .open(rel!("data.bin"), Open::read_write().create_new(Perms::mode(0o600)))
    .await?;                                  // File<ReadWrite>
```

`Open<M>` is a builder whose type parameter fixes the resulting mode:
`Open::read()`, `Open::write()`, `Open::read_write()`, `Open::append()`.
Creation is `create_new(perms)` (fail if exists), `create(perms)` (open or
create), or the default (must exist). `truncate()` is only available on
writable modes.

Defaults that took the industry years to learn:

- **Close-on-exec always.** Never configurable.
- **Regular files only by default.** Opening a FIFO, socket, device, or tty
  through `open` fails with `ErrorKind::NotRegularFile` unless the builder
  says `.kind(Kinds::ANY)` or similar. Implementation: open with
  `O_NONBLOCK` so a FIFO cannot hang the open, `fstat`, reject or clear the
  flag. Tools that open "a file" and get a FIFO or `/dev/zero` hang or run
  forever.
- **Windows sharing.** Open with `FILE_SHARE_READ | FILE_SHARE_WRITE |
  FILE_SHARE_DELETE` and use POSIX delete and rename semantics where
  available, so rename-over and unlink behave as on Unix while the file is
  open elsewhere.
- **Permissions and initial xattrs at creation.** They are applied before the
  name becomes visible (via `O_TMPFILE` or a hidden temporary), so a secrets
  file is never briefly world-readable. See [§14](#14-atomic-replacement).
- **No `O_NOATIME` by default**, but available, since it fails with `EPERM`
  for files the caller does not own.
- **Direct I/O** (`.direct()`) requires `AlignedBuf` buffers sized from
  `Metadata::dio_align`; misaligned buffers fail before submission.

### 8.4 Closing and handle lifetime

Driver handles are refcounted (`alloc::sync::Arc`). The OS descriptor is
closed when the last reference drops. Because Rust has no async `Drop`, a
dropped handle is passed to the driver's background closer rather than
closed inline, since `close` can block (NFS) and can fail. Failures surface
through an optional driver error hook and `tracing`.

`close(self)` is explicit and fallible because on NFS and some quota setups
the write error only appears at close.

### 8.5 Directory handles

`Dir` carries `(driver, handle, rights, resolve)`. Directory handles are
opened with `O_PATH | O_DIRECTORY` where possible (cheap, cannot read file
contents) and upgraded internally when listing requires `O_RDONLY`. `Dir`
is `Clone` (refcount bump) and `Send + Sync`.

## 9. Directory enumeration

```rust
pub struct Entry {
    pub name: EntryName,
    pub kind: Option<FileKind>, // d_type; None when DT_UNKNOWN
    pub id: Option<FileId>,     // d_ino where meaningful
}

impl<D: Driver> Dir<D> {
    pub fn entries(&self) -> ReadDir<D>; // impl Stream<Item = Result<Entry>>
    pub async fn names(&self) -> Result<Vec<EntryName>>;
}
```

- Streams, and a `Vec` only of names (D55): `names` collects a listing's
  names for when they are all a caller needs, and an `EntryName` compares
  equal to a string of its text, so a listing checks against literals.
  Order is unspecified in both; sorting is the caller's job.
- Each driver hop fetches a large batch (`getdents64` with a 32 KiB buffer
  or more); the stream yields from the batch.
- `kind` may be `None` legitimately (some filesystems return `DT_UNKNOWN`).
  The walker falls back to a no-follow `stat` in that case.
- Concurrent modification follows POSIX: entries added or removed during
  iteration may or may not appear; others appear exactly once.
- `.` and `..` are never yielded.
- The stream implements `futures_core::Stream` (feature `futures`; the trait
  is tiny and `no_std`). Without that feature it offers an inherent
  `async fn next(&mut self) -> Option<Result<Entry>>`.

## 10. Metadata

```rust
bitflags::bitflags! {
    pub struct MetaMask: u32 {
        const KIND = 1 << 0;  const PERMS = 1 << 1;  const LEN = 1 << 2;
        const ALLOCATED = 1 << 3;  const MTIME = 1 << 4;
        const CTIME = 1 << 5;  const BTIME = 1 << 6;  const ATIME = 1 << 7;
        const ID = 1 << 8;  const NLINK = 1 << 9;  const OWNER = 1 << 10;
        const ATTRS = 1 << 11;  const DIO_ALIGN = 1 << 12;
        const CHANGE_TOKEN = 1 << 13;  const ATOMIC_WRITE = 1 << 14;
        const MOUNT = 1 << 15;
    }
}

#[non_exhaustive]
pub struct Metadata {           // None = unsupported or not requested
    pub kind: Option<FileKind>,
    pub perms: Option<Permissions>,
    pub len: Option<u64>,
    pub allocated: Option<u64>,
    pub modified: Option<Timestamp>,
    pub changed: Option<Timestamp>,   // ctime; not settable by users
    pub created: Option<Timestamp>,
    pub accessed: Option<Timestamp>,
    pub id: Option<FileId>,
    pub nlink: Option<u64>,
    pub owner: Option<Owner>,         // numeric uid/gid or a SID, opaque
    pub attrs: Option<FileAttrs>,
    pub dio_align: Option<DioAlign>,
    pub change_token: Option<ChangeToken>,
    pub atomic_write: Option<AtomicWriteUnit>,
    pub mount: Option<MountId>,
}

pub struct Timestamp {
    pub secs: i64,
    pub nanos: u32,
    pub granularity: Option<core::time::Duration>,
}
```

As built (2026-11-01), only the fields a read-only driver needs exist:
`kind`, `len`, the four timestamps, `id`, and `attrs`, with `MetaMask` bits
at the values above (others are added with their operations and never renumbered).
Drivers fill `Metadata::default()` field by field, since the struct is
`#[non_exhaustive]`. `Timestamp` keeps its fields private behind a
constructor that rejects `nanos` of a full second or more, and converts to
`SystemTime` under `std`.

Rationale:

- The mask mirrors `statx`. Metadata is not uniformly cheap: birth time and
  attributes cost differently on different filesystems, and on NFS every
  unneeded field is a round trip or a stale cache. Absent fields are `None`,
  never zero.
- `FileId` is opaque: a 64-bit volume id plus a 128-bit file id, enough
  for `(st_dev, st_ino)` and for Windows `FILE_ID_INFO` (a 64-bit volume
  serial plus ReFS's 128-bit file ids). It is not a perfect identity test:
  btrfs subvolumes, overlayfs, and NFS inode reuse break it.
  `same_file(&a, &b)` compares open handles, which is the robust check.
- `Timestamp::granularity` is reported because FAT has 2-second mtimes and
  change-detection caches are fooled by edits within one tick (git's
  "racily clean" index logic exists for this reason).
- `ChangeToken` is the best available "has this changed" signal: the NFSv4
  change attribute, an NTFS USN, a kernel change cookie if exposed
  (**VERIFY** `STATX_CHANGE_COOKIE`), else a tuple of id, size, mtime, and
  ctime. Tokens are comparable only for equality and only from the same
  driver. They underpin optimistic preconditions in
  [transactions](#15-transactions) and `Replace::IfUnchanged`.
- `DioAlign` comes from `STATX_DIOALIGN` (Linux 6.1+). `AtomicWriteUnit`
  comes from `STATX_WRITE_ATOMIC` (Linux 6.11+; ext4 and XFS from 6.13,
  currently one filesystem block).
- `MountId` uses `STATX_MNT_ID_UNIQUE` where available. It is how
  transactions check they stay within one filesystem.

`Permissions` is minimal and portable (`readonly`, and Unix `mode` behind an
accessor that returns `Option<u32>`), with platform extension traits for
the rest. Setting permissions is a `META` operation on a handle.

## 11. Extended attributes and file flags

### 11.1 Extended attributes

```rust
pub enum XattrNs { User, Trusted, Security, System }

pub struct XattrName { ns: XattrNs, name: alloc::boxed::Box<str> }

impl<M: Mode, D: Driver> File<M, D> {          // likewise on Dir
    pub async fn xattr<B: IoBufMut>(&self, name: &XattrName, buf: B)
        -> BufResult<Option<usize>, B>;
    pub async fn set_xattr<B: IoBuf>(&self, name: &XattrName, value: B,
        how: SetHow) -> BufResult<(), B>;      // Create | Replace | Upsert
    pub async fn xattr_names(&self) -> Result<alloc::vec::Vec<XattrEntry>>;
    pub async fn remove_xattr(&self, name: &XattrName) -> Result<()>;
}
```

- Handle-based (`fgetxattr` and friends) to avoid TOCTOU.
- Names are UTF-8; listing yields `Utf8` or `Foreign` entries exactly like
  directory names.
- Namespaces map natively on Linux and FreeBSD. macOS has one flat
  namespace, mapped to `User`; other namespaces are `Unsupported`. A Windows
  driver may map `User` onto NTFS alternate data streams, but must say so in
  `Features::xattr`.
- `Features::xattr` reports support, maximum value size (effectively one
  block on ext4, 64 KiB on XFS and Linux's `XATTR_SIZE_MAX`), and whether
  symlinks can carry them.
- As built (2026-11-01), the read side: `Op::Xattr(XattrOp)` with `Get {
  handle, name, buf }` (completing with `Output::Xattr(Option<u64>)`, the
  value's whole length, having copied what fits) and `List { handle }`
  (`Output::XattrNames`). `XattrName` parses `ns.name` (not empty, no NUL;
  length limits are the filesystem's); `XattrEntry` is `Utf8` or `Foreign
  { ns, name }`, minted by `DriverKey::xattr_entry`. `File` and `Dir`
  have `xattr(name)` (the whole value, retried if it grows),
  `xattr_into(name, buf)`, and `xattr_names()`, needing `READ`.
  `filer-heap` and `filer-ntfs` read them; other drivers answer
  `Unsupported`.
- Lesson recorded in rustdoc: xattrs are the first thing lost in transit.
  `cp` without `-a`, `tar` without `--xattrs`, `rsync` without `-X`, git,
  most cloud sync, and many container tools drop them. Treat them as caches
  and annotations, never as the sole copy of data. Copy operations report
  which requested attributes were dropped
  ([§12](#12-copies-clones-and-sparse-files)).

POSIX ACLs, NFSv4 ACLs, and Windows security descriptors are out of scope
for the core; they may become extension traits later.

### 11.2 File flags

`FileAttrs` exposes, read-only through metadata and settable through
`set_flags` where supported (as built, read-only, in `Metadata::attrs`,
with Windows' `hidden`, `system`, and `archive` too; read-only stays a
permission):

- `immutable`, `append_only` (Linux `chattr`, BSD `chflags`);
- `nocow` (btrfs `+C`; only effective on empty files, so it belongs in the
  `Open` builder's creation options as well);
- `compressed`, `encrypted`, `verity` (fs-verity), `dax`;
- `sparse`, `offline`, `dataless` / recall-on-access (cloud placeholders,
  see [§21.6](#216-cloud-placeholder-files)).

## 12. Copies, clones, and sparse files

```rust
pub enum CloneMode {
    /// Share extents or fail with `Unsupported`.
    Require,
    /// Share extents if possible, copy otherwise, and report which.
    Prefer,
    /// Always duplicate data (independent extents).
    Never,
}

#[non_exhaustive]
pub struct CopyOpts {
    pub clone: CloneMode,
    pub preserve: Preserve,  // PERMS | TIMES | XATTRS | ACLS | OWNER | FLAGS
    pub replace: Replace,    // Never | Always | IfUnchanged(ChangeToken)
    pub sparse: bool,        // preserve holes when duplicating
    pub durability: Durability,
}

#[non_exhaustive]
pub struct CopyReport {
    pub bytes: u64,
    pub cloned: u64,         // bytes shared rather than duplicated
    pub offloaded: bool,     // server-side copy (NFS 4.2, SMB)
    pub dropped: Preserve,   // requested but not preservable here
}

impl<D: Driver> Dir<D> {
    pub async fn copy_file(&self, from: &RelPath, to: (&Dir<D>, &RelPath),
        opts: CopyOpts) -> Result<CopyReport>;
}

impl<M: Writable, D: Driver> File<M, D> {
    pub async fn clone_range_from<S: Readable>(&self, src: &File<S, D>,
        src_off: u64, len: u64, dst_off: u64) -> Result<()>;
    pub async fn dedupe_range_with<S: Readable>(&self, src: &File<S, D>,
        src_off: u64, len: u64, dst_off: u64) -> Result<u64>;
}

impl<M: Readable, D: Driver> File<M, D> {
    pub fn extents(&self) -> Extents<D>; // stream of Data | Hole ranges
}
```

- **No silent fallback.** "Instant snapshot" quietly becoming "copy 200 GB"
  is the trap. `Prefer` reports via `cloned`; `Require` fails.
- `Never` exists because sometimes independent extents are the point: a
  backup that must survive corruption of the original's blocks, or avoiding
  shared-extent fragmentation.
- `punch_hole` is also how a filesystem image tells its host which of its
  ranges are free (§17.13, D51), so it should land with or before write
  support for any filesystem driver; its report must say whether holes
  read as zeros.
- Mapping: `FICLONE` / `FICLONERANGE` and `copy_file_range` on Linux (btrfs,
  XFS with reflink, bcachefs, NFS 4.2; not ext4); `clonefileat` on APFS;
  `FSCTL_DUPLICATE_EXTENTS_TO_FILE` on ReFS and Dev Drive. `FIDEDUPERANGE`
  for dedupe.
- Range clones have block-alignment requirements; `Features::clone_align`
  reports them so callers do not discover them as `EINVAL`.
- Clones and server-side copies only work within one filesystem (or one
  server). `Features::volume` lets callers check up front.
- `extents()` uses `SEEK_DATA` / `SEEK_HOLE` (Windows:
  `FSCTL_QUERY_ALLOCATED_RANGES`). `allocate` uses `fallocate` /
  `F_PREALLOCATE` / `SetFileInformationByHandle(FileAllocationInfo)`.
- `allocate` matters beyond performance: reserving space before a commit is
  how `ENOSPC` moves from "halfway through" to "before anything happened".

## 13. Durability

```rust
pub enum Durability {
    /// No sync. Survives a process crash, not power loss.
    None,
    /// Contents and size on stable storage (fdatasync).
    Data,
    /// Contents, metadata, and the directory entries that name the file
    /// (fsync on the file and on each parent whose entries changed).
    Full,
}
```

- **Directory sync is part of `Full`.** A newly created or renamed file is not
  durable until its parent directory is synced. This is the step hand-rolled
  code forgets.
- **macOS:** `fsync` does not flush the drive's write cache; `Full` and
  `Data` map to `F_FULLFSYNC`. `F_BARRIERFSYNC` (ordering only) is exposed as
  a platform extension, not as a portable level.
- **Windows:** `FlushFileBuffers`. Directory flushing is not generally
  available; `Features::durability` reports what is real.
- **Linux:** `fdatasync` / `fsync`. `sync_file_range` is never used for
  durability.
- `syncfs` is exposed on `Dir` as `sync_filesystem()` for bulk writers.

### 13.1 Sync failure poisons the handle

In 2018 PostgreSQL discovered ("fsyncgate") that after a failed `fsync`,
Linux could mark dirty pages clean and drop them, so a retried `fsync`
reported success for data that was gone. Linux 4.13's `errseq_t` improved
error reporting across descriptors but did not change the core lesson: after
a sync failure, data written since the last successful sync is in an unknown
state, and retrying does not help.

So a `File` **poisons itself** on sync failure: every later `sync` on that
handle (and on clones of it) returns the original error, with
`ErrorKind::Poisoned` wrapping the cause. The error's documentation says
"recover from your own copy or log", not "try again". `filer-heap`
models the kernel behavior (drop pages, report clean) so this path is
tested.

## 14. Atomic replacement

The ALICE study (Pillai et al., OSDI 2014) found crash-consistency bugs in
nearly every application it examined, mostly in hand-rolled versions of
this primitive. It is therefore first-class:

```rust
let mut w = dir.atomic_writer(rel!("state.json"), AtomicOpts {
    durability: Durability::Full,
    perms: Perms::mode(0o600),
    replace: Replace::Always,      // or Never, or IfUnchanged(token)
}).await?;
w.write_all(buf).await?;
w.set_xattr(&name, digest, SetHow::Create).await?; // before visibility
let receipt = w.commit().await?;                    // drop = discard
```

Also offered as one call: `dir.write_atomic(path, bytes, opts)`, which runs
as a single chain (one hop on the thread pool).

### 14.1 Algorithm

Linux:

1. `openat(parent, ".", O_TMPFILE | O_RDWR | O_CLOEXEC, mode)`. On
   `EOPNOTSUPP`, `EISDIR`, or `ENOENT` (filesystem without `O_TMPFILE`),
   fall back to a hidden named temporary `.{name}.{random}.filer-tmp` opened
   with `O_CREAT | O_EXCL`.
2. Write contents; apply permissions, xattrs, and flags through the fd.
3. Sync per `durability` (`fdatasync` suffices before the link).
4. Make it visible:
   - `Replace::Never` with `O_TMPFILE`: `linkat` straight to the target
     name. `linkat` fails with `EEXIST` if the name exists, so this is an
     atomic create-if-absent with contents, no rename needed.
   - `Replace::Always` with `O_TMPFILE`: `linkat` cannot replace, so link to
     a unique hidden name, then `renameat` over the target.
   - Named temporary: `renameat`, or `renameat2(RENAME_NOREPLACE)` for
     `Replace::Never`.
   - `linkat` of an `O_TMPFILE` fd uses `AT_EMPTY_PATH`, which historically
     required `CAP_DAC_READ_SEARCH`; the documented unprivileged alternative
     is linking through `/proc/self/fd/N` with `AT_SYMLINK_FOLLOW`. This
     internal use of a magic link is acceptable because the path is
     constructed by the driver, never by a caller. **VERIFY** current kernel
     requirements and pick the order of attempts accordingly.
5. `Full`: `fsync` the parent directory.

macOS: named temporary (no `O_TMPFILE`), `renameatx_np` with `RENAME_EXCL`
for `Replace::Never` (through `rustix`'s `renameat_with`; **VERIFY** the
`NOREPLACE` mapping), and `F_FULLFSYNC`.

Windows: named temporary in the same directory (hidden attribute),
`FlushFileBuffers`, then `SetFileInformationByHandle` with
`FileRenameInfoEx` and `FILE_RENAME_FLAG_POSIX_SEMANTICS` plus
`FILE_RENAME_FLAG_REPLACE_IF_EXISTS` (or without it for `Replace::Never`).
`filer-windows` issues these calls directly (§17.2); **VERIFY** the minimum
Windows version for `FileRenameInfoEx` with POSIX semantics.

### 14.2 `Replace::IfUnchanged`

Checked immediately before the visibility step by comparing the target's
`ChangeToken` with the caller's. This narrows the race window but does not
close it unless the caller also holds a lock, and the rustdoc must say so.
A stricter variant could exchange first (`RENAME_EXCHANGE`), verify the
displaced file's identity, and exchange back on mismatch; that is atomic
against crashes but briefly exposes the new content to observers, so it is
not the default.

### 14.3 Cleanup

Named temporaries left behind by a crash carry a recognizable suffix.
`Dir::sweep_temporaries(older_than)` removes them; nothing removes them
implicitly, because the library never performs surprise writes.

## 15. Transactions

No mainstream POSIX filesystem exposes multi-file transactions. Windows'
TxF is deprecated, and Microsoft steers developers away from it. Research
systems like TxFS (USENIX ATC 2018) showed a clean kernel design but were
never upstreamed. What did land in Linux is narrower: `RWF_ATOMIC` untorn
writes (6.11 for block devices; ext4 and XFS from 6.13, one filesystem block
at a time). So Filer does not pretend. It offers a transaction API whose
commit picks the strongest strategy available for the staged changes and
reports what it delivered.

Transactions live in `filer-tx`, which stays `publish = false` until the
crash-property suite ([§24](#24-testing-strategy)) passes for every
strategy (D16).

```rust
#[non_exhaustive]
pub enum Atomicity {
    /// Observers see all changes or none; a crash leaves all or none.
    Atomic,
    /// A crash leaves all or none after recovery; concurrent observers may
    /// see intermediate states while the commit runs.
    CrashAtomic,
}

let tx = filer_tx::Transaction::begin(&app, TxOpts {
    durability: Durability::Full,
    require: Atomicity::CrashAtomic,   // fail at begin if not achievable
    lock: Lock::Advisory,              // or Lock::None
}).await?;

tx.write(rel!("config/app.toml"), bytes).expect_unchanged(token).await?;
tx.remove(rel!("config/legacy.toml")).await?;
tx.rename(rel!("data/next"), rel!("data/current")).await?;
tx.copy_from(&cache, rel!("blob"), rel!("assets/blob"), CloneMode::Prefer)
    .await?;

let receipt = tx.commit().await?;      // receipt.atomicity() says what
receipt.rollback().await?;             // best effort, from pre-images
// or: receipt.finalize().await?;      // discard pre-images
```

### 15.1 Strategies, strongest first

**Single entry.** One write or rename is temp-plus-rename
([§14](#14-atomic-replacement)): `Atomic`.

**Subtree swap.** When every change falls under one directory `S`:

1. Build a shadow `S'` beside `S`: unchanged files reflinked (or hardlinked
   when the caller declares committed files immutable, as Nix and OSTree
   do); changed files written fresh; directories recreated.
2. Sync everything.
3. Swap with one `renameat2(RENAME_EXCHANGE)` (`RENAME_SWAP` on macOS).
4. The old tree now sits at the shadow name; keep it as the pre-image until
   `finalize`.

That is `Atomic`. Where exchange is unavailable, the portable form is a
**symlink flip**: `current -> gen-41`; write `gen-42`; atomically rename a
new symlink over `current`. Also `Atomic` for readers who resolve through
the symlink.

Readers that opened the old root keep a handle to it and see a consistent
old generation until it is garbage-collected, which must tolerate them. Cost
scales with subtree size, which makes this a data-layout lesson: put things
that must change together in one swappable directory. A btrfs-aware driver
may replace step 1 with an O(1) subvolume snapshot.

**Journaled.** Arbitrary changes within one filesystem:

1. Stage all new content as files under `.filer-tx/<id>/staged/`.
2. Write intent records (ops, preconditions, where pre-images go); sync.
3. Write and sync a commit record. **That sync is the commit point.**
4. Apply each op. Each is individually atomic and idempotent.
5. Write a done record; pre-images stay until `finalize`.

Recovery rolls forward anything with a valid commit record and discards
anything without. That is `CrashAtomic`, not `Atomic`.

### 15.2 Journal format

A transaction directory `.filer-tx/<id>/` (the base name is configurable;
it must be on the same filesystem as the transaction root) contains
`staged/`, `pre/`, and `journal`. The journal is append-only, one record per
line, each line `len:payload:crc32c`, so a torn tail is detectable. Records:

- `begin` with format version, transaction id, root `FileId`, mount id, and
  durability;
- `op` with sequence number, kind, arguments (names as escaped UTF-8 or
  foreign-name tokens valid only for this journal), precondition token, and
  pre-image location;
- `commit` with the count and a checksum over all `op` records;
- `done`.

The format is private, versioned, and documented in the module rustdoc. No
`serde` dependency is required.

### 15.3 Idempotent application

- **write**: rename `staged/<n>` over the target. If `staged/<n>` is gone and
  the target's `FileId` equals the recorded staged id, the step already
  happened.
- **remove**: rename the target into `pre/<n>` instead of unlinking. That
  preserves the pre-image for rollback and makes the step idempotent.
- **overwrite**: the displaced target goes to `pre/<n>` via a hardlink or
  reflink taken before the rename.
- **rename**: check source and destination identities against the journal.
- **mkdir**, **set_perms**, **set_xattr**: compare current state with the
  intended state.

### 15.4 Rules for every strategy

- **One filesystem.** Checked at `begin` by mount id. Cross-filesystem
  rename is `EXDEV` and would silently turn a commit into copy-plus-delete.
- **Work before the commit point.** Staging, cloning, syncing, and space
  reservation (`allocate`) all happen first, so the post-commit phase is
  renames, which rarely fail.
- **Rollback.** Before commit it is free: delete staging. After commit it is
  best effort, from pre-images kept until `finalize`.
- **Isolation is opt-in.** `Lock::Advisory` takes an OS lock on
  `.filer-tx/lock` for other processes (`flock` on Unix per D27, OFD locks
  on Linux once available, `LockFileEx` on Windows; never a classic POSIX
  `fcntl` lock, which is released when the process closes *any* descriptor
  for the file) and an `async-lock` mutex for other tasks in the same
  process (§7.7). Optimistic preconditions (`expect_unchanged`) abort
  before the commit point.
- **Recovery is explicit.** `filer_tx::recover(&dir).await` returns a
  report. A library must not perform surprise writes when a handle is
  opened; the application knows when recovery is safe.

## 16. The driver boundary

A trait with thirty async methods is awkward to make dyn-compatible,
painful to wrap (an overlay or tracing layer must forward all thirty), and
breaks every implementation when an operation is added. At the boundary,
operations are data.

### 16.1 Operations

```rust
#[non_exhaustive]
pub enum Op {
    Open { at: Handle, path: RelPathBuf, how: OpenHow },
    OpenTmpfile { dir: Handle, how: OpenHow },
    Close { handle: Handle },
    Read { file: Handle, buf: ErasedBufMut, off: u64 },
    Write { file: Handle, buf: ErasedBuf, off: u64 },
    Stat { target: Target, mask: MetaMask },
    StatFs { at: Handle },
    SetLen { file: Handle, len: u64 },
    Allocate { file: Handle, range: core::ops::Range<u64> },
    PunchHole { file: Handle, range: core::ops::Range<u64> },
    Extents { file: Handle, from: u64, max: usize },
    Sync { target: Handle, level: Durability },
    ReadDir { dir: Handle, cookie: Option<DirCookie>, max: usize },
    Mkdir { at: Handle, name: NameBuf, perms: Perms },
    Symlink { at: Handle, name: NameBuf, target: alloc::string::String },
    ReadLink { at: Handle, name: EntryName },
    Link { from: Target, to: (Handle, NameBuf) },
    Rename { from: (Handle, EntryName), to: (Handle, NameBuf),
             flags: RenameFlags },               // NoReplace | Exchange
    Remove { at: Handle, name: EntryName, dir: bool },
    SetPerms { target: Target, perms: Perms },
    SetTimes { target: Target, times: SetTimes },
    SetFlags { target: Target, flags: FileAttrs, mask: FileAttrs },
    Xattr(XattrOp),
    Clone { src: Handle, dst: Handle, range: Option<CloneRange> },
    Dedupe { src: Handle, dst: Handle, range: CloneRange },
    CopyRange { src: Handle, dst: Handle, range: CloneRange },
    Lock { file: Handle, kind: LockKind, wait: bool },
    Unlock { file: Handle },
    Features { at: Handle },
    Walk { dir: Handle, opts: WalkOpts, batch: usize }, // optional, coarse
}

pub enum Target {
    Handle(Handle),
    Entry { at: Handle, name: EntryName, follow: bool },
}
```

Completions mirror ops: `Completion { result: Result<Output>, bufs:
ReturnedBufs }`, where `Output` is an enum with one variant per op family.
The frontend matches exhaustively on what it submitted and treats a
mismatched variant as a driver bug (`ErrorKind::Other` with a diagnostic,
plus a `debug_assert!`).

Erased buffers (`ErasedBuf`, `ErasedBufMut`) box the caller's buffer as
`alloc::boxed::Box<dyn core::any::Any + Send>` behind the sealed traits and
are downcast back to the caller's type on completion. That costs one
allocation per I/O operation, negligible next to a syscall; a typed fast
path can be added later for the common `Vec<u8>` case if profiling says so.

As built (2026-11-01), the read subset exists: `Open`, `Close`, `Read`,
`Stat`, `ReadDir`, and `ReadLink`, with `Target`, `OpenHow` (`kind` plus
`Resolve`; read-only for now), `Entry`, `DirCookie` (an opaque `u64`, such
as a `d_off`), `Output`, and `Completion { result, buf }`, which returns the
read buffer whatever happened. `ErasedBufMut` boxes a `dyn IoBufMut`;
`IoBuf` extends `core::any::Any`, so trait upcasting (stable since 1.86)
recovers the original type, and the erased buffer itself implements
`IoBufMut` so drivers fill it without knowing its type. `Op::kind()`
returns its `OpKind`. `Output::Done` is the unit result (`Close`).
`Op::unsupported()` fails an operation with `Unsupported` and returns its
buffer: drivers answer unknown operations with it, so that an operation
added later never loses a caller's buffer.

### 16.2 Handles

```rust
#[derive(Clone)]
pub struct Handle(alloc::sync::Arc<HandleInner>);

struct HandleInner {
    driver: DriverId,                 // which driver instance minted it
    kind: HandleKind,                 // Dir | File | Path
    slot: HandleSlot,                 // driver-private
}

enum HandleSlot {
    Pending(u32),                     // chain placeholder
    Payload(alloc::boxed::Box<dyn core::any::Any + Send + Sync>),
}
```

Drivers downcast the payload (an `OwnedFd`, a heap inode id, a mount
index) safely with `Any`. A driver receiving a handle minted by a different
`DriverId` returns `ErrorKind::CrossDriver` (routing drivers translate
handles explicitly). Dropping the last reference sends the payload to the
driver's background closer ([§8.4](#84-closing-and-handle-lifetime)).

Drivers that cooperate share a payload type: `filer-uring` mints handles
with `filer-posix`'s public payload type, so routing between them needs no
translation. Since handles are minted and read through a `DriverKey`
(below), cooperating drivers must also share one key, for example by
`Posix` accepting an `Arc<DriverKey>` from the `Uring` that wraps it.

As built (2026-11-01): a handle is `{ driver: DriverId, kind: HandleKind,
payload: Arc<dyn Any + Send + Sync> }`, one allocation instead of two.
Like foreign names (D32), handles are minted only by `DriverKey::handle`,
and `DriverKey::payload::<T>(&handle)` returns `None` for handles from
another driver, which drivers report as `CrossDriver`. `HandleKind` has
only `Dir` and `File` so far. Deferred: the `Pending` placeholder (with
chains, DRV-03) and the background closer (until then the payload drops,
and closes, wherever the last clone is dropped).

### 16.3 The trait

```rust
pub trait Driver: Clone + Send + Sync + 'static {
    /// Stable identity used to validate handles and foreign names.
    fn id(&self) -> DriverId;

    fn submit(&self, op: Op)
        -> impl core::future::Future<Output = Completion> + Send + use<Self>;

    /// Ops run in order; the first failure cancels the rest.
    fn submit_chain(&self, chain: Chain)
        -> impl core::future::Future<Output = alloc::vec::Vec<Completion>>
            + Send + use<Self>
    {
        sequential_chain(self.clone(), chain) // default implementation
    }
}

/// Type-erased driver: boxed futures, blanket impl for every `Driver`.
#[derive(Clone)]
pub struct DynDriver(alloc::sync::Arc<dyn ErasedDriver>);
```

The frontend is generic, `Dir<D: Driver = DynDriver>`: static dispatch when
the driver is known, one boxed future per op when it is not.

`DynDriver` as built (2026-11-01) wraps `Arc<dyn ErasedDriver>`, a private
object-safe trait with a blanket impl for every `Driver` that boxes the
future. Its methods are named `erased_*` because the blanket impl also
covers `DynDriver`. `DynDriver::new` of a `DynDriver` returns a clone
rather than wrapping twice, and the wrapper keeps the inner driver's id.

Language features available at MSRV 1.97 (probed 2026-11-01 on
1.97.0 and 1.99.1):

- **Usable:** `impl Future + Send` in trait return position, including
  default method bodies (as `submit_chain` above); precise capturing in
  traits, which the futures need (below); trait upcasting (`&dyn Sub` to
  `&dyn Any`), useful for erased drivers and payloads; `core::error::Error`;
  `panic!` and `assert!` in `const fn`, which the `rel!` and `name!`
  validators rely on.
- **Not usable:** return type notation; `impl Trait` in associated types
  (so a `type Future = impl Future` per driver is out, and nameable future
  types need boxing or a hand-written future); `core::io::BorrowedBuf`.

As built (2026-11-01, without chains yet): a `+ 'static` bound alone is
not enough. In edition 2024 the returned future implicitly captures the
`&self` lifetime, so `let f = d.submit(op); drop(d);` fails to borrow-check
(`E0505`) even though the bound holds. The trait therefore declares
`+ Send + use<Self>`, and implementations write `+ Send + use<>` (writing
`use<Self>` in an `impl` is `E0799`, since there `Self` is an alias). The
future then borrows nothing and outlives the driver; a test checks this
through a generic `D: Driver`.

### 16.4 Features, per directory

```rust
#[non_exhaustive]
pub struct Features {
    pub enforcement: Enforcement,         // Kernel | Userspace | Virtual
    pub naming: NamingRules,
    pub volume: VolumeId,                 // same volume = can clone/rename
    pub reflink: bool,
    pub clone_align: Option<u32>,
    pub server_side_copy: bool,
    pub rename_noreplace: bool,
    pub rename_exchange: bool,
    pub tmpfile: bool,
    pub hardlinks: bool,
    pub symlinks: bool,
    pub xattr: XattrSupport,
    pub flags: FileAttrs,                 // settable flags
    pub sparse: bool,
    pub durability: DurabilitySupport,    // which levels are real
    pub atomic_write: Option<AtomicWriteUnit>,
    pub locks: LockSupport,
    pub remote: Option<bool>,
    pub placeholders: bool,               // cloud dataless files possible
}
```

Features are queried per directory handle (`Op::Features { at }`), because
a mount point or a casefolded directory changes the answers mid-tree.
Drivers cache answers per mount id. Everything not supported returns
`ErrorKind::Unsupported` at use; nothing is emulated unless the caller
opted into a fallback.

### 16.5 Per-operation fallback

io_uring has no upstream `getdents` operation and does not cover the clone
ioctls or `copy_file_range` (**VERIFY** at implementation time); Windows
IoRing covers only a small set of operations. So `filer-uring` composes a
`Posix` instance and routes what io_uring cannot do to it (§17.3). The
same mechanism handles a specific filesystem returning `EOPNOTSUPP` for an
op the primary driver supports elsewhere.

For drivers that do not share payloads, `filer_api::Hybrid<A, B>` routes by
op kind and mints its own handles, translating them for each inner driver;
it performs no I/O itself, so it lives in `filer-api` (§4.1).

## 17. Drivers

### 17.1 `filer-posix`

The portable Unix driver: Linux, macOS, FreeBSD, and other `rustix`
targets. `Posix` takes an executor:

- `Posix::thread_pool(spawner)`: ops run on a `Spawner`; a chain or coarse
  op is one job; directory listing fetches large batches per job. The
  default.
- `Posix::blocking()`: executes each op synchronously inside `poll`. It blocks
  the executor, and its rustdoc says so in the first sentence. Useful for
  single-threaded tools and deterministic tests on a real filesystem
  (`filer-wasi` offers the same for WASI, §17.8). Same syscall layer, so
  it is nearly free.

`rustix` coverage, audited against `rustix` 1.1.6 source on 2026-10-31:

- **Present:** `openat2` with `RESOLVE_*` flags; `statx` including
  `STATX_DIOALIGN` and `STATX_MNT_ID`; `renameat_with` (`NOREPLACE`,
  `EXCHANGE`; on Apple targets via `renameatx_np` with `RENAME_SWAP`);
  `O_TMPFILE`; `linkat`; `ioctl_ficlone`; `copy_file_range`; `fallocate`;
  `SEEK_DATA` / `SEEK_HOLE`; fd-based xattrs; `flock`; `syncfs`; `RawDir`
  for `getdents`; `fdatasync`; `ioctl_getflags`; `fcntl_fullfsync` and
  `fclonefileat` on Apple; `O_RESOLVE_BENEATH` and `AT_RESOLVE_BENEATH` on
  FreeBSD.
- **Missing:** `STATX_MNT_ID_UNIQUE` and the `STATX_WRITE_ATOMIC` fields;
  `FICLONERANGE`; `FIDEDUPERANGE`; OFD locks. **Also a trap:** `rustix`'s
  `fcntl_lock` uses classic `F_SETLK`, the POSIX lock that is released when
  the process closes *any* descriptor for the file. Filer must not use it.
- **VERIFY:** `F_BARRIERFSYNC`, `FS_IOC_SETFLAGS`, `RWF_ATOMIC` via
  `pwritev2`.

Plan for the gaps (D28): upstream PRs to `rustix` first. Until they land,
either defer the feature (`FICLONERANGE`, `FIDEDUPERANGE`, unique mount
IDs, atomic write units are all optional) or add a confined `sys` module
over `libc` with documented `unsafe` (§3.3). Advisory locks use `flock`
meanwhile (D27): it locks per open file description, so it does not have
the classic `fcntl` lock's close-any-descriptor problem. On Linux NFS,
`flock` is emulated with byte-range locks, which the docs mention.

Configuration: maximum threads (via the spawner), forced userspace
resolution (for testing and for kernels that misbehave), and an error hook
for background closes.

As built (2026-11-01), read-only and blocking only: `Posix::blocking()`
answers the six read ops, and `open_ambient_dir(path, AmbientAuthority)`
opens roots. On Linux and Android, `Op::Open` first tries `openat2` with
`RESOLVE_NO_MAGICLINKS` and `BENEATH`, `IN_ROOT`, or `BENEATH |
NO_SYMLINKS` per `Symlinks`, plus `NO_XDEV` when mount crossing is off.
`EAGAIN` is retried up to 8 times; `EXDEV` (an escape or a mount
crossing, indistinguishable) and exhausted retries fall back to the walk
for this open, and `ENOSYS` or `EPERM` switch the instance to the walk for
good (which errno seccomp filters return is still VER-04). Errors after a
fallback carry `Fallback("openat2", "userspace walk")`, which a Linux test
uses to prove the kernel path ran. `Posix::with_userspace_resolution()`
forces the walk; the specification runs both ways, and a third time on
`Posix::thread_pool(DefaultPool::new())`, which wraps the same synchronous
operation in `run_on` (a panicking job fails the operation with
`Other`). Everything else, and other systems, use the generic
`BlockingResolver` (§6.5) on a private `Walker`: `lookup` is `statat` with
`AT_SYMLINK_NOFOLLOW`, then directories open with `O_DIRECTORY |
O_NOFOLLOW` (a swap to a symlink in between fails rather than escapes);
files open with `O_NOFOLLOW | O_NONBLOCK`, are `fstat`-checked to be
regular, and have `O_NONBLOCK` cleared. Metadata is `fstat`/`statat`
(no birth time; `statx` later). Listing uses `rustix::fs::Dir`, which
reopens `.` for an independent stream; the stream stays in the handle
between batches, and a `DirCookie` counts entries listed, because
`d_off`/`telldir` are not portable through `rustix` (`DirEntry::offset`
is missing on Apple targets); an unexpected cookie restarts and skips.
Errno values map to `ErrorKind` through a table, keeping the raw code.
`EINTR` never surfaces: every system call goes through a `retry` helper,
and since a `rustix::fs::Dir` stream stops for good after any error, an
`EINTR` while reading entries reopens the stream at the same position
instead. A debug assertion in the errno mapping enforces this.
`ErrorKind::Interrupted` is thus reserved for interruptions a driver
cannot retry. Verified on macOS and, in Docker, Linux at 1.97.

Planned kernel restriction (§6.7, D39): Capsicum rights on FreeBSD
descriptors, mirroring each handle's `Rights`; Landlock rulesets on Linux;
`unveil` and `pledge` on OpenBSD. Each is opt-in, per driver instance or
per call, since the process-wide ones (Landlock, `unveil`, `pledge`) are
irreversible.

### 17.2 `filer-windows`

Built directly on `windows-sys` (D22), with all `unsafe` in its `sys`
module. Symbols this design needs were confirmed present in `windows-sys`
0.61.2 on 2026-10-31: `NtCreateFile`, `NtQueryDirectoryFile`, and
`NtFlushBuffersFileEx` (features `Wdk_Storage_FileSystem`,
`Wdk_Foundation`), `FILE_RENAME_FLAG_POSIX_SEMANTICS`,
`FILE_DISPOSITION_FLAG_IGNORE_READONLY_ATTRIBUTE`,
`FLUSH_FLAGS_FILE_DATA_SYNC_ONLY`, and `FSCTL_DUPLICATE_EXTENTS_TO_FILE`.

Design:

- **Confined resolution** is a userspace walk (§6.5) over `NtCreateFile`
  with `OBJECT_ATTRIBUTES.RootDirectory` set to the parent handle and a
  single-component name. `FILE_OPEN_REPARSE_POINT` opens links instead of
  following them; the walker reads reparse data
  (`FSCTL_GET_REPARSE_POINT`), follows `IO_REPARSE_TAG_SYMLINK` and
  `IO_REPARSE_TAG_MOUNT_POINT` (junctions) under the `Symlinks` policy,
  treats absolute targets per `Beneath`/`InRoot`, and refuses other tags
  (app execution aliases, cloud placeholders, dedup) as links.
- **Case sensitivity:** pass `OBJ_CASE_INSENSITIVE` to match Win32
  behavior, except in directories flagged case-sensitive. **VERIFY** how
  NTFS per-directory case sensitivity interacts with the flag.
- **Names:** NT-relative opens bypass Win32 name normalization, so they
  *could* create `CON` or `name.` that Win32 tools cannot open. Filer
  rejects those with `InvalidNameForTarget` by default (§21.4); creating
  them would be a trap for every other program on the machine.
- **Listing:** `NtQueryDirectoryFile` (or `GetFileInformationByHandleEx`
  with `FileIdExtdDirectoryInfo`) in large batches; 128-bit file IDs come
  for free.
- **Metadata:** `GetFileInformationByHandleEx` (`FileBasicInfo`,
  `FileStandardInfo`, `FileIdInfo`, `FileAttributeTagInfo`,
  `FileStorageInfo`).
- **Rename** relative to a directory handle: `SetFileInformationByHandle`
  with `FileRenameInfoEx`, whose `RootDirectory` field makes the target
  name relative; flags `FILE_RENAME_FLAG_POSIX_SEMANTICS` and optionally
  `FILE_RENAME_FLAG_REPLACE_IF_EXISTS`.
- **Delete:** `FileDispositionInfoEx` with `FILE_DISPOSITION_FLAG_DELETE`,
  `FILE_DISPOSITION_FLAG_POSIX_SEMANTICS`, and
  `FILE_DISPOSITION_FLAG_IGNORE_READONLY_ATTRIBUTE`, so read-only files are
  removable without a separate attribute change.
- **Hard links** relative to a directory handle: `NtSetInformationFile` with
  `FileLinkInformation` (which has a `RootDirectory` field).
- **Symlinks:** created as reparse points; creation needs Developer Mode or
  the symlink privilege, which `Features::symlinks` reports.
- **Durability:** `NtFlushBuffersFileEx` with
  `FLUSH_FLAGS_FILE_DATA_SYNC_ONLY` for `Durability::Data`,
  `FlushFileBuffers` for `Full`. **VERIFY** whether flushing a directory
  handle is meaningful on NTFS and ReFS.
- **Sparse and clones:** `FSCTL_SET_SPARSE`, `FSCTL_SET_ZERO_DATA`,
  `FSCTL_QUERY_ALLOCATED_RANGES`, `FSCTL_DUPLICATE_EXTENTS_TO_FILE` (ReFS,
  Dev Drive).
- **Ambient roots:** `CreateFileW` with a `\\?\` path and
  `FILE_FLAG_BACKUP_SEMANTICS` for directories.
- **Executor:** the shared `Spawner` first. Overlapped I/O on an IOCP for
  reads and writes is a natural later step: completion-based, and the
  owned-buffer model fits it.

As built (2026-11-02): `Windows::blocking()` and
`Windows::thread_pool(spawner)`, as for `Posix`. `open_ambient_dir` opens
roots through the standard library's `OpenOptions`: `CreateFileW` with
`FILE_FLAG_BACKUP_SEMANTICS`, to which the standard library adds `\\?\`
for long paths, listing, traversal, and attribute access, and all share
modes; anything but a directory is `NotADirectory`. Win32 error codes map
to `ErrorKind` through a table, keeping the code; an `NTSTATUS` maps
through the code `RtlNtStatusToDosError` gives, except where the code
misstates it (a file opened as a directory is `ERROR_ACCESS_DENIED`).
`Op::Open` and `Op::ReadLink` resolve through the generic
`BlockingResolver`: `lookup` opens one component with `NtCreateFile`
relative to the directory, with `FILE_READ_ATTRIBUTES` only (which needs
no permission on the file), `FILE_OPEN_REPARSE_POINT`, and
`OBJ_CASE_INSENSITIVE`, and classifies it by `FileAttributeTagInfo`:
symbolic links, junctions, and WSL symlinks are symlinks, other name
surrogates `Other`, and other reparse points (cloud placeholders, WOF,
dedup) the files or directories they stand for. Directories, and files
at the end, are reopened from that handle with an empty relative name
(`ReOpenFile` refuses directories), without `FILE_OPEN_REPARSE_POINT`, so
a filter's files read through the filter; nothing is opened by name
twice, except a link read for its target. Targets come from
`FSCTL_GET_REPARSE_POINT` through `filer_msft::reparse`, as `filer-ntfs`
reads them, so a junction's `\??\C:\path` is `/path`, absolute: an
escape under `Beneath`, from the root under `InRoot`. Names holding `\`
or `:` (another component, or a stream) are `InvalidNameForTarget`,
since `NtCreateFile` would parse them. `crosses_mount` compares volume
serial numbers. `Op::Read` is `ReadFile` at an offset, through the
standard library's `seek_read`, `ERROR_HANDLE_EOF` reading nothing.
`Op::Stat` reads `FileAttributeTagInfo`, `FileBasicInfo`,
`FileStandardInfo`, and `FileIdInfo` from the handle, or from the
resolved entry's lookup handle: the kind, as above; the length, or for
a link its target's, as on POSIX; the four times through
`filer_msft::nt`, a zero time absent; the id, the volume's serial number
and the 128-bit file id; and the attributes that are flags. A file that
the Windows Overlay Filter compresses hides its reparse point and reads
through the filter. `Op::ReadDir` reopens the directory for each listing
(the system keeps a listing's position in the handle) and reads
`FileIdExtdDirectoryInformation` with `NtQueryDirectoryFile`, 64 KiB at
a time; file systems that refuse the class before the first entries
(FAT) are read in `FileIdBothDirectoryInformation`, with 64-bit ids and
the tag where the extended attributes' size would be. Records parse in
a pure function. Entries keep the kind lookups give them and an id on
the directory's volume; names that are not well-formed UTF-16 are
foreign names in WTF-8, which open and stat. As for `filer-posix`, the
listing stays in the handle between batches, a `DirCookie` counts the
entries listed, and an unexpected cookie restarts and skips. The whole
specification passes, blocking and on a thread pool, with FIFOs,
extended attributes, and its raw-byte foreign name (SPEC-06) left out.
What the driver parses from the system, reparse points and directory
records, is pure functions: their unit tests mutate real records (and
pass under Miri), and `fuzz/` has `cargo-fuzz` targets for both, which
build and run on any host. `tests/windows.sh` runs the crate's tests in
Windows from Linux: it builds them for `x86_64-pc-windows-gnu` and runs
them in the `dockur/windows` container that `filer-ntfs`'s fixtures use.

`winx` (0.36.4) has useful helpers (`reopen_file`, reparse-point
`read_link`, access-mode queries), but it constrains `windows-sys` to
`>=0.52, <=0.59`, so depending on it alongside `windows-sys` 0.61 would put
two copies in the tree. Use it as a reference and port the handful of
helpers, or depend on it only after its range covers the workspace version.

### 17.3 `filer-uring`

- Wraps a `Posix` instance: mints handles with `filer-posix`'s payload type
  and routes every op io_uring cannot do (listing, clones, and whatever the
  opcode probe reports missing) to it, so no handle translation is needed.
- Probes supported opcodes at construction (`IORING_REGISTER_PROBE`).
- Setup failures with `ENOSYS` or `EPERM` (old kernels, container seccomp
  profiles, distributions that disable io_uring) make construction return
  `Unsupported`; `Uring::new_or_posix` falls back. CI containers are such
  an environment, so the fallback path is tested by default.
- First implementation: one ring per driver instance, owned by a dedicated
  completion thread; ops arrive through a channel and complete through
  wakers. That works with any runtime and keeps futures `Send`. Per-thread
  rings for thread-per-core executors come later with `LocalDriver`.
  Tokio's experimental io_uring file support found single-ring contention
  to be its main limit, so measure.
- Chains map to linked SQEs; placeholders map to direct descriptors;
  `PooledBuf` maps to registered buffers; reads may target uninitialized
  memory (§7.1).
- Backend: the `io-uring` crate or `compio-driver`; a spike decides.
  Either way the crate contains `unsafe`, confined to `sys`.

### 17.4 `filer-heap`

An in-memory filesystem: the reference implementation of the driver
contract and the backbone of the test strategy. `no_std` + `alloc`.

- **State.** An inode table and directory maps, each with a *volatile* and a
  *durable* version. Ops mutate volatile state; `Sync` copies the relevant
  parts to durable state (`Data`: contents and size; `Full`: plus metadata;
  sync on a directory: its entries).
- **Crash model.** `crash(policy)` rebuilds volatile state from durable state
  according to a `CrashPolicy`:
  - `Pessimistic`: nothing unsynced survives;
  - `Reordered { seed }`: a random subset of unsynced operations survives,
    respecting per-file write order, modeling real filesystem latitude;
  - `Torn { seed, sector }`: unsynced writes may persist partially at sector
    granularity.
- **Fault hooks.** `ENOSPC` after N bytes, `EIO` on the Nth sync with
  fsyncgate semantics (pages dropped and reported clean), short reads and
  writes, `Busy` on rename (simulating Windows sharing violations).
- **Personalities.** Naming rules (case-insensitive,
  normalization-insensitive, 255-byte or 255-unit limits, Windows reserved
  names), feature set (no reflink, no `RENAME_EXCHANGE`, no `O_TMPFILE`),
  and timestamp granularity, so code can be tested against filesystems the
  CI machine does not have.
- **Determinism.** An injectable `Clock` and seeded randomness.
- **Foreign names.** A test-support API inserts non-UTF-8 names.
- **Crash harness.** A `harness` feature exposes the crash-point
  enumeration used by `filer-tx` and application tests (§24).
- **Locks.** Synchronous mutexes only (§7.7).

As built (2026-11-01), read-only: `HeapBuilder` fixes the contents at
construction (`dir`, `file`, `symlink`, `xattr`, taking string or `RelPath`
arguments, creating parents like `mkdir -p`, panicking on mistakes), and
the tree, a `filer_api::tree::Tree<Vec<u8>>` (§17.7), is then immutable
behind an `Arc`, so no locks are needed yet. Directories map raw name
bytes to inodes in a `BTreeMap`, so listings are sorted by bytes, foreign
names will fit, and a `DirCookie` is an entry index. Handle payloads are
inode numbers; `FileId`s are `(0, inode)`. Paths resolve through the
tree's `BlockingResolver` (§6.5), and a following single-entry stat uses the
policy `Target::Entry` carries. `ReadLink` on a non-symlink is
`ErrorKind::NotASymlink`.

### 17.5 `filer-vfs`

A mount table mapping `RelPath` prefixes to `(DynDriver, root handle)`
pairs. Resolution that crosses a mount boundary is done component by
component; within a mount it is delegated to that mount's driver (and so to
`openat2` where available). Cross-mount rename returns
`ErrorKind::CrossesDevices`; copy-plus-delete is opt-in. `Features::volume`
differs per mount, so clone and transaction checks work unchanged. Handles
are translated by wrapping inner handles with a mount index.

### 17.6 `filer-overlay`

Read-only lower layers under a writable upper layer, following overlayfs
semantics: whiteouts for deletions, opaque directories, copy-up on first
write (using clone when upper and lower share a volume). Useful for
sandboxes, test fixtures, and "try this change" workflows.

### 17.7 Archive drivers: `filer-tar`, `filer-zip`, and more

Read-only archive drivers, one crate per format (D41, answering Q7's
packaging question): they need different dependencies and codec features,
and users rarely need both. Both are virtual drivers like `filer-heap`:
`Enforcement` is `Virtual`, rights are at most `READ`, paths resolve
through the generic `BlockingResolver` (so archive symlinks are confined
beneath the archive root), and no `AmbientAuthority` is needed, because the
archive's `Dir` derives from a capability the caller already holds:

```rust
let archive = root.open_file("release.zip").await?;
let contents: Dir<Zip<_>> = filer_zip::Zip::open(archive).await?;
```

**Source.** Drivers read through `filer_api::ReadAt` (D42): an async
positional `read_at` plus `size` (not `len`, which would shadow slices' own
`len` wherever the trait is in scope), implemented by `File<M: Readable,
D>` and by in-memory bytes (`&'static [u8]`, `Vec<u8>`, `Arc<[u8]>`, ...), so a
member of one driver, a `no_std` `include_bytes!` archive, or a stored
member of another archive can all be sources. Access needed per format:

| Format | Needs | Why |
| --- | --- | --- |
| ZIP | random access, length | its index is the central directory at the end |
| TAR | random access | no index: one pass over 512-byte headers |
| compressed TAR | rewinding | seeking back re-decompresses from the start |
| forward-only | not a `Dir` | a streaming visitor or extractor only |

Reading at an offset inside a compressed member (Deflate, zstd, ...) costs
decompressing from the member's start; drivers cache or fully decompress
small members, and only stored members are truly random-access. Codecs
are features, and pure-Rust decoders only (D45), preferring `no_std`
ones.

**Names and paths.** Non-UTF-8 names become `ForeignName`s. Names with
`..`, absolute or drive prefixes, or backslashes are never resolved
outside the root: quarantined or rejected. Directories missing from the
archive (common in ZIP) are synthesized. Decoding ZIP's legacy names
(no UTF-8 flag, bit 11) is planned with `known-codepages` (D49): nominally
CP437, but in practice often the writer's OEM code page, so the code page
will be an open option, as in `filer-fat`; until then such names are
UTF-8 if valid and foreign otherwise. **Duplicate entries** are resolved the same way in both
formats: the later entry wins (TAR's append semantics; for ZIP, later in
the central directory). TAR hardlinks become one node with one `FileId`.

**Metadata.** TAR gives mode, owner, and type; ZIP gives DOS local times
at 2-second granularity without a time zone (reported through
`Timestamp::granularity`) unless an extended-timestamp field gives Unix
time. Compressed size will map to `allocated`.

**Security.** Strict limits are the default, configurable per archive:
total uncompressed size, per-member decompression ratio (zip bombs),
entry count (index memory), and path length. ZIP local and central headers
that disagree are an error, not a choice, since parser differentials are
exploited. Extracting by walking an archive `Dir` into a destination `Dir`
is confined by construction ("zip slip" and symlink-then-write attacks
cannot name anything outside the destination), which makes these drivers
a safer extraction tool than most.

**`filer-tar` as built** (2026-11-01, uncompressed, random access): an
own parser (answering Q7 for TAR: small, `no_std`, and under our limits),
reading POSIX ustar with the name prefix, PAX `path`/`linkpath`/`size`/
`mtime`, GNU long names and base-256 numbers; global PAX headers are
ignored, GNU sparse members are `Unsupported`, and unknown types read as
regular files, as POSIX requires. Opening fails as a whole on any unsafe
member name (`InvalidName`: absolute, `..`, NUL), malformed or truncated
header (`Corrupt`), or exceeded `Limits` (the new `LimitExceeded`: entry
count, name length, extended-header size); the error's source names the
header's offset. Quarantining instead of failing is left for later.
Reads go through a bounded intermediate buffer so a source never reads
past a member into the next header; a `Slice` buffer (CORE-06) would
remove that copy. The full `filer-spec` passes, with nothing skipped, on
archives written from its tree; a doc example reads a bsdtar archive, and
a test reads an archive stored as a `File` in `filer-heap`. Since
2026-11-03, `tests/fuzz.rs` mutates headers and the records of PAX and
GNU extended headers, mostly resealing a mutated header's checksum so
that the mutation reaches what parses it; `fuzz/` has a `cargo-fuzz`
target over the same walk, which reseals each header with the `ustar`
magic. Also since 2026-11-03, it reads archives compressed with gzip,
zstd, xz, or bzip2 (TAR-02), told by their streams' magic numbers unless
the first block is a valid header, through `filer-pack`'s decoders and an
`Unpacked` source: opening decodes the stream once to learn its size,
bounded by `Limits::max_total_size` against bombs, and again as it
indexes; reading on from the last read decodes no more, while reading
back decodes again from the start, or from a checkpoint, where the
codec offers them (PACK-16 to 20). The full `filer-spec` passes over gzip-compressed archives too.
`filer-archive` names compressed tarballs by their two-part extensions,
and since ARC-01 (2026-11-03) tells every format by its content first:
magic numbers at the start, those of the first bytes decoded from a
compressed stream, which only a tarball or cpio archive may be, and a
ZIP end record at the end, after a self-extractor's stub.

**`filer-zip` as built** (2026-11-01, stored members): an own parser
too (answering Q7 for ZIP), reading the end of central directory record,
ZIP64 records and extra fields, and the central directory, through a
reader that fetches it in 64 KiB chunks. The end record must be the one
candidate whose comment runs exactly to the archive's end; a comment
holding a second plausible record is `Corrupt`, as is a ZIP64 record that
a non-sentinel field of the classic record contradicts. Each member's
local header must agree with its central header on name, method,
encryption, and, without a data descriptor, CRC and sizes, and its data
must lie before the central directory. Names with a backslash or a drive
letter are `InvalidName`, besides what `Index` refuses. Directories,
symlinks, and special files come from Unix modes (hosts 3 and 19), with
a trailing `/` or the DOS directory attribute also making directories.
Times come from the NTFS extra field, then the extended timestamp, then
the DOS fields, read as UTC. Since 2026-11-04 (ZIP-03), names without
the UTF-8 flag come from Info-ZIP's Unicode Path extra field, if its CRC
matches, or, from DOS and Windows hosts (0, 6, 11, 14), are decoded from
`Options`' code page, 437 by default, through `known-codepages`, as
Info-ZIP decodes them; Unix hosts' stay UTF-8 if valid, since macOS's
`zip` writes UTF-8 without the flag, and foreign otherwise. Names are
checked after decoding. Its codec records each member's method,
CRC-32, and encryption: Deflate decodes with `filer-pack`'s, over
`decode-deflate` (feature `deflate`), checked against the CRC at the
member's end; since 2026-11-03 (ZIP-02), so do bzip2 (12), LZMA (14,
after its own header of the SDK's version and the properties), zstd
(93), and xz (95), each behind a feature, which a Python 3.14 `zipfile`
archive of each tests, and since 2026-11-04 (DEC-02) Deflate64 (9),
behind `deflate64`, which a 7-Zip archive tests; other methods
and encryption fail on read, not open, except for symlinks, whose
targets are decoded at open. Spanned archives are `Unsupported`.
Against bombs, `Limits` gained `max_total_size` (16 GiB of declared
sizes) and `max_ratio` (100:1, for members over 1 MiB, as Apache POI
exempts small ones), checked at open; since decoding stops at declared
sizes, they bound the output too. Members whose local header and data
spans overlap are `Corrupt`, which stops David Fifield's bombs without
recursion (2019): no archiver writes shared bytes. The
full `filer-spec` passes on archives written from its tree, stored,
deflated, and with ZIP64; a doc example reads an Info-ZIP archive with
stored and deflated members. `tests/fuzz.rs` mutates archives at their
records and checks each walk, including that the bytes read never
exceed 1 MiB per file plus `max_ratio` times the input (the bound the
limits and the overlap check together guarantee), and that generated
names are refused exactly when unsafe; `fuzz/` has `cargo-fuzz` targets
for whole archives, Deflate data, and names (D50).

**`filer-ar` as built** (2026-11-03): an own parser of the 60-byte
headers, reading all three dialects of names: System V's and Debian's
short names, GNU's `/`-terminated ones and its `//` table of long names
(which Windows ends with NUL rather than `/\n`), and BSD's `#1/` names
stored before the data. Symbol tables and other members for the linker
(`/`, `/SYM64/`, `/<`...`>/`, `__.SYMDEF` and its variants) are hidden; a
`/` within a name separates directories, as `ar P` writes; GNU thin
archives are `Unsupported`. The name table's size counts against
`max_extension_len`. Of members with the same name, which static
libraries may hold, the later wins, as `Index` decides for every format.
`filer-spec` passes in every dialect, with a new `Profile::empty_dirs`
off, since ar holds only files; the fixtures come from GNU `ar`,
`llvm-ar` (BSD and Windows formats), macOS's `ar`, and `dpkg-deb`, and a
doc example opens a `.deb`'s `data.tar` with `filer-tar`. `tests/fuzz.rs`
mutates headers and checks that a walk never reads more than the
archive's length; `fuzz/` has a `cargo-fuzz` target over the same walk.

**`filer-cpio` as built** (2026-11-03): an own parser of `newc` (with
or without checksums), `odc`, and the old binary format in either byte
order, each header by its own magic number. Hardlinks are found by
device and inode number among non-directories whose link count exceeds
one, within each archive, and share one node through the new
`Index::add` and `Index::alias`, so a link holds even if a later member
takes the first link's name; since `newc` stores a file's data only
with its last link, a first pass collects the headers and a second
builds the tree with each file's data from its last link that has any.
A `newc` checksum is a `Codec`, `Sum`, checked when a read reaches a
regular file's end; GNU cpio writes 0 for other members, so theirs are
not checked. Archives concatenated, as in a Linux initramfs image, read
as one, with zeros allowed between them; a compressed archive among
them (gzip, bzip2, LZMA, xz, LZO, LZ4, zstd) is `Unsupported`, which
for now means most distributions' images (an uncompressed early archive
of microcode, then a compressed one), and anything else after a trailer
is `Corrupt`. Unknown file types are `Corrupt`; an archive may end
without its trailer. The full `filer-spec` passes in every format; the
fixtures come from GNU cpio in each of its formats and macOS's
`bsdcpio`, and an uncompressed initramfs image; decompressed by hand, a
real Ubuntu image's 2,468 entries read as GNU cpio extracts them.
`tests/fuzz.rs` mutates headers and names; `fuzz/` has a `cargo-fuzz`
target over the same walk. Since 2026-11-03 (CPIO-02), an archive
compressed with gzip, zstd, xz, or bzip2 at an archive boundary reads
decoded, as the Linux kernel unpacks it: the image becomes a spliced
source, as it is up to there and decoded after, through `filer-pack`'s
`Unpacked`, so that indexing goes on through the decoded archives as
through plain ones; the `Compression` that tells them apart moved to
`filer-pack` from `filer-tar`. Ubuntu 26.04's own 61 MB image, an early
archive and a zstd one, reads whole as its `unmkinitramfs` extracts it:
1,805 files of 302 MB, 480 directories, 183 symlinks, and 5 devices.
Reading every file in the order of a directory walk took 73 s in a
release build, as each read before the last decodes again from the
start. Measured there, opening decodes the 97 MB of its zstd part twice
in 1.6 s; reading every file in the archive's order takes 6.4 s, and in
reverse 383 s. Its zstd part is one frame, with a 4 MiB window, so the
checkpoints at frame boundaries of PACK-16 cannot spare those restarts:
within a frame, they need a copy of the decoder's state, which
`ruzstd`'s could not make, and which ours, since PACK-24, can
(PACK-20).

**Shared structure.** An archive index is an immutable tree like
`filer-heap`'s, with leaves pointing at (offset, size, codec) instead of
bytes. As built (2026-11-01), that tree is `filer_api::tree::Tree<F>`,
generic over the file payload: `Node::{Dir, File(F), Symlink(bytes),
Special(FileKind)}`, policy-free primitives (`add`, `link` returning the
replaced inode, `make_dir`), and a `BlockingResolver` impl. Drivers apply their
own policies (`Heap` panics on duplicates; archives let the later entry win)
and keep their tree private.

**Shared machinery (D44).** Everything but header parsing lived in
`filer_api::archive`: `Index` (the shared rules: safe names, implied
directories, later duplicates win, hardlinks share a node), `Archive<S>`
(the read-only driver serving a `Tree<Member>` from a `ReadAt` source),
`Member`, `Limits`, and `ArchiveError` (format, offset, reason). Each
format crate is then a parser feeding an `Index` and a thin wrapper type
delegating to `Archive`, as `filer_tar::Tar` is. This bent the placement
rule (D20: state and semantics get their own crate) for convenience while
the formats settled; with four formats built, it moved to `filer-pack`
(D59, below).

**Compressed members** (as built, 2026-11-01). `Member<C>`, `Index<C>`,
and `Archive<S, C>` are generic over the format's `Codec`, defaulting to
`Stored` (TAR's, which never decodes). A member records its `packed`
length and its codec value (for ZIP, the method and CRC); a codec that
is not stored gives a boxed streaming `Decoder`. `Archive` keeps up to
four streams part way through members, behind a lock per §7.7 (`spin`
without `std`): a read continues the furthest stream not past its
offset, or starts one, and decodes exactly to the read's end, so a
sequential pass costs one decode and seeking backwards restarts. Output
never passes the declared size, which bounds decompression bombs by what
the index accepted; `Decoder::finish` checks a member (its CRC) only when
a read reaches its end.

**`filer-pack`** (D59, 2026-11-03) is where the archive drivers share
code, as `filer-fs` is for the file system drivers (D54). Read side by
side, the four drivers turned out to repeat, beyond what
`filer_api::archive` already shares, the same read helper and limit
checks in every parser, the same checksum-verifying decoder in two
codecs, and, in `filer-zip`, NT times, DOS times, and Unix modes that
`filer-fs` and `filer-fat` already decode. And the codecs to come
(TAR-02, ZIP-02, CPIO-02, 7Z-01) had no shared home: compression comes
in three shapes, members each a stream (ZIP), the whole archive one
stream (compressed tarballs and initramfs images), and solid blocks of
many members (7z, RAR), and the codec traits and `Archive`'s stream
cache serve only the first. The name: `filer-archive` is the umbrella
(D56), so the shared crate needed another word for the thing itself. A
ZIP, a tarball, or a 7z is a pack of members, `Member` already calls its
compressed length `packed`, and the decoders unpack. `filer-codec` was
the runner-up, right for decoders alone but not for the index and the
driver; `filer-archive-core` broke the workspace's two-word names;
`filer-compress` implied writing (D43); `filer-stream` is kept for the
streaming visitor (TAR-03). The name was free on crates.io that day.

What moves in, in this order (TODO PACK-01 to PACK-10):

- all of `filer_api::archive`, as one mechanical change, `spin` with
  it (done, 2026-11-03): `filer-api` then holds no archive state and no
  lock, which mends D44's bend of the placement rule (D20), and the
  stream cache can be shared without `filer-api` depending on
  `filer-pack`; nothing is re-exported from `filer-api`, as 0.0.x may
  break (D29);
- the limit checks, into `Index`, which then counts recorded members
  rather than headers, and the read helper the parsers copy (done,
  2026-11-03: `Parser` reads and fails for a format, and `Index` takes
  it and the `Limits`; cpio records in one pass, a hardlinked file's
  late data replacing its first link's member through
  `Tree::node_mut`);
- one checksum-verifying decoder over a digest trait, in place of
  `filer-zip`'s CRC wrapper and `filer-cpio`'s byte sum, ready for the
  sums gzip, xz, zstd, and 7z stamp (done, 2026-11-03: `Verified`,
  `Digest`, and `Passthrough`);
- the stream cache, out of `Archive` and keyed by an opaque stream id
  (done, 2026-11-03: `Streams` of `Stream`s by `StreamId`, a `Stream`
  holding the decode loop over a packed range), and over it a `ReadAt`
  source over one decoded stream, for compressed tarballs and initramfs
  images, and for solid blocks (done, 2026-11-03: `Unpacked`, over a
  `Codec` as its decoder factory);
- the decoders, one per algorithm behind one feature each, Deflate
  first, from `filer-zip`; each format crate forwards the features it
  uses, as `filer-archive` forwards `deflate`; each decoder crate is
  assessed in `LEGAL.md` before it is adopted (D45: pure Rust, and
  `no_std`) (Deflate done, 2026-11-03: `Deflate`, behind `deflate`,
  which `filer-zip` forwards; gzip done, 2026-11-03: `Gzip`, behind
  `gzip`, over `Deflate`, reading concatenated members and ignoring
  trailing garbage after the first, as zlib does; a `Stream` now gives
  its decoder the packed bytes after the declared size, with no room for
  output, so that it checks a trailer before it finishes; `tests/fuzz.rs`
  and `fuzz/` check that each decoder decodes the same in pieces as
  whole; zstd done, 2026-11-03: `Zstd`, behind `zstd`, at first over
  `ruzstd`, which decoded only the whole blocks that `Zstd` gathered,
  parsing frame and block headers itself, and refusing what `ruzstd`
  would not: the frame header's reserved bit, and reserved blocks, on
  which it panicked; our own since 2026-11-04 (PACK-24, below);
  LZMA done, 2026-11-03: `Lzma` and `Lzma2`, behind `lzma`, for raw
  streams with the properties their container gives, and `Xz`, behind
  `xz`, over `lzma-rust2`'s sans-I/O streams, each told that the input
  is exhausted by an empty slice; dictionaries over 128 MiB, as zstd's
  windows, are `Unsupported`; checkpoints done, 2026-11-04 (PACK-16):
  `Decoder::checkpoint` offers a decoder that goes on from where one is,
  which a `Stream` records at most every 4 MiB it decodes, and which
  `Streams` resumes a read from when it lies nearer than any kept
  stream, as `Unpacked::measured`'s first decode records them for every
  later read; zstd offers a fresh decoder between frames, and Deflate
  and gzip a copy of their decoder's state anywhere (PACK-17), as does
  `Verified`, with its digest so far, for ZIP's members: reading the
  member before the last of a 12 MiB `.tar.gz`, after the last, takes 82
  reads of 64 KiB rather than 177; past 256 checkpoints, every other one
  is dropped, which bounds gzip's to about 11 MB; bzip2 offers a fresh
  decoder between blocks (PACK-18), at the bit where the next begins,
  with the stream's limit and CRC so far, and `Decoder::buffered` says
  how many bytes before what it consumed that copy must be given again,
  as it gathers whole blocks: reading near the end of 12 MiB in 14 blocks
  decodes at most 5 MB, not all of it; bzip2 done,
  2026-11-03: `Bzip2`, behind
  `bzip2`, written here from Joe Tsai's description, which gathers each
  block whole, trying again only once its input has doubled, so that
  parsing stays linear, and undoes the Burrows–Wheeler transform and the
  first run-length coding as the block is read, so that one holds at
  most about 900 KiB packed and 3.5 MiB of links, whatever it decodes
  to; randomized blocks, unwritten since 1999, are `Unsupported`);
- the archive fuzz walk, into `filer-spec`, where shared test tooling
  goes (D54): `filer-zip`'s predates it (done, 2026-11-03: `Checks`
  allows refusals at open and reads files whole for zip's bomb bound,
  `Outcome` counts files, and `Mutations` writes unaligned words and
  fewer mutations at once, as ZIP needs).

What stays: each format's parser, codec value, wrapper type, and
hand-written test writer. The wrapper types repeat as `filer-fat`'s
does over `Mounted`, by design, and a macro would cost the docs that
make up most of their lines. The NT and DOS times the archives share
with Microsoft's file systems come from `filer-msft` (D60, §17.17),
Unix modes from a `FileKind` constructor in `filer-api`, and the field
readers stay per crate for now.

**Codec crates are interim (D61, 2026-11-04).** The decoder crates
`filer-pack` adopts under permissive licenses (`miniz_oxide`, `ruzstd`,
`lzma-rust2`) are conveniences on the way to owning a clean-room,
public-domain decoder of each format, written here from its
specification, as `Bzip2` already is. Gaps in them are not waited on
upstream: a pull request may take months or years to be merged and
released. When a crate becomes an inconvenience or a blocker, its own
decoder takes priority instead. The first such gaps: checkpoints within
a zstd frame need a copy of `ruzstd`'s decoder, which it cannot make
(PACK-20), so `Zstd` is now ours (PACK-21 to 24); and checkpoints at
xz blocks a block-level decoder that `lzma-rust2` does not expose
(PACK-19), whose upstream request is recorded (UP-06), at low
priority.

**Our own zstd (PACK-21 to 24).** Written from RFC 8878 beside `Zstd`,
and checked against it, which is to say against `ruzstd`, until it
replaces it. Frames done, 2026-11-04 (PACK-21): headers, skippable
frames, raw and RLE blocks, and XXH64, from the xxHash specification.
Each frame header and block is gathered whole, or decoded straight from
the input when it holds it whole, onto a window that keeps the frame's
window size of the latest bytes and drops what is produced past them
once it is as much again, so that each byte moves once at most. It
refuses what zstd refuses and `ruzstd` lets pass: a frame whose content
size disagrees with what it decodes to, which its differential test
checks frame by frame. Literals done, 2026-11-04 (PACK-22): stored,
repeated, or Huffman-coded in one stream or four, with trees described
directly or by FSE-coded weights, or reused. FSE tables are built as
RFC 8878 §4.1.1 lays out, checked against its worked example, and
zstd's own output, in every size format, decodes as `ruzstd` decodes
it. zstd refuses, and so does it, a block that decodes to more than its
frame allows, and a sequences section that does not end the block;
`ruzstd` lets the former pass, which the differential test checks
block by block. Sequences done, 2026-11-04 (PACK-23): tables of each
mode, the predefined ones checked state by state against RFC 8878's
Appendix A, and repeat offsets as its prose has them (its Table 18
errs in its last two rows), run over the window with overlapping
copies doubling. zstd's output of synthetic text, LZ77-shaped data,
and records, at levels from 1 to 22 and windows from 1 KiB, decodes as
`ruzstd` decodes it, as do Ubuntu's initrd and a corpus of binaries,
libraries, and documentation at every level, strategy, and window,
and mutations of them, run on comet, find no difference. In place of
`ruzstd`, 2026-11-04 (PACK-24): Ubuntu's initrd, read as above, opens
in 1.2 s rather than 1.6 s, and reads in archive order in 5.1 s rather
than 6.2 s, and in reverse in 298 s rather than 381 s, which only
checkpoints within frames can spare (PACK-20). Spun out, 2026-11-04,
as `decode-zstd` (D62). Checkpoints within frames done, 2026-11-04
(PACK-20): `Decoder::checkpoint` copies the frame's window's size of
the latest bytes, its tables, repeat offsets, and hash so far, and
`Decoder::footprint` says what that holds, by which `Streams` keeps its
checkpoints within 64 MiB, dropping every other one past that, and none
over 8 MiB. Each kept stream also holds the last 4 MiB or more it
decoded, from which a read a little behind it is served without
decoding: as an archive's files read in reverse are, each just before
the last. Ubuntu's initrd, one frame with a 4 MiB window that decodes to
302 MB, now reads in archive order in 0.9 s, and in reverse in 2.1 s
rather than 298 s, on comet; with 32 MiB of checkpoints, in 3.8 s.

**Our decoders are `decode-*` crates (D62, 2026-11-04).** Each decoder
written for Filer (D61) is a crate of its own, `decode-<codec>`, a
namespace otherwise free on crates.io: `decode-zstd` and `decode-bzip2`
so far. Like `known-codepages` (D49), they are generic, not
Filer-specific, and written to be spun out: no Filer dependency
(`filer-spec` only to test), and no dependency at all. Each is written
clean room from its format's specification (`LEGAL.md`), dedicated to
the public domain, pure safe Rust, `no_std` with an allocator, never
panics on any input, and bounds its memory. All share one sans-I/O
shape: a `Decoder` with `new`, `decode(input, output)` returning what it
consumed and produced, `finish`, `checkpoint`, and `buffered`, and an
`Error` that is `Corrupt` or `Unsupported`. `filer-pack` adapts each to
its `Decoder` trait behind a feature named for the codec, keeping its
own types (`Zstd`, `Bzip2`, `Deflate`, `Gzip`, `Lzma`, `Lzma2`, `Xz`). A
side effect of Filer is thus a family of clean-room, public-domain
decoders of every codec it reads: Deflate, gzip, LZMA and LZMA2, and xz
next, in place of the interim crates (TODO DEC-02 to 05), then zlib,
LZ4, and LZO as the drivers that need them come (DEC-06 to 08). Stubs in
`tmp/` reserve their names. LZMA2 shares `decode-lzma` with LZMA, as it
is LZMA in chunks over the same model and window; and Deflate64, Deflate
with a larger window, a longer last length, and two more distances, is
to share `decode-deflate`, chosen per stream behind an additive feature,
so that enabling it never changes what Deflate decodes. `decode-deflate`
decodes since 2026-11-04 (DEC-02), from RFC 1951 alone, refusing what
zlib refuses: it decodes zlib's own output at each level and strategy,
and mutations of it, as zlib does, through `zlib-rs`, a port of it used
to test, and consumes as much of each stream as zlib does. It reads bits
ahead for speed only within a call's input, and gives back the whole
bytes it does not use when it returns, so that it never consumes past a
stream's end. While the input holds two words past what is taken, a fast
loop decodes codes without checking for too few bits, as every refill,
without a branch, leaves enough for two codes, a length's extra bits,
and, refilled again if short, a distance; its tables hold each length's
and distance's base and extra bits, and matches are copied in chunks of
16 or 8 bytes into a buffer with room past its end. So it takes a
quarter less time than `miniz_oxide` on the M4, and as much on Zen 1,
where `zlib-rs` takes two thirds of its time (on the M4, five sixths).
Deflate64 is a variant chosen per decoder (`Decoder::deflate64`),
checked against 7-Zip's output and the `deflate64` crate, which decodes
matches from before a stream's start, as zeros, that zlib and this crate
refuse. `filer-pack`'s `Deflate` adapts it since 2026-11-04, and with it
ZIP's method 9, in place of `miniz_oxide`, now only a test dependency,
which writes members and gzip streams to read. `decode-gzip` (DEC-03,
2026-11-04) frames it in gzip's members, as `filer-pack`'s `Gzip`, moved
there whole, did, with a CRC-32 of its own; its tests check it against
zlib's gzip decoding, member by member, through `zlib-rs`. `decode-zlib`
(DEC-06, 2026-11-04) frames it in zlib's header and Adler-32 trailer,
`compute-adler32`'s, and is checked against zlib likewise; as zlib does
by default, it checks the header's window size but does not impose it,
and a stream needing a preset dictionary is `Unsupported`.
`filer-squashfs` reads through it (FMT-08), as `filer-cdfs` does for
zisofs (FMT-07); btrfs and ZFS will.
`decode-lz4` (DEC-07, 2026-11-04) is written from LZ4's block and frame
format descriptions: `decode_block` decodes a raw block whole, as
SquashFS, EROFS, ZFS, F2FS, and ReFS store them, and `Decoder` decodes
frames, linked or independent, checked by XXH32 where they say,
skippable frames, and legacy frames, as the Linux kernel reads them from
initramfs images, ended by the input's end, a frame, or four zero bytes.
As `lz4` does, and `lz4_flex`, its tests' oracle, does not, it refuses
what follows the last frame and a frame cut short between blocks; it
accepts blocks that break the end conditions encoders keep (the last
five bytes literals, no match within the last 12), as both do.
`filer-squashfs` reads its blocks through it (FMT-08). `decode-lzo`
(DEC-08, 2026-11-04) is written from the Linux kernel's description of
the LZO1X stream format, and lzop's framing from its published layout,
checked against files `lzop` writes: `decode_block` decodes a raw block
whole, as btrfs, SquashFS, and F2FS store them, and `Decoder` decodes
lzop files, one after another, as the Linux kernel reads them from
initramfs images, each block checked by the Adler-32s or CRC-32s its
header asks for. As the Linux kernel does, and `lzokay-native`, its
tests' oracle, does not, it refuses bytes after a block's end mark; as
both do, an end mark of a length other than 3. It skips zero bytes after
the last file, as `lzop` does, and refuses anything else there; filtered
and multipart files are `Unsupported`. `filer-squashfs` reads its blocks
through it (FMT-08). `decode-lzma`
(DEC-04, 2026-11-04) is ported from the LZMA SDK's specification, its
reference decoder, and its LZMA2 decoder, and refuses what they refuse:
data that ends otherwise than its size or end marker says, or with the
range coder short of its end, which `lzma-rust2` accepts. Its dictionary
wraps around once full, and grows to its size only as the stream does;
while the input surely holds a packet, 20 bytes, it decodes straight
from it, and otherwise holds the bytes after a dry run shows them too
few, as the SDK's decoder does. Literals and bit trees decode without
branches, which their bits, as likely one as the other, would
mispredict; so it decodes as fast as `lzma-rust2` on the M4, and within
3% on Zen 1. `filer-pack`'s `Lzma` and `Lzma2` adapt it, without
checkpoints, which would copy the whole dictionary. `decode-xz` (DEC-05,
2026-11-04) is written from the `.xz` file format specification, its
branch converters ported from the LZMA SDK (x86, PowerPC, IA-64, ARM,
ARM Thumb, and SPARC from 9.20's plain ones, ARM64 and RISC-V from
26.05's), and its SHA-256 from FIPS 180-4, over `decode-lzma`'s LZMA2.
Each filter is a stage holding back what it cannot yet convert until
more follows or the block ends; the index is checked in constant memory,
by hashing the blocks' sizes as they end and as it lists them, which
must agree. It decodes chains of filters that `lzma-rust2` refuses, as
xz does, and decodes a tenth faster than it on the M4. It offers
checkpoints between blocks, and returns as each ends, so that
`filer-pack`'s `Xz`, which adapts it, resumes a read at the block before
it (PACK-19); `lzma-rust2` is now only the tests' oracle and encoder.

**Benchmarking the decoders (D63, 2026-11-04).** Each `decode-*` crate
keeps a `BENCH.md`: its decoder's figures alone, and against the
reference C implementation and the leading pure-Rust decoders, by one
protocol, `bench/METHOD.md`. In brief: public corpora (Silesia,
enwik8, Canterbury, a kernel tarball, executables) and synthetic edge
cases, encoded once by the reference encoder at levels across its range
and verified by SHA-256, with every decoded output checked; whole
streams, streaming in pieces, small streams, memory, checkpoints,
counters, and code size; 64-bit x86 (current AMD and Intel on bare
metal, and a VM) and ARM (Apple M4, and a Neoverse server), baseline
and native builds; independent processes with decoders interleaved,
medians with bootstrap confidence intervals, comparisons as geometric
means of paired ratios; and a release blocked by a regression past its
interval and 3%. The harness lives in `bench/`, a crate of its own
workspace, never published nor depended on: like the fuzzers (D50), it
may build C, there the counterparts' bindings, which the decoders
themselves never may (D45). Counterparts are run as black boxes, never
read.

**Our checksums are `compute-*` crates (D64, 2026-11-04).** The
checksums and hashes that formats stamp are crates of their own, as the
decoders are (D62), named for what they do, as theirs are:
`compute-<algorithm>`, a namespace free on crates.io. CRC-32 had come to
be written three times over, in `filer_api::checksum`, `decode-gzip`,
and `decode-xz`, each with its own tables in a build; `decode-bzip2`'s
CRC, `decode-xz`'s CRC-64 and SHA-256, and `decode-zstd`'s XXH64 were
each private to one decoder, and Adler-32 (zlib) and XXH32 (LZ4) were to
come. `compute-crc` holds every CRC, each a type and a feature, over one
engine of eight tables built at compile time: CRC-16/MODBUS, CRC-32,
CRC-32C, CRC-32/BZIP2, and CRC-64/XZ, each a `Copy` value, with its
register exposed for formats that chain it, as ext4 does. Since
2026-11-05 the five crates share one shape, as `filer-pack`'s `Digest`
and the hashes already had it: `update` feeds bytes in place, `chain`
feeds them by value and returns the next, and `finish` takes a
reference; before, the CRCs' and Adler-32's `update` returned the next.
`compute-sha256` (FIPS 180-4) and `compute-xxhash` (XXH32
and XXH64, from any seed) are crates of their own, as their algorithms
share nothing, and `compute-adler32` (RFC 1950) for `decode-zlib`,
written ahead of it, which sums 16 lanes, vectorized, and combines them
every 64 KiB: 25 GB/s on the M4, nearly three times `adler2`'s.
Reserved, each for the reader that needs it: `compute-blake2` for
btrfs's BLAKE2b and RAR 5's BLAKE2sp; and `compute-blake3` for ZFS's
BLAKE3. `compute-sha512`, reserved for ZFS's SHA-512/256 too, has SHA-512
and SHA-512/256 since 2026-11-05, its constants computed from their
definitions in FIPS 180-4; and `compute-fletcher` ZFS's Fletcher-4 and
Fletcher-2 and APFS's Fletcher-64 likewise, established by observation,
as neither ZFS's specification nor APFS's reference defines them, its
Fletcher-4 in four lanes whose sums combine by integer weights. ZFS's Skein and
Edon-R wait for a ZFS driver that needs them; a format's own name hashes
and sums, such as ext4's and XFS's directory hashes, stay in its driver.
Each is clean room, public domain, pure safe Rust, `no_std` without an
allocator, and dependency-free; the `decode-*` crates depend on them,
and `filer_api::checksum` re-exports `compute-crc`'s, which keeps D53.
Their ceiling by default is portable code; the CPU's own instructions
are behind a feature (D65), and their `BENCH.md` (D63) compares them with
both kinds of counterparts.

**The CPU's instructions, behind `intrinsics` (D65, 2026-11-05).** A
`compute-*` crate may take the CPU's dedicated instructions behind an
`intrinsics` feature, never enabled by default, so that by default it
stays dependency-free (D64). It depends then on `compute-multiversion`,
our fork of `multiversion` 0.9.2 with the fix of its pull request 67,
until upstream releases it: chosen at run time with `std`, and without
it, where the build enables them. The intrinsics are called from
functions given the instructions' target features, which makes them
safe, so the crate keeps `#![forbid(unsafe_code)]`; the dispatch's own
`unsafe` code is the macro's. Upstream's 0.9.2 cannot: where the build
enables a feature, as AArch64 does NEON and Apple's targets `crc` and
`sha2`, or `-C target-cpu=x86-64-v2` SSE4.2, its functions lacked it and
the intrinsics did not compile. Measured 2026-11-05: portable SIMD and
whole-crate multiversioning gained nothing, within 5% on Zen 1 built
for its own CPU, and `fearless_simd` reaches SHA-NI only with AVX-512
and ARM's SHA2 and CRC instructions not at all; the dedicated
instructions are what pay. `compute-crc` takes CRC-32C by SSE4.2's
`crc32`, and CRC-32, CRC-32C, and CRC-32/BZIP2 by ARM's `crc`
extension: 3.8 times on Zen 1, 4.7 times on Apple M4; and
`compute-sha256` SHA-256 by x86-64's SHA extensions and ARMv8's SHA2:
5.7 to 7.9 times on Zen 1, 6.4 times on Apple M4; and `compute-sha512`
SHA-512 by ARMv8.2's SHA-512 instructions, 2.5 times on Apple M4, with
x86-64's, on too few CPUs to test, unused. The decoders whose
checks they compute forward the feature: `decode-gzip` decodes a
quarter faster on Apple M4, `decode-xz` checked by SHA-256 a quarter
and by CRC-32 a tenth, and `decode-lzo` checked by CRC-32 an eighth; the
others gain nothing, their checksums having no instruction or too small
a share, and their own loops within 5% built for the CPU (2026-11-05).
`filer-api` forwards it for its `checksum`s, and `filer-pack` for those
and its decoders'; their `std` features forward to their dependencies',
without which the instructions would not be chosen at run time.

**Rotates under `x86-64-v2`** (measured 2026-11-05, Zen 1 in a VM).
Built with `-C target-cpu=x86-64-v2`, as some Linux distributions build
by default, the hashes made of rotates run far slower on AMD:
`compute-sha256` at 71 MB/s rather than 158, XXH32 at 2,640 rather than
4,511, and XXH64 at 4,711 rather than 6,430; the CRCs and Adler-32 as
before. Not our code: that CPU's tuning in LLVM has `fast-shld-rotate`,
which compiles each rotate to `shld` of a register with itself rather
than `rol`, faster on Intel since Sandy Bridge but slower on Zen;
`x86-64-v3`'s tuning has it not, nor any Zen's, and the same features
without the CPU run at full speed. Rust's rotates, or shifts and an OR,
all become LLVM's one rotate, so the source cannot choose the
instruction. `-C target-feature=-fast-shld-rotate` restores full speed,
with a warning that the feature is unstable; and `intrinsics` (D65)
spares SHA-256 on CPUs with SHA-NI, every Zen among them.

**Header reads** (measured 2026-11-03, PACK-10). `filer-tar`,
`filer-ar`, and `filer-cpio` read each header with a read of its own, and
cpio its name with another, where `filer-zip` reads its central
directory ahead in 64 KiB chunks. Opening archives of 20,000 one-byte
members (release build, Apple M4, macOS 26.6), from memory, from a
`File` on the blocking native driver, and on its thread pool:

| Archive | Reads | Memory | Blocking | Thread pool |
| --- | --- | --- | --- | --- |
| tar | 20,002 | 12 ms | 19 ms | 97 ms |
| ar | 20,001 | 4 ms | 10 ms | 92 ms |
| cpio | 40,003 | 4 ms | 17 ms | 180 ms |

A read through the blocking driver costs under half a microsecond more
than one from memory, but through the thread pool, which async programs
use, about 4.5 µs: opening then takes 8 to 40 times as long, and an
archive of the Linux kernel's sources (some 90,000 members) would spend
most of a second on it. That decides it: the parsers are to share a
forward reader, which reads ahead in chunks and jumps past what a header
says to skip, so that small members cost a fraction of a read each and
large ones one read each, as now (TODO PACK-11).

Done (PACK-11): `filer_pack::Reader` reads ahead from 16 KiB, doubling
to 64 KiB while what it read serves later reads, and quartering to 512
bytes when it served none, as when large members lie between headers.
tar, ar, and cpio read their headers through it, and ZIP its central
directory and its members' local headers. The same archives open in
314, 21, and 40 reads, through the thread pool in 15, 3, and 4 ms, as
from memory.

**Read-only, permanently (D43).** Archive drivers read archives as
directories and will never create or modify them: writing archives is out
of scope for this project, not merely deferred. It also does not fit the
driver model (no in-place rename or removal; ZIP writes its index last),
and dedicated archiving libraries already do it well. Every archive
crate's docs say so.

**More formats (2026-11-01).** Each format is its own crate, so that they
compose through `ReadAt` (a `.deb` is an ar archive holding compressed
TARs; an RPM is a header followed by a compressed cpio):

- `filer-ar`: flat ar archives (`.deb`, static `.a` libraries), with GNU
  and BSD long names and hidden symbol tables;
- `filer-cpio`: `odc`, `newc`, and old binary cpio (initramfs, RPM
  payloads), with hardlinks identified by device and inode number;
- `filer-rar`: RAR 4 and 5, reserved for now. RAR's compression is
  proprietary and the official `unrar` license forbids using its source to
  recreate the algorithm, so a reader must be a clean-room implementation
  of decompression only, as libarchive's is; solid archives also make
  reads at an offset costly;
- `filer-7z`: 7-Zip's archives, reserved for now (2026-11-03). The
  format is open, documented in the LZMA SDK, which its author placed in
  the public domain; the work is in pure-Rust decoders for its codecs
  (LZMA and LZMA2 above all, then PPMd, BZip2, Deflate, and the BCJ and
  BCJ2 filters; AES encryption would be `Unsupported`). Its index lies at
  the end, usually compressed itself, so even opening an archive takes
  a decoder, and solid archives make reads at an offset costly, as RAR's
  do.

`filer-ar` and `filer-cpio` are built (above); `filer-rar` and
`filer-7z` are published empty crates in `tmp/`, reserving their names
(D58).

**`filer-archive`** (D56, 2026-11-03) is to the archive drivers what
`filer-system` is to the native ones: one dependency, and one driver
type, `Archive`, whatever the format. `Archive::open(source, name)` picks
the format from the name's last extension (`Format::from_name`, ignoring
ASCII case: `.a`, `.deb`, `.lib`; `.cpio`; `.tar`; `.zip` and the
formats built on it, such as `.jar`, `.whl`, and `.docx`), and
`open_as` and `open_with` take it, and `Limits`, from the caller. The
format's own driver then opens the archive; `Archive` holds it as a
`DynDriver`, for one type at the cost of one boxed future per operation,
and labels the root with the name. Each format is a feature of the same
name, re-exporting its crate as a module (`filer_archive::zip`), and
`deflate` forwards `filer-zip`'s; `rar` and `7z` enable the empty
`filer-rar` and `filer-7z` (the latter as the module `sevenz`), which
`Format` will name once they read anything. Every archive crate is
added to it so. `Format` names every format
whatever the features, and opening a disabled one, or a name of none, is
`Unsupported` with a `FormatError` as the source. Detecting the format
from an archive's first bytes, for names that say nothing of it, such as
`initrd.img`, is ARC-01.

### 17.8 `filer-wasi`

The WASI driver, gated on `cfg(target_os = "wasi")` (D30); it exists as an
empty shell. WASI targets are not `cfg(unix)`, so `filer-posix` is empty
there and cannot be the WASI driver directly.

- `Dir`s come from preopens via `AmbientAuthority` (§6.1). Preopens already
  behave like capabilities, and WASI resolution is confined by the host, so
  `Enforcement` is reported per what the host guarantees.
- Executor: blocking (as `Posix::blocking()`, §17.1) where threads are
  unavailable, the shared `Spawner` where they are.
- Syscall layer: `rustix`'s WASI support if it covers what is needed,
  otherwise `std`'s WASI extensions or the `wasi` bindings (**VERIFY**
  `rustix` WASI coverage, VER-02). If that sharing works out, the syscall
  code may be factored so `filer-posix` and `filer-wasi` share it.
- Later: a native WASI 0.3 async binding once the ecosystem settles.

### 17.9 `filer-cli`

The `filer` binary, built only with `std` (§3.2). It uses the native
driver through `filer-system` (§17.10), obtains roots through
`AmbientAuthority`, and exposes the frontend's
operations as subcommands. It is also a living example and a manual test
tool for the drivers; it does nothing yet, and its command set is designed
when the frontend (API items in `TODO.md`) exists.

### 17.10 `filer-system`

The one dependency for programs replacing `std::fs` (D37). It re-exports
all of `filer_api` (answering Q5 for this crate: yes, since it is meant to
be the only import) and adds `System`, a newtype over the target's native
driver (`filer-posix` on Unix, `filer-windows` on Windows, later
`filer-wasi`), target-gated like `filer-cli`. Being a newtype rather than
an alias, it has one API everywhere (`System::blocking()`, `open_ambient`,
and a free `filer_system::open_ambient`). On targets without a driver yet,
including `wasm32-unknown-unknown`, `System` wraps a private stub that
fails every operation with `Unsupported`, so portable code compiles
everywhere and starts working as drivers land; nothing is emulated.
Without `std` it is just the re-export.

The broader umbrella of §3.8 (re-exporting `filer-heap`, middleware, and
the rest as modules, Q11) remains separate and may not be needed.

### 17.11 `filer-whatwg`

The browser driver, gated on `cfg(all(target_family = "wasm", target_os =
"unknown"))` and selected there by `filer-system`; it exists as an empty
shell. It implements the WHATWG File System Standard
(<https://fs.spec.whatwg.org/>), whose `FileSystemDirectoryHandle` and
`FileSystemFileHandle` are already capabilities: no global root, only
handles a page was given. Roots come from the Origin Private File System
(`navigator.storage.getDirectory()`) or from the File System Access picker
(`showDirectoryPicker()`, a WICG extension returning the same handle
types); the driver code is the same for both. The API is asynchronous on
the main thread; the faster `FileSystemSyncAccessHandle` exists only in
dedicated workers, so the driver will need both paths. It is for browsers
only: Node and Deno do not implement this API.

Naming (D38), decided 2026-11-01 after weighing three names:

- `filer-opfs` named one storage area, but the driver equally serves
  user-picked directories, which would make it a misnomer;
- `filer-web` named a platform, inviting Node, Deno, and Cloudflare Workers
  users (also `wasm32-unknown-unknown`) to expect it to work there;
- `filer-whatwg` names the specification the handle types come from, like
  `filer-posix` and `filer-wasi`, and promises an API rather than a place.
  Its costs (WHATWG is the body, not the standard's name; developers say
  "OPFS") are met by the crate's first doc line and its crates.io keywords
  (`opfs`, `browser`, `wasm`, `file-system-access`, `whatwg`).

### 17.12 Planned drivers

Recorded 2026-11-01 (D40); none exists yet:

- **`filer-uefi`:** UEFI's `EFI_SIMPLE_FILE_SYSTEM_PROTOCOL` on
  `*-unknown-uefi` targets, for bootloaders and firmware tools, selected
  there by `filer-system`.
- **Remote drivers:** `filer-9p` (9P2000.L, as used by WSL and
  `virtio-9p`), `filer-smb`, `filer-nfs`, and `filer-sftp`. They meet the
  network semantics of §21.5 head on (`Features::remote`, `Stale`, errors
  deferred to close) and are not selected by `filer-system`.
- **On-disk formats:** `filer-fat` (§17.13, in progress), `filer-exfat`
  (exFAT shares almost nothing on disk with FAT, so it is its own crate,
  D46), and, at low priority, other file systems such as littlefs, over a
  block device, for embedded and `no_std` use and for reading disk images.
- **Read-only file systems:** `filer-ext4` (ext2, ext3, and ext4;
  §17.15), `filer-ntfs` (§17.14), `filer-xfs` (§17.16), and
  `filer-squashfs`, `filer-cdfs` (ISO 9660), and `filer-udf` (read-only
  by design or for now; §17.19 to §17.21), first published as stubs to
  reserve their names; and as stubs still, in `tmp/` (D58, 2026-11-03),
  `filer-apfs`, `filer-btrfs`, `filer-minix`, `filer-refs`, `filer-ufs`
  (BSD's FFS), and
  `filer-zfs`, with
  `filer-exfat`. `LEGAL.md` governs them all: exFAT is not to be
  released before its patents expire (mid-2028); ReFS and APFS are
  read-only for good, ReFS best effort; LZO is never decoded from GPL
  code; UFS and Minix are clear (assessed 2026-11-03). Each would follow
  `filer-fat`'s model: a lazy driver over any `ReadAt` source, the async
  `Resolver`, panic-free by lint, fixtures from the real tools, and a
  differential test against the Linux kernel's driver. Writing them is out of scope for now: their
  journals, B+ trees, and checksums make safe writing far harder than
  FAT's. Each must refuse a volume with unknown incompatible features, and
  a dirty journal or log is either refused or replayed in memory, never
  read as if clean. Their crate docs list what each format needs. What
  they share lives in `filer-fs` (§17.17).

### 17.13 `filer-fat`

A driver for FAT12, FAT16, and FAT32 volumes (D46), read-only at first but
designed for writing later, and meant to be the most robust and most
freely licensed FAT implementation there is. It reads a bare volume (no
partition table) from any `ReadAt` source: a disk-image `File` of another
driver, bytes in memory, or a block device. It needs no OS, builds
`no_std`, and is pure Rust (D45).

**A lazy driver, not an index.** Unlike archives, a FAT volume is not
indexed at open: file data lies in cluster chains scattered across the
volume (so `archive::Member`, one contiguous extent, cannot describe it),
and volumes can hold millions of files. Opening parses only the boot
sector; directories and chains are read as operations need them. Path
resolution uses the generic `Resolver` (§6.5) over a private type whose
`lookup` scans one directory, so confinement and symlink policy (FAT has no
symlinks) come from the shared walker.

**On-disk facts the parser relies on** (Microsoft's FAT specification;
re-check each against it while implementing):

- The boot sector holds the BIOS parameter block: bytes per sector (512,
  1024, 2048, or 4096), sectors per cluster (a power of two up to 128),
  reserved sectors (at least 1), the number of FATs (usually 2), root
  entries (FAT12/16; 0 on FAT32), total sectors (16- or 32-bit field), FAT
  size (16-bit, or 32-bit on FAT32), and on FAT32 the root cluster and the
  extended flags (FAT mirroring, active FAT). Bytes 510 and 511 hold
  `0x55 0xAA`.
- The FAT type is decided by the count of data clusters, and nothing else:
  under 4085 is FAT12, under 65525 is FAT16, otherwise FAT32.
- FAT entries are 12 bits (packed, possibly straddling sectors), 16 bits,
  or 28 bits (the top 4 bits of a FAT32 entry are reserved). Clusters are
  numbered from 2; a next-cluster value is valid only within the volume;
  dedicated values mark free, bad, and end-of-chain clusters.
- Directory entries are 32 bytes: an 8.3 short name, attributes
  (read-only, hidden, system, volume label, directory, archive), Windows
  NT case flags, creation, access, and modification times, the first
  cluster (split into high and low halves on FAT32), and the size. First
  byte `0x00` ends a directory, `0xE5` marks a deleted entry, and `0x05`
  stands for a real `0xE5`. The FAT12/16 root directory is a fixed region;
  the FAT32 root is a chain.
- Long names (VFAT) precede their short entry as UTF-16 fragments of 13
  units, numbered in reverse, each carrying the short name's checksum. An
  orphaned or inconsistent long name is, per the specification, not part
  of the entry, which then has its short name.
- Times are local, with no time zone: modification at 2-second
  granularity, creation at 10 milliseconds, access as a date only.

**How it maps onto Filer:**

- Names: a long name is UTF-16; one with unpaired surrogates becomes a
  `ForeignName` holding WTF-8, so enumeration stays total. A short name
  without a long name is decoded from its OEM code page, which the volume
  does not record: 437 by default, as in the Linux driver, or another
  `CodePage` from `Options` (with the `code-pages` feature, D49). Decoding
  keeps every short name UTF-8 (a `ForeignName` must not be UTF-8), the
  same name every time; a byte the chosen page leaves undefined is
  `Corrupt`, naming the slot, rather than replaced. Lookups are
  case-insensitive, as FAT preserves case but does not distinguish it: the
  first driver whose name matching is not byte-exact, informing
  `NamingRules`.
- Metadata: kind, length, the three times (`Timestamp::granularity` set;
  local times read at a fixed `UtcOffset` from `Options`, UTC by default,
  as Linux's `time_offset`: following daylight saving time would need
  zone rules and leaves an hour ambiguous each year), and ids: the volume
  serial number (0 without an extended boot record) and each entry's
  location, since empty files have no first cluster. A directory's length
  is its chain's bytes, or the fixed root's size, as Linux reports; a stat
  by path walks the chain only if the mask asks for the length.
  Attributes (read-only, hidden, system, archive) await `FileAttrs`: the
  last three are Windows attributes it does not yet have (§11.2), shared
  with `filer-windows`; read-only is a permission.
- Listings: a `DirCookie` is an entry index; `.` and `..` are hidden, as
  are deleted entries and volume labels.
- Reads: a handle holds its file's run list (first cluster and length of
  each contiguous run), built and validated once at open, so reads merge
  contiguous clusters into one source read and never re-walk the chain.
  No locks are needed: the run list is immutable.

**Robustness.** A corrupt or hostile image must never cause a panic, a
hang, or unbounded memory use. The crate denies `unwrap`, `expect`,
`panic!`, unchecked indexing, and unchecked arithmetic by lint, so this
holds by construction. Chains are bounded by the cluster count (catching
loops), next-cluster values out of range or marked bad are errors,
directories are bounded (the specification allows 65,536 entries), a file
whose chain is shorter than its size fails to open, and every such error
is `Corrupt` with a source naming what and where. Nothing is silently
skipped: an inconsistent long name falls back to the short name only
because the specification defines it so. A fuzz target covers the parsers.

**Verification.** Besides unit tests, the driver runs `filer-spec` on
images its test writer lays out (contiguous and fragmented, for each FAT
type), reads fixtures made by `mkfs.fat` and `mtools` (stored sparsely),
and `tests/differential.sh` compares it with the Linux kernel's FAT driver
on loop-mounted images, including volumes Linux wrote itself: names
in directory order, kinds, sizes, contents, and all three times must be
identical. The `fat-manifest` example prints a volume through `Fat` layered
over a native `File`, which is what the comparison reads. `tests/fuzz.rs`,
a dependency-free, structure-aware fuzzer, mutates the fixtures and
writer images where their structures are and walks each result, checking
that errors are only `Corrupt` and that listings, stats, and reads agree;
it runs briefly in every `cargo test` and for as long as asked by
environment variable. `fuzz/` has `cargo-fuzz` targets (D50) that lay
their input over the boot sector, the FAT, and the root directory of a
valid volume, and run the same walk.

**Ready for writing.** The layers are the boot sector and geometry
(validated once, immutable), the FAT table (entry reads now, writes
later), directory entries (decoding now, encoding later, kept symmetric),
and the driver. Writing will add: allocation with the FAT mirrored to
every copy, the FSInfo free-count hints, the volume dirty bit around
mutations, ordering of writes so a crash leaves at most lost clusters
rather than cross-linked files (FAT has no journal; see §15), and
in-process locking of shared state (§7.7).

**Discard (TRIM) (D51).** Writing must design discard in from the start.
The split: only the filesystem driver knows which ranges stop being used;
only the storage below knows how to drop them. So the writable source
trait (a `WriteAt` beside `ReadAt`, with a durability barrier) carries an
optional `deallocate(range)`: afterwards the range's contents are
unspecified. Each source maps it on its own, one layer at a time:

- a disk-image `File` of another driver punches a hole (`punch_hole`,
  §12: `fallocate(PUNCH_HOLE)`, `F_PUNCHHOLE`, `FSCTL_SET_ZERO_DATA`);
- a raw block device discards (`BLKDISCARD` on Linux, `DKIOCUNMAP` on
  macOS, the trim data-set management action on Windows);
- a partition window (backlog) translates and clips to its partition;
- memory ignores it or zeroes.

This composes: a FAT image on ext4 on an SSD frees clusters, ext4 frees
the punched blocks, and ext4's own discard reaches the device; no layer
reaches past the one below. The source reports whether `deallocate` works
on this medium (`Unsupported` is a normal answer, not an error), its
granularity and alignment (the driver coalesces free clusters into runs
and discards only whole aligned units), and whether deallocated ranges
read back as zeros (holes do; SSD TRIM need not). The filesystem never
relies on that last one: it still zeroes new directory clusters.

The hazards are orderings, all inside the filesystem driver:

1. Free first: mark the clusters free in every FAT copy and pass the
   barrier before discarding them, or a crash leaves a chain pointing at
   lost contents (worse than FAT's usual lost clusters).
2. No allocation may race a discard: a batch trim reserves each free run,
   discards it, awaits completion (devices reorder queued requests), and
   only then releases it to the allocator, as the Linux kernel's ext4
   locks a block group during `FITRIM`.
3. Open handles hold immutable run lists, so a file deleted while open
   frees (and discards) its clusters only when its last handle closes,
   as the Linux kernel defers freeing an unlinked open inode.
4. Discard is not erasure: SSD TRIM is advisory and old contents may
   survive. It is never documented or offered as a privacy feature.

Policy follows Linux: online discard (per free) is off by default, as an
`Options` setting, since many small discards hurt some devices; an
explicit batch trim, like `fstrim`, scans the durable FAT's free
clusters and reports the bytes discarded. Both need `WRITE` and `MUTATE`
rights on the volume. A driver-generic trim operation can follow later;
on POSIX it would wrap `FITRIM`, which needs `CAP_SYS_ADMIN`.

**Backlog, not now:** partitioned images (MBR and GPT), best served by a
general window adapter over any `ReadAt`; exFAT, in `filer-exfat`.

### 17.14 `filer-ntfs`

A read-only driver for NTFS volumes (versions 3.0 and 3.1, as written
since Windows 2000), following `filer-fat`'s model (§17.13): a lazy
driver over any `ReadAt` source, the async `Resolver`, panic-free by lint
(D47), pure Rust (D45), fixtures from the real tools, and a differential
test against the Linux kernel's `ntfs3` driver. Writing is out of scope.

**Clean room (D52).** NTFS has no official on-disk specification, and the
mature implementations (ntfs-3g, the Linux kernel's `ntfs3` and `ntfs`)
are GPL, which this public-domain project cannot take code from. The
driver is written from public descriptions of the format only, and no
GPL source code is consulted while writing it: Microsoft's open
specifications [MS-FSCC] (attribute, file-attribute, and reparse-point
layouts) and [MS-XCA] (LZNT1), and the Linux-NTFS project's *NTFS
Documentation* (a description of the on-disk format, not code; its
GFDL, like the GPL-2.0 on the kernel's own documents, binds the prose,
not the facts read from it: see `LEGAL.md`, *Documentation and the
clean room*).

**On-disk facts the parser relies on:**

- The boot sector: `NTFS    ` at byte 3, bytes per sector, sectors per
  cluster (a byte above 0x80 encodes 2^(256 - n), for clusters over 64
  KiB), total sectors, the clusters of `$MFT` and `$MFTMirr`, and the
  size of MFT records and index records (a positive byte counts
  clusters; a negative one n means 2^-n bytes), and the serial number.
- The master file table (MFT): fixed-size records, `FILE` signed, each
  protected by an update sequence array: the last two bytes of every
  512-byte stride hold the update sequence number and are restored from
  the array, so a torn write is detected. Records 0 to 15 are system
  files: `$MFT` (0), `$MFTMirr`, `$LogFile`, `$Volume`, `$AttrDef`, the
  root directory (5), `$Bitmap`, `$Boot`, `$BadClus`, `$Secure`,
  `$UpCase` (10), and `$Extend` (11). `$MFT` is itself a file, so records
  are read through its own `$DATA` runs.
- Attributes: typed (`$STANDARD_INFORMATION`, `$ATTRIBUTE_LIST`,
  `$FILE_NAME`, `$DATA`, `$INDEX_ROOT`, `$INDEX_ALLOCATION`, `$BITMAP`,
  `$REPARSE_POINT`, ...), optionally named, resident (value inline) or
  non-resident (data runs: a header byte giving the sizes of a length and
  a signed, relative cluster offset; no offset means a sparse run). Data
  past the initialized size reads as zeros. A file whose attributes do
  not fit one record lists them, across extension records, in
  `$ATTRIBUTE_LIST`.
- Directories: `$I30` indexes of `$FILE_NAME` keys, a B+ tree whose root
  is resident (`$INDEX_ROOT`) and whose other nodes are `INDX` records
  (also fixed up) in `$INDEX_ALLOCATION`, collated by the volume's
  `$UpCase` table. A name is in the POSIX, Win32, DOS (8.3), or Win32 and
  DOS namespace; a file with a long and a short name has two entries.
- Times: 100-nanosecond intervals since 1601, UTC.
- Compression: LZNT1 over compression units (usually 16 clusters); a
  unit whose runs are shorter than the unit, then sparse, is compressed.

**How it maps onto Filer:**

- Ids: the MFT record number, with the volume serial number, so hard
  links share an id, as they should.
- Names: UTF-16, unpaired surrogates as WTF-8 foreign names. Listings
  show each file once, skipping DOS names that shadow a Win32 name, and
  hide the system files (records below 16). Lookups match exactly, else
  case-insensitively by `$UpCase`, as on Windows.
- Metadata: from `$STANDARD_INFORMATION`: modified, changed (the MFT
  record's change time), created, and accessed, at 100 ns; length from
  the unnamed `$DATA`.
- Reparse points: symbolic links, junctions, and WSL symlinks become
  symlinks, confined by the `Resolver`. A relative target's `\` become
  `/`; an absolute one on a drive (`\??\C:\path`) becomes `/path`, from
  the volume's root, which is assumed to be that drive: resolving it needs
  `Symlinks::InRoot`, and under `Beneath` it is `EscapeBlocked`, as any
  absolute target is. Other absolute forms (UNC, volume GUID paths) keep
  their text and dangle. Reparse points of other tags (cloud
  placeholders, files compressed by WOF) and encrypted files keep their
  kind and metadata, but opening them to read is `Unsupported`, as their
  `$DATA` is not their contents.
- Interix special files, as Services for UNIX and ntfs-3g keep them, are
  what they say: system files that are empty (FIFOs), of one byte
  (sockets), `IntxLNK\1` and a UTF-16 target (symlinks), or `IntxCHR\0`
  or `IntxBLK\0` and two `u64` device numbers (devices). Only a node's
  contents tell, so a listing reports no kind for a system file. The
  forms were taken from what ntfs-3g reads and writes, observed through
  it, not from its code.
- Named data streams are extended attributes in the `user` namespace, as
  ntfs-3g has them (`file:stream` is `user.stream`), listed once however
  many extents they have, and matched as names are. Other namespaces are
  empty. `$STANDARD_INFORMATION`'s file attributes are `FileAttrs`:
  hidden, system, archive, sparse, compressed, offline, encrypted, and
  recall on open or on data access as dataless; read-only is a permission
  (not reported yet), and directory and reparse point are the kind.
- A volume marked dirty (its `$LogFile` not replayed) is refused unless
  the caller opts in: it is never read as if clean (§17.12).

**Robustness** follows §17.13: every count, offset, and run is checked;
run lists, attribute lists, and index trees are bounded (a loop in the
index or in extension records is an error, not a hang); fixups are
verified before a record is used.

**Verification.** Fixtures in four layouts (4 KiB, 512-byte, and 64 KiB
clusters, and 4 KiB sectors with 4 KiB records), made by `mkntfs` and
filled through ntfs-3g, are read in tests; one more holds files ntfs-3g
compressed, another links of every kind (reparse points and Interix
files, with FIFOs and sockets made by ntfs-3g), another named streams
(100 of them on one file, so listed in `$ATTRIBUTE_LIST`) and file
attributes, and another `filer-spec`'s tree.

Windows itself makes four more (4 KiB, 512-byte, 64 KiB, and 128 KiB
clusters), in Windows 11 run by `dockur/windows` under KVM
(`tests/fixtures/make-windows.sh`): long names with the DOS names
Windows makes, Unicode, fragmented, sparse, and compressed files, hard
links, symbolic links and junctions, streams on files and directories,
attributes, and in the 4 KiB one a 1000-entry directory (an index of
several levels), names chosen to test collation (case, accents, `ß`,
`ǅ`, `İ` and `ı`, punctuation), 100 streams and 40 hard links on one
file, a directory with per-directory case sensitivity, an encrypted file,
and a file compressed by WOF. Windows writes its own manifest of each,
and `tests/windows.rs` requires Filer's to equal it line by line, in
Windows' listing order, so collation order is checked too; only access
times (which Windows' reading updates), `System Volume Information`
(which Windows keeps from administrators), and what Filer refuses to
read (encrypted and WOF-compressed contents, and WOF's sparse flag, which
Windows hides) are left out. These found a bug no ntfs-3g fixture could:
a compression unit stored as is, followed in the same run by a
compressed one, as Windows writes them, was read on into the next.

`tests/differential.sh` compares every fixture, and fresh ones, with the
Linux kernel's `ntfs3`: every name, kind, size, content, and time agrees;
and with ntfs-3g, which has streams as attributes as `ntfs3` does not:
every flag and attribute agrees (streams over Linux's 64 KiB limit on
attribute values are read as `file:stream` instead). Listings are
compared sorted, as the order differs by design: Filer lists an index in
collation order, as Windows does, where `ntfs3` lists a node's separating
entries after its children. Reparse points differ by design too: `ntfs3`
keeps a drive in absolute targets (`C:/path`), makes junctions `./path`,
reads WSL symlinks as files and other tags' `$DATA` as contents, and
decompresses WOF files, which Filer refuses; the script lists those
entries and leaves them to the fixture tests. `ntfs3` reads Interix files
as regular files, so those are compared with ntfs-3g only.
`tests/external.sh` tests against other projects' disk images
(`CREDITS.md`), never committed, fetched at a pinned commit, checked by
their SHA-256 sums, and their NTFS partitions unpacked: walked, compared
with `ntfs3` and ntfs-3g, and checked for the artifacts their makers
describe (times taken from records, not index entries; no entry from MFT
slack; no slack in any file's contents; clusters of 2 MiB).
`tests/fuzz.rs` mutates the fixtures' boot sectors, MFTs, and index
records, Windows' among them, and walks each result, as for `filer-fat`,
and `fuzz/` has `cargo-fuzz` targets for the boot sector, the MFT,
indexes, and compressed data (D50).

### 17.15 `filer-ext4`

A read-only driver for ext2, ext3, and ext4 volumes, following
`filer-fat`'s model (§17.13): a lazy driver over any `ReadAt` source, the
async `Resolver`, panic-free by lint (D47), pure Rust (D45), fixtures from
the real tools, and a differential test against the Linux kernel's `ext4`
driver, which mounts all three. Writing is out of scope. It is built in
the order the format grew: ext2 first, then ext3 (the journal and hashed
directories), then ext4's features one by one, each refused by its
feature flag until it lands.

**Clean room (D52).** The Linux kernel's `ext4` and `jbd2`, e2fsprogs,
and GRUB are GPL. The driver is written from descriptions of the format
only: the kernel's prose documentation (*ext4 Data Structures and
Algorithms*, `Documentation/filesystems/ext4/`, which also describes
JBD2) and Dave Poirier's *The Second Extended File System*; and from what
`mke2fs`, `debugfs`, and `dumpe2fs` show of real volumes. No
implementation's source code is consulted. The directory hashes, which
only code defines exactly, are built from RFC 1320 (MD4) and the
published TEA cipher, and checked against `debugfs`'s `dx_hash`.

**On-disk facts the parser relies on:**

- The superblock: 1024 bytes at byte 1024, magic `0xEF53` at 0x38; block
  size 2^(10 + n), 1 KiB to 64 KiB; the first data block (1 for 1 KiB
  blocks, else 0); blocks, inodes, and the per-group counts of both;
  revision 0 (128-byte inodes, first inode 11, no features) or 1
  (`s_inode_size`, `s_first_ino`, and three feature sets). An unknown
  incompatible feature must stop a reader; read-only-compatible ones may
  not, as a reader writes nothing.
- Block groups: their descriptors follow the superblock's block, 32 bytes
  each (with `64bit`, a power of two from 64, holding the high halves of
  block numbers, as the superblock holds its count's); each names its
  inode table, which with `flex_bg` may lie in another group. Under
  `meta_bg`, from `s_first_meta_bg` on, each metagroup (the groups one
  block of descriptors covers) keeps its descriptors in its first
  group's first block, past any superblock backup there: groups 0, 1,
  and powers of 3, 5, and 7 with `sparse_super`, the two the superblock
  names with `sparse_super2`, else every group. `huge_file` adds 16 bits
  to inodes' block counts, in blocks rather than sectors for inodes with
  its flag. Under `bigalloc`, blocks are allocated in clusters of 2^n,
  whose count a group's bitmap bounds, and data starts with block 0
  though the superblock is in block 1 of 1 KiB blocks, where group 0's
  `meta_bg` descriptors then are one block further. `large_dir` allows
  indexes three levels deep, `mmp` and `orphan_file` concern only
  writers, and `verity` files read as they are, their Merkle trees past
  their ends.
- Checksums, verified, each failure `Corrupt` (`csum.rs` details each):
  under `metadata_csum`, CRC-32C registers (before the final inversion)
  chained from the volume's seed (the register over the UUID, or with
  `metadata_csum_seed` the superblock's own), over the superblock, group
  descriptors (their low 16 bits), inodes, extent tree nodes, directory
  blocks (in an unused last entry) and index nodes (in a tail after
  their entries), and attribute blocks; those of one inode's structures
  chained further over its number and generation. Under `gdt_csum`,
  CRC-16/MODBUS over group descriptors only. Which coverage an index
  node's checksum has was settled against real volumes, as the
  documentation leaves it open.
- Inodes: number n is entry (n - 1) mod `s_inodes_per_group` of group
  (n - 1) / `s_inodes_per_group`; there is no inode 0, and 2 is the root.
  The 128-byte base holds the mode (kind and permissions), owner, size
  (64 bits, the high half once called `i_dir_acl`), four times as signed
  32-bit seconds, links, flags, `i_block` (60 bytes), and the extended
  attribute block; larger inodes add `i_extra_isize` bytes of fields
  (each time's nanoseconds and two epoch bits, the creation time), then
  in-inode extended attributes.
- `i_block` is, by kind and flags: 12 direct, one indirect, one double
  and one triple indirect block numbers (0 is a hole); an extent tree
  (ext4, the `extents` flag: a header of magic `0xF30A`, then four
  entries, interior nodes of indexes and leaves of extents in blocks
  below, at most five levels; an extent longer than 32768 is allocated
  but unwritten, and reads as zeros, as do blocks no extent holds);
  inline data (ext4, `inline_data` and the inode's flag: a file's bytes,
  continued in the in-inode attribute `system.data`, which listings
  hide, as Linux's do; a directory's parent's inode number, then entries
  as a block holds them, continued likewise); a symlink target under 60
  bytes; or a device number.
- Directories: blocks of entries (inode, record length, name length,
  and with `filetype` a type byte), never split across blocks; inode 0
  marks an unused entry. Hashed (htree) directories hide their index in
  blocks that read as one unused entry, so a linear read sees exactly the
  entries; lookups descend the index (legacy, half-MD4, or TEA hashes,
  over signed or unsigned bytes, by `s_flags`; up to two levels of
  interior nodes), reading on into blocks whose hash has the
  continuation bit, and fall back to the linear read for an index they
  cannot follow.
- Extended attributes: in-inode (after `i_extra_isize`, magic
  `0xEA020000`) and in one block (`i_file_acl`, a 32-byte header); entries
  name a prefix by index (`user.`, `trusted.`, `security.`, `system.`,
  and the POSIX ACLs), and values sit at offsets in the same region, or
  under `ea_inode` in the data of an inode of their own (a file with the
  attribute inode flag, of the value's size, at most 16 MiB), which no
  directory names.

**How it maps onto Filer:**

- Ids: the inode number, with a volume id folded from the UUID, so hard
  links share an id.
- Names are bytes: UTF-8 ones are names, others `ForeignName`s. Lookups
  match bytes exactly. Listings are in directory order, skip `.`, `..`,
  and unused entries, and take their kind from the type byte (none
  without `filetype`); a `DirCookie` is the byte offset of the next
  entry, so a listing resumes wherever a cookie points, never mid-entry.
- Metadata: kind and length (`i_size`, for directories too, as Linux
  reports) from the inode; modified, changed, and accessed times, and
  created where the inode has room for it, at 1 s, or 1 ns with the extra
  fields; flags: immutable, append-only, compressed, encrypted, and
  verity. FIFOs, sockets, and devices are what they say, and do not open.
- Symlinks: fast (the target in `i_block`) or slow (in data blocks),
  confined by the `Resolver`.
- Extended attributes: both places, in their namespaces; POSIX ACLs are
  converted from the on-disk form to the one Linux's `getxattr` returns.
- Journals: a clean ext3 or ext4 journal changes nothing for a reader.
  A volume that needs recovery (`needs_recovery`, as a machine losing
  power leaves it) is read as replaying the journal leaves it, never as
  if clean (§17.12): its committed transactions, in order, are laid over
  the source through `filer_fs::recover` as XFS lays its log's (§17.16),
  but for the blocks later transactions revoke; `csum_v3` checksums are
  verified, and a torn commit ends the log. A journal on another device,
  or using `csum_v2`, asynchronous commits, or fast commits, is refused.
  `Options::allow_dirty` reads such a volume as it is instead, which may
  be inconsistent.
- Encrypted files and directories keep their kind and metadata, but
  reading their contents or names is `Unsupported`. Case-folded
  directories match names exactly (case-insensitive lookups wait for
  Unicode folding tables).

**Robustness** follows §17.13: every block number is checked against the
volume before it is read; indirect trees are bounded by their depth and
by the file's size; directory entries must fit their block and their
record; inodes, groups, and attribute entries are range-checked. Reads
map blocks lazily, a read's range at a time, merging contiguous blocks
into one source read, so no file's whole map is ever held.

**Verification.** `tests/fixtures/make.sh` formats volumes with
`mke2fs` (revisions 0 and 1; 1, 2, and 4 KiB blocks; 128-byte inodes;
and 64 KiB blocks, which x86 Linux cannot mount, so that one is filled
by `mke2fs -d`), fills them through the kernel (hard links, holes deep
enough for triple indirect blocks, fast and slow symlinks, special files,
names that are not UTF-8, hashed directories), and stores them sparsely;
`filer-spec` runs on its own tree. Two more hold extended
attributes in every namespace and place (in the inode, in a block, in a
block two files share, POSIX ACLs, a name that is not UTF-8) and inode
flags; two of a hashed directory of 700 long names, by TEA over
unsigned bytes two levels deep, and by the legacy hash over signed ones,
whose every name the index finds as the linear read lists it; ext3
volumes of 1 and 4 KiB blocks, with and without extents; ext4 volumes
as `mke2fs -t ext4` makes them, of 1 and 4 KiB blocks, one with
`meta_bg` of two metagroups, one with `gdt_csum` instead of
`metadata_csum`, and, checksummed, one of attributes, one of a hashed
directory two levels deep, and one of deep extent trees, whose every
checksummed structure, changed, is `Corrupt`; two with `inline_data`,
one of a directory and files continued past `i_block`; one with
`bigalloc`, one with `large_dir`, `mmp`, and `sparse_super2`, one of a
file fs-verity seals, and one of attribute values in inodes of their
own; and one of a
file of 2000 extents two levels deep and one allocated but unwritten
but for its middle; and three that need
recovery, of 1 and 4 KiB blocks and with `metadata_csum`, copied while
mounted after transactions committed but before they were written back
(files made, renamed, and removed; a directory and an indirect block
removed, revoking their blocks). `tests/differential.sh` compares each,
and fresh ones, with the kernel, recovering those that need it, and all
agree: names, kinds, sizes, contents, link targets, all four times to
the nanosecond, flags, and extended attributes. Listings
are compared sorted, as the order differs by design: Linux lists hashed
directories in hash order, Filer every directory in block order.
`tests/fixtures/dx_hash.txt` holds what `debugfs dx_hash` computes for
every hash version, seeded and not, over names of every length a block
of the hash matters to. `tests/fuzz.rs` mutates the fixtures'
superblocks, descriptors, inode tables, root directories, hash indexes,
extent trees, journals, and attribute blocks and walks each result,
as for `filer-fat`, and `fuzz/` has `cargo-fuzz` targets for the same
structures (D50).

### 17.16 `filer-xfs`

A read-only driver for XFS volumes, v4 and v5, following `filer-fat`'s
model (§17.13): a lazy driver over any `ReadAt` source, the async
`Resolver`, panic-free by lint (D47), pure Rust (D45), fixtures from the
real tools, and a differential test against the Linux kernel's `xfs`
driver. Writing is out of scope. It is built in the order the format
grew: v4 volumes first (the oldest that `mkfs.xfs` still makes, deprecated
but mounted by Linux 7.0), then v5's self-describing metadata and CRCs,
then v5's later features one by one, each refused by its feature flag
until it lands. Volumes without version 2 directories, 32-bit link
counts, or unwritten extent flags, older than any tool that still runs,
are refused.

**Clean room (D52).** The Linux kernel's `xfs` and xfsprogs (`libxfs`)
are GPL. The driver is written from descriptions of the format only: the
*XFS Algorithms & Data Structures* book (the `xfs-documentation`
repository) and what `mkfs.xfs`, `xfs_db`, `xfs_info`, and `xfs_logprint`
show of real volumes. No implementation's source code is consulted.
Where the book names a flag but not its value, the value is found by
setting the bit with `xfs_db -x` and reading back the name `xfs_db`
prints (done 2026-11-01 for the superblock's feature sets).

**On-disk facts the parser relies on:**

- Everything is big-endian but CRCs (little-endian) and log payloads
  (the writer's order). Sizes are powers of two, recorded with their
  logarithms.
- The superblock: sector 0 of each allocation group (AG); AG 0's is
  primary. Magic `XFSB`; block size 512 B to 64 KiB; sector size, inode
  size (256 B to 2 KiB, at most a block), and inodes per block; AG size
  (`sb_agblocks`, with `sb_agblklog` its logarithm rounded up) and count;
  `sb_dblocks`; the root inode; the directory block size (2^`dirblklog`
  blocks, at most 64 KiB); the internal log's start and length. The low
  nibble of `sb_versionnum` is 4 (v4) or 5 (v5).
- v4 features are bits of `sb_versionnum` above the nibble: `ATTR`
  0x10, `NLINK` 0x20 (required), `QUOTA` 0x40, `ALIGN` 0x80, `DALIGN`
  0x100, `SHARED` 0x200 (refused), `LOGV2` 0x400, `SECTOR` 0x800,
  `EXTFLG` 0x1000 and `DIRV2` 0x2000 (both required), `ASCII_CI`
  0x4000, `MOREBITS` 0x8000; and, with `MOREBITS`, of `sb_features2`
  (mirrored in `sb_bad_features2`): `LAZYSBCOUNT` 0x2, `ATTR2` 0x8,
  `PROJID32BIT` 0x80, `CRC` 0x100 (v5 only), `FTYPE` 0x200.
- v5 adds four sets: compatible (none defined), read-only compatible
  (`FINOBT` 0x1, `RMAPBT` 0x2, `REFLINK` 0x4, `INOBTCNT` 0x8; a reader
  may ignore them), incompatible (`FTYPE` 0x1, `SPINODES` 0x2,
  `META_UUID` 0x4, `BIGTIME` 0x8, `NEEDSREPAIR` 0x10, `NREXT64` 0x20,
  `EXCHANGE` 0x40, `PARENT` 0x80, `METADIR` 0x100, `ZONED` 0x200), and
  log-incompatible. Unknown incompatible bits in any set must stop a
  reader. `mkfs.xfs` 6.18 sets `PARENT` and `METADIR` only with
  `EXCHANGE`; `METADIR` keeps metadata inodes (flagged 0x20 in
  `di_flags2`) in a tree of their own from `sb_metadirino`, which the
  root's never reaches, and splits a realtime device into groups.
  `ZONED`, a realtime section within the data device, is refused.
- Block numbers in file maps pack the AG above `sb_agblklog` bits of AG
  block; the byte offset is (AG × `sb_agblocks` + AG block) × block size.
  Inode numbers pack the AG, AG block, and slot in the block likewise
  (`sb_agblklog` + `sb_inopblog` bits below the AG), so an inode is found
  without the inode B+ trees.
- Inodes: magic `IN`; version 1 or 2 (v4; 1 only with a 16-bit link
  count) or 3 (v5); a 100-byte core (176 with v3) holding the mode,
  owner, links, times (signed 32-bit seconds and nanoseconds, or with
  `BIGTIME`, flagged 0x8 in `di_flags2`, unsigned 64-bit nanoseconds since
  1901-12-13T20:45:52Z), size, extent counts (32 bits at 76 and 16 at
  80, or with `NREXT64`, flagged 0x10, 64 bits at 24 and 32 at 76), flags,
  the data fork's format, and the attribute fork's offset (`di_forkoff`
  × 8 into the literal area after the core) and format. v3 adds the
  CRC (at 100, over the whole inode), `di_flags2` (120), the creation
  time (144), the inode's own number (152), and the UUID (160); its
  literal area starts at 176. Fork formats: device, local (in the
  inode), extents, B+ tree.
- File maps: 128-bit extent records (unwritten flag, 54-bit file block,
  52-bit volume block, 21-bit length), sorted and disjoint; unwritten
  extents read as zeros, and gaps are holes. A `BMAP` (v5 `BMA3`) B+ tree
  is rooted in the fork (level, record count, keys, then pointers at the
  offset the fork's capacity sets) with 64-bit pointers; its blocks carry
  a 24-byte header (72 with v5).
- Directories (version 2): short form in the inode (a 32- or 64-bit
  parent, and entries of name, offset tag, optional type, and 32- or
  64-bit inode); else data blocks of the directory block size at fork
  offsets below 32 GiB, holding 8-byte-aligned entries (inode, name,
  optional type, tag) and unused regions (`0xFFFF`, length), with `.`
  and `..` explicit. One block holds everything (`XD2B`), or data blocks
  (`XD2D`) are indexed by hash leaves at 32 GiB (`0xD2F1`, or
  `0xD2FF` under `0xFEBE` nodes) and free-space blocks at 64 GiB; a
  linear read of the data blocks sees exactly the entries. Leaves hold a
  hash and an address in 8-byte units of the data; nodes a hash, the
  greatest below, and a fork block; leaves of one level are chained.
- The name hash: four bytes at a time, shifted by 21, 14, 7, and 0
  bits, XORed with the hash so far rotated left by 28; a last group of
  fewer is shifted less, and the hash rotated by 7 bits for each.
  `ASCII_CI` folds bytes 0x41 to 0x5A and 0xC0 to 0xDE but 0xD7 to lower
  case, Latin-1's letters too, both to hash and to compare.
- Symlinks: local, or in blocks of the data fork; with v5 each extent's
  first block starts with an `XSLM` header.
- Extended attributes: short form in the attribute fork (size, count,
  then entries of name and value lengths, flags, name, and value); else
  fork blocks, one leaf (`0xFBEE`) or nodes (`0xFEBE`) over leaves,
  whose entries name a hash, the offset of the name, and flags, and hold
  a value or name its first block and length. Flags: local 0x1, `root`
  (`trusted`) 0x2, `secure` 0x4, parent 0x8, incomplete 0x80. ACLs are
  `root` attributes `SGI_ACL_FILE` and `SGI_ACL_DEFAULT`: a count, then
  big-endian entries of tag, id, and permissions; Linux 7.0 lists each
  both so and converted. `attr1` and `attr2` differ only in how
  `di_forkoff` is chosen.
- v5 metadata blocks are self-describing: their magic, the volume's UUID
  (or `sb_meta_uuid` with `META_UUID`, which `xfs_admin -U` sets), their
  own disk address in 512-byte units, their owner (inode or AG), and a
  CRC32C of the block with the CRC field zeroed, stored little-endian.
  The fields' offsets (CRC, address, UUID, owner): `BMA3` 64, 24, 40,
  56, after a 72-byte header; `XDB3` and `XDD3` 4, 8, 24, 40, with
  entries from 64; leaves and nodes (0x3DF1, 0x3DFF, 0x3EBE, 0x3BEE) 12,
  16, 32, 48, their counts at 56, entries from 64 (attribute leaves from
  80); `XARM` and `XSLM` 12, 40, 16, 32, data from 56. A remote value
  has a header in each block, a symlink target in each extent, whose
  CRC covers it all. The superblock's CRC covers its whole sector.
- The log: a circle of 512-byte sectors, each stamped with its cycle
  number; records with `0xFEEDBABE` headers. A volume whose last record
  is not an unmount record with nothing pending needs recovery.

**How it maps onto Filer:**

- Ids: the inode number, with a volume id folded from the UUID, so hard
  links share an id.
- Names are bytes: UTF-8 ones are names, others `ForeignName`s. Lookups
  match bytes exactly (folded to lower case with `ASCII_CI`), by hash
  in a directory of leaves, falling back to a scan if the index is
  inconsistent; so are attributes.
  Listings are in directory order, skip `.`, `..`, and unused space, and
  take their kind from the type byte (none without `FTYPE`); a
  `DirCookie` is the directory offset of the next entry (its tag in a
  short-form directory), so a listing resumes wherever a cookie points.
- Metadata: kind and length (`di_size`) from the inode; modified,
  changed, and accessed times at 1 ns, and created with v3 inodes;
  flags: immutable, append-only, and DAX. FIFOs, sockets, and devices
  are what they say, and do not open.
- Symlinks: local or remote, confined by the `Resolver`.
- Extended attributes: short form, leaf, node, and B+ tree attribute
  forks, with remote values, in their namespaces; POSIX ACLs are
  also converted to the form Linux's `getxattr` returns; parent pointers
  (`PARENT`) are not attributes and stay hidden, as Linux hides them.
- Shared (reflinked) extents are read like any other. Files on a
  realtime subvolume keep their metadata, but their contents are
  `Unsupported`: the driver reads one source, the data device.
- The log: a clean log changes nothing for a reader. A volume that needs
  recovery is read as recovery leaves it, its log replayed in memory,
  never read as if clean (§17.12): committed transactions from the tail,
  their buffer items (but cancelled ones, inode buffers, and quota) and
  inode items laid over the volume's bytes, which reads see, v5 blocks
  and inodes not replayed over if their LSN is newer, and resealed.
  Items are in the writer's byte order, which the transaction header's
  magic tells; the logged inode core in memory's order too, and a logged
  B+ tree root in block form. Unfinished intents that change what reads
  see (mapping, exchange, and attribute intents), and unknown items,
  refuse the volume; so does an external log, whose state the driver
  cannot see, unless the caller opts in to reading either as it is.

**Robustness** follows §17.13: every block number is checked against its
AG and the volume before it is read; B+ trees are bounded by their level
(which must fall by one per step), records by their block, and extents
must be sorted, disjoint, and within the file's possible range; directory
entries must fit their block and be aligned; inode numbers are checked
against the AG geometry, and an inode must carry its magic and, with v5,
its own number, the UUID, and a valid CRC. Reads map blocks lazily, a
read's range at a time, merging contiguous blocks into one source read,
so no file's whole map is ever held.

**Verification.** `tests/fixtures/make.sh` formats volumes with
`mkfs.xfs` (v4 and v5; 512 B to 64 KiB blocks; 256 B to 2 KiB inodes;
large directory blocks; without `FTYPE`; and v5's later features each on
and off), fills them through the Linux kernel (fragmented and sparse
files whose maps become B+ trees, directories of every form, local and
remote symlinks, special files, names that are not UTF-8, extended
attributes of every form), and stores them sparsely; `filer-spec` runs
on its own tree. Linux mounts no v4 volume of blocks larger than its
pages, so those are filled by `mkfs.xfs -p`. Each log is then cleared by
`xfs_db logformat`, as the kernel's records would be most of a fixture.
`tests/differential.sh` compares each, and fresh ones, with the kernel:
names, kinds, sizes, contents, link targets, times, flags, and extended
attributes; listings are compared sorted. It then checks logs as the
kernel leaves them, with each sector size and log stripe unit: clean,
wrapped, shut down mid-write (read as the kernel recovers them), and
recovered. Each image and each log set-up is a job, run as many at once
as there are cores, against a release build: a run takes about two
minutes on four. Run repeatedly, it catches rare states, such as a
chunk of inodes freed and its blocks reused while the log still holds
the inodes. `tests/external.sh` tests against other projects' volumes
(`CREDITS.md`), never committed, fetched at pinned commits and checked
by their SHA-256 sums: walked, refused as expected, the trees recorded
of logs that need recovery matched, and compared with the kernel; they
found volumes no fixture had, such as a leaf directory shrunk to one
data block. `tests/fuzz.rs`
and `cargo-fuzz` targets in `fuzz/` mutate the superblock, inodes,
directory blocks, B+ tree blocks, and attribute forks (D50).

### 17.17 `filer-fs`

The shared machinery of the on-disk file system drivers (D54), recorded
2026-11-02. `filer-fat`, `filer-ext4`, `filer-ntfs`, and `filer-xfs`
depend on it, as does every file system stub in `tmp/` (§17.12) and
every file system driver to come. Read side by side, the four
working drivers turned out to be largely one program over four formats:
the same driver shell (handle payloads, the `Op` dispatch, the open
matrix, the `Resolver` walker), the same block-mapped read loop, and the
same field readers, source reads, name rules, Unix file types, and POSIX
ACL output, with FAT and NTFS encoding UTF-16 names as WTF-8 identically.
That moves here, step by step, each driver keeping only its format:

- field readers for little- and big-endian structures, and exact reads
  from a source;
- WTF-8 for UTF-16 names (FAT and NTFS, later exFAT and Joliet), moved
  to `filer-msft` (D60);
- Unix file types from directory entries (ext4, XFS); from modes, they
  moved to `FileKind::from_unix_mode` in `filer-api` (D60);
- POSIX ACLs in the form Linux's `getxattr` returns (ext4, XFS);
- reading a file through a map of runs (all four);
- the driver shell: a trait a format implements, and one `Driver` over
  it, as `filer-pack` serves archives (§17.7, D59), but leaving
  room for writing, which FAT plans (§17.13);
- partitioned disk images: MBR tables, extended partitions included, and
  GPTs, at 512- and 4096-byte sectors, by their backup header if need be;
  and a `Window` over one partition, which a driver opens as it would a
  volume;
- recovery in memory: the `Overlay` of 128-byte chunks that replaying a
  log or journal puts, and `Recovered`, a source read through it (XFS,
  and ext4's journal);
- Windows NT times and the file attributes that are flags (NTFS, and
  `filer-windows`, which reads both from Windows), moved to
  `filer-msft` (D60);
- reparse points, and Filer's reading of links' targets (NTFS, and
  `filer-windows`, which reads the same values through
  `FSCTL_GET_REPARSE_POINT`, so both report a link alike), moved to
  `filer-msft` (D60);
- FAT's local times, once a second driver needed them: `filer-zip` was
  that driver, and they went to `filer-msft` instead (D60).

What stays elsewhere: helpers every driver needs, not only file systems
(copying a slice into an `IoBufMut`, naming an entry in an error, the
rule every entry name meets), go to `filer-api`; checksums stay in `filer_api::checksum` (D53); test tooling
the drivers share (the corruption exerciser, the mutation fuzz loop,
sparse fixtures, and `tests/jobs.sh`, which runs a differential test's
comparisons at once) goes to `filer-spec`. Each format keeps its own error
enum, `Options`, and on-disk parsing.

Like `filer-pack` (D59), which took over from `filer_api::archive`
(D44), this is a crate of its own, per the placement rule (D20): it has
semantics of its own, and only file system drivers need it, with
`filer-windows` for its field readers alone, since `filer-msft` took the
on-disk forms it shares with NTFS (D60).

**`filer-msft`** (D60, 2026-11-03) takes Microsoft's on-disk conventions
out of this crate and `filer-fat`: NT times and the attribute flags
(`nt`), UTF-16 names as WTF-8 (`wtf8`), reparse points (`reparse`), and
FAT's local date and time, with their offset and granularity. ASK-02
found `filer-zip` re-implementing NT times, DOS times, and Unix modes,
and asked whether archive crates may depend on this crate for them. They
are a family's conventions rather than file systems': `filer-windows`,
an OS driver, already depended on this crate for them alone, and
`filer-7z` (NT times, UTF-16 names), `filer-rar` (DOS and NT times),
`filer-cdfs` (Joliet's UTF-16 names, FMT-07), `filer-exfat` (FAT-style
times and attributes), and `filer-refs` (NT times, reparse points) do or
would too. Every reader of the names module, built or planned, reads names a
Microsoft format wrote, so UTF-16 goes with the family; UDF has its own
character set. Unix modes are the other family's, read by ext4, XFS,
and `filer-posix` today and by cpio, ZIP, 7z, RAR, and most planned
file systems: near enough to every driver (D54's rule) that a
`FileKind` constructor in `filer-api` takes the one function the
archives need, and `unix` here keeps the entry type bytes, with the
POSIX ACLs. The field readers stay: generic, with seven users including
`filer-windows` and `filer-volume`, they are a later question, and the
archive crates' own cursors cost nothing meanwhile. Afterwards this
crate is read by file system drivers alone, but for `filer-windows`'s
use of `le`. The name: "msft" over "microsoft" for the workspace's
short names, and because the ticker is how others name the company
rather than how it names itself; the crate docs say it holds the
conventions of Microsoft's formats and nothing of Microsoft's. Free on
crates.io that day. Tasked as MSFT-01, CORE-07, and PACK-09.

As built (MSFT-01, 2026-11-03): `nt`, `wtf8`, and `reparse` moved
whole; `reparse` reads its two little-endian fields itself, as
`filer-msft` depends on `filer-api` alone. FAT's times became `dos`,
public now: `Stamp` (unset, valid, or invalid as written), `DateTime`,
the three granularities, and `UtcOffset`, which `filer-fat` re-exports
for its `Options`. CORE-07 named the constructor
`FileKind::from_unix_mode`, as `FileKind` is platform-independent; ext4,
XFS, and `filer-posix` call it.

**The driver shell** (as built, 2026-11-02). A format implements
`FileSystem`: its `DriverKey`, its root, a node's kind, lookup, a
symlink's target, opening a directory or a file, reading a file,
metadata (given what the handle holds open, as FAT and NTFS need it),
a page of a directory, and extended attributes, `Unsupported` by
default. Its associated `Node`, `Dir`, and `File` types are what lookups
find and what opening keeps. `Mounted<F>` serves it as the `Driver`,
the same way for every format: the `Op` dispatch, handle payloads, the
open matrix (`IsADirectory`, `NotADirectory`, `NotRegularFile`), entry
names to bytes with `ForeignNameMismatch`, paths through the userspace
`Resolver`, buffers filled with `fill_from`, and errors naming their
entries. A page of a directory is the format's to build, with its own
cookies (offsets, slots, or counts) and its errors for bad names. Each
driver's public type, such as `filer_xfs::Xfs<S>`, wraps a `Mounted`:
its `open` functions open the root with `Mounted::root`, and it
delegates `Driver`. Writing will add operations to `FileSystem`, each
`Unsupported` by default, and their dispatch to `Mounted`, so FAT's
writing (§17.13) extends the trait rather than replacing it.

### 17.18 `filer-volume`

`filer-volume` (D57, 2026-11-03) is to the file system drivers what
`filer-archive` is to the archive drivers: one dependency, and one
driver type, `Volume`, whatever the file system. Unlike an archive's,
a volume's format comes from its content, as images are rarely named
for it (`disk.img`): `VolumeDriver::detect` reads the first 2 KiB for XFS's
`XFSB` at 0, SquashFS's `hsqs` at 0, NTFS's OEM name at 3, an ext
superblock at 1024 (magic number, block size, and revision), then a FAT
boot sector as `filer-fat` checks one, last, since one may be left in a
volume formatted since. `Volume::open` detects and opens; `open_as` takes the
driver from the caller; `open_with` and `open_as_with` take
`VolumeOptions`, each enabled driver's own, with `allow_dirty` setting
every journaled one's. A partitioned disk is not a volume: `open` says
so (`VolumeError::Partitioned`), and `open_partition` opens one
partition through `filer_fs::partition`'s `Window`. As in
`filer-archive`, the format's driver is held as a `DynDriver`, each
driver is a feature re-exporting its crate as a module, `code-pages`
forwards `filer-fat`'s (not in `all`, as in `filer-fat`), `gzip`,
`lz4`, `lzma`, `lzo`, `xz`, and `zstd` forward `filer-squashfs`'s
compressors, and `zisofs` `filer-cdfs`'s (in `all`, as `filer-archive`
forwards its codecs), and opening a volume with a disabled driver is
`Unsupported` with a `VolumeError`. `VolumeFormat` is the format in
detail: ext2, ext3 (a journal), or ext4 (any feature flag past ext3's),
and FAT12, FAT16, or FAT32 (by the count of clusters, as Microsoft's
specification and `filer-fat` decide), with `Iso9660` (`CD001` at
sector 16, after ext's superblock and before FAT's boot sector, as an
ISO 9660 image's first sectors may hold a hybrid image's partition
table), `Udf` (`NSR02` or `NSR03` after `BEA01` in the volume
recognition sequence, before ISO 9660, so that a bridge disc is UDF's),
`Ntfs`, `Squashfs`, and `Xfs`; detection
lives there, `VolumeDriver::detect` is its `driver`, and
`Volume::format` gives an open volume's, or `None` if `open_as` opened
it with a driver its first bytes do not name. `blkid` agrees with it on
every fixture. Its tests open fixtures of the drivers' own crates, so
none is copied, and check that each lists as its driver does.

### 17.19 `filer-squashfs`

A read-only driver for SquashFS 4.0 images (FMT-08, 2026-11-05), the
compressed file system of live CDs, snaps, AppImages, and firmware,
following `filer-fat`'s model (§17.13): a lazy driver over any `ReadAt`
source, the async `Resolver`, panic-free by lint (D47), pure Rust (D45),
fixtures from the real tools, and a differential test against the Linux
kernel's `squashfs` driver. It is read-only for good: SquashFS images
are built whole, never changed in place. It is the first driver to read
through most of the `decode-*` crates (D62): each compressor that
`mksquashfs` offers is a feature, named as `mksquashfs` names it, over
its crate: `gzip` over `decode-zlib` (SquashFS's gzip is zlib's stream),
`lzma` over `decode-lzma`, `lzo` over `decode-lzo`'s raw blocks, `xz`
over `decode-xz`, `lz4` over `decode-lz4`'s raw blocks, and `zstd` over
`decode-zstd`. An image of a compressor whose feature is off, or of a
version before 4.0, which the Linux kernel no longer reads either, is
`Unsupported`.

**Clean room (D52).** The Linux kernel's `squashfs` and squashfs-tools
are GPL. The driver is written from descriptions of the format only: the
kernel's prose documentation and the published description of the
binary format, read for facts, and what `mksquashfs` makes of known
trees. No implementation's source code is consulted.

**On-disk facts the parser relies on:**

- Everything is little-endian. The superblock is the first 96 bytes:
  magic `hsqs`, the inode count, the time of making, the block size
  (4 KiB to 1 MiB, a power of two, with its logarithm), the fragment
  count, the compressor (1 to 6: gzip, LZMA, LZO, xz, LZ4, zstd), flags,
  the id count, version 4.0, the root inode's reference, the bytes used,
  and where the id, extended attribute, inode, directory, fragment, and
  export tables start; an absent table is all ones. `mksquashfs` pads
  images to 4 KiB past the bytes used.
- The inode, directory, fragment, and attribute tables are streams of
  metadata blocks, each up to 8 KiB decoded, behind a 16-bit header:
  its size on disk, and bit 15 if stored as is. A reference is a block's
  position in its table, shifted 16 bits up, and an offset in what it
  decodes to; entries cross blocks freely.
- An inode: its type (1 to 7, basic: directory, file, symlink, block
  and character device, FIFO, socket; 8 to 14, the same extended, with
  an attribute index and wider fields), mode, owner indices, modification
  time (seconds, the only time recorded), and number, then its type's
  fields. A directory gives its listing's place in the directory table
  and its size plus 3; a file its first block's position, size, fragment
  and offset in it (all ones if none), and the sizes of its blocks: one
  each, but for a tail in a fragment. A block's size has bit 24 if stored
  as is; a size of 0 is a block of zeros, which `mksquashfs` writes for
  sparse files.
- A listing is runs of up to 256 entries whose inodes share a metadata
  block, each behind a header: the count less one, that block, and an
  inode number the entries' are offsets from. An entry gives its inode's
  offset in the block, its number's offset, its basic type, and its name
  (up to 256 bytes) and length less one. `mksquashfs` sorts entries by
  name. An entry's number and type repeat its inode's, and a lookup
  refuses an entry that disagrees, so listings and lookups agree.
- The fragment table lists, raw, the metadata blocks of its entries, 512
  to a block: each a block's position and size. The attribute table's
  header gives where its pairs are, the id count, then lists the blocks
  of ids, 512 to a block: each a reference to the inode's pairs, their
  count, and size. A pair is a type (0 user, 1 trusted, 2 security; bit 8
  if the value is a reference to one stored once for all who share it), a
  name without its prefix, and a value. The export table maps inode
  numbers for NFS and the id table owners: Filer reports neither.

**Reading.** Opening reads the superblock and the attribute table's
header, and checks the tables' order. A file keeps a mark every 256
blocks of where its block is and where its size is listed, so that a
read sums at most 256 sizes, and never more of a list than a read's
offset reaches; a corrupt inode claiming billions of blocks costs
nothing until read. Metadata blocks (32, and those that do not decode,
which a corrupt image may point to again and again), fragment blocks
(4), listings (16, spanning at most 1 MiB of the directory table), the
attribute names last read, and each open file's last block are cached,
behind `filer_fs::Lock`. A lookup searches its directory's listing,
decoded whole and shared with handles, so paths through a directory of
many entries decode it once, not once each; by halves if its names
ascend, as `mksquashfs` sorts them, and else in order. Attribute values
are read only as asked for, since pairs may share one: a value of 64 KiB
that 65,536 pairs share would be 4 GiB read whole. A file's attribute
names may total 64 KiB, as Linux's list of them holds. `mksquashfs`
4.7.5's images of every compressor, of 4 KiB and 1 MiB blocks, stored,
without fragments, and with every tail in one read exactly as the Linux
kernel mounts them, LZMA's as `unsquashfs` extracts them; the
structure-aware fuzzer walked 60,000 mutated images clean.

### 17.20 `filer-cdfs`

A read-only driver for ISO 9660 images (FMT-07, 2026-11-05), of
installers, live systems, and archived discs, following
`filer-squashfs`'s model (§17.19): a lazy driver over any `ReadAt`
source, panic-free by lint (D47), pure Rust (D45), fixtures from the
real tools, and a differential test against the Linux kernel's `isofs`
driver. Named as Windows names its driver, CDFS; `filer-volume` names
the format `iso9660`, as `blkid` does. It reads one tree of an image, as
Linux does: Rock Ridge's if the root's `.` record begins with an `SP`
entry, else Joliet's (its highest level), else ISO 9660:1999's, else
the primary one's. Linux reads the primary tree where Filer reads the
1999 one, whose names are whole; the differential test skips such
images. zisofs is the `zisofs` feature, over `decode-zlib`.

**Sources (D52, `LEGAL.md`).** ECMA-119, the System Use Sharing Protocol
and Rock Ridge (IEEE P1281 and P1282, as drafted), Joliet, and zisofs's
published description: free, unpatented, and read for facts; and what
`xorriso` and `genisoimage` make of known trees.

**On-disk facts the parser relies on:**

- The volume descriptor set starts at sector 16 (of 2048 bytes), each
  descriptor with `CD001` at offset 1, its type at 0 (1 primary, 2
  supplementary, 255 the terminator), its version at 6 (2 for
  ISO 9660:1999's enhanced one); Joliet's has the escape sequence `%/@`,
  `%/C`, or `%/E` at offset 88. Each holds the logical block size at 128
  (512, 1024, or 2048) and its root's directory record at 156. Numbers
  are recorded both little- and big-endian; only the first is read, as
  the second is often wrong in practice.
- A directory record: its length, its extended attribute record's (in
  blocks, before the data), its extent, its data's length, a seven-byte
  date (years since 1900 to seconds, and the offset from Greenwich in
  quarters of an hour), flags (hidden 0x01, directory 0x02, associated
  0x04, more extents follow 0x80), the interleaving unit and gap, the
  name's length, the name, a padding byte after a name of even length,
  and System Use entries. Records end in the sector they begin in; a
  zero byte ends a sector's. `.` and `..` (names 0 and 1) come first.
  Plain names end in their version, `;1`, after a `.` even without an
  extension; Joliet's are UTF-16, big-endian.
- System Use entries: a signature of two letters, a length, a version,
  data. The root's `.` begins with `SP`, which says how many bytes to
  skip in every other record's; `CE` continues them in a block area;
  `ST` ends them. Rock Ridge's: `NM` names (in parts), `PX` modes and,
  in 1.12 (44 bytes), serial numbers; `SL` symlink components, with
  flags for continuation, `.`, `..`, and the root; `TF` times, short or
  long (seventeen digits) by its flags, in the order of their bits
  (creation, modification, access, attributes); `CL` a relocated
  directory's place, which the record stands for, and `RE` the relocated
  one, not shown where it is; `SF` sparse files; `ZF` zisofs (`pz`, the
  header's size, the block size's logarithm, the size).
- zisofs: a file begins with a magic number, its size, the header's
  size, and the block size's logarithm (15 to 17); then a pointer per
  block, and one past the last; each block is a zlib stream, or none for
  zeros.

**Reading.** Opening reads the descriptors and the root's `.` record. A
listing holds every entry's node, built from its records, since they
hold all that is reported; a file of several extents (beyond 4 GiB)
reads them in order; of several versions of a name, the first, the
latest, is shown; associated files are not. Lookups search a listing as
`filer-squashfs`'s do (§17.19), by halves when its names ascend. A
directory's id is its extent's place, so a relocated one has one id
however it is reached; a file's is its Rock Ridge 1.12 serial number,
which hard links share, or its record's place. Times are Rock Ridge's,
else the record's date as the modification time. Hidden files are shown,
with `FileAttrs::HIDDEN`, as Linux shows them; zisofs's files read
uncompressed, with `FileAttrs::COMPRESSED`. Interleaved files, sparse
ones, and zisofs's without the feature are `Unsupported`: no current
tool writes the first two. A read past the image's end, where only a
corrupt pointer leads, is `Corrupt`. Linux takes a directory's time from
its own `.` record, Filer from its record in its parent: they differ only
where a tool writes them apart, as `genisoimage` does in directories it
cuts off for depth. Linux looks Joliet's names up regardless of case;
Filer matches them exactly.

Fixtures are `xorriso` 1.5.6's, fixed in time: Rock Ridge 1.12; 1.10
with Joliet, a directory relocated; Joliet alone; plain names;
ISO 9660:1999's; and zisofs's; and `genisoimage`'s with a hidden file,
made fresh by the differential test, as it writes the time it runs.
Images made record by record cover files of several extents, versions,
associated files, interleaving, sparseness, and continuation areas. All
read as the Linux kernel mounts them, and the structure-aware fuzzer
walked 200,000 mutated images clean.

### 17.21 `filer-udf`

A read-only driver for UDF volumes (FMT-06, 2026-11-05), of DVDs,
Blu-ray discs, and rewritable media, following `filer-cdfs`'s model
(§17.20): a lazy driver over any `ReadAt` source, panic-free by lint
(D47), pure Rust (D45), fixtures from the real tools, and a
differential test against the Linux kernel's `udf` driver.
`filer-volume` detects UDF before ISO 9660, so that a bridge disc, of
both, is read as UDF, whose names and sizes are whole: for a DVD, UDF
is the file system that counts.

**Sources (D52, `LEGAL.md`).** ECMA-167 and OSTA's UDF 2.60
specification, free, and read for facts; and what `mkudffs` makes, and
the Linux kernel writes into it.

**On-disk facts the parser relies on:**

- Every descriptor begins with a 16-byte tag: its identifier, version,
  a checksum of the tag's other bytes, a serial number, a CRC-16 (the
  CCITT polynomial, from 0) of what follows, its length, and the block
  the descriptor says it is at. Checksums, identifiers, and every
  field's bounds are checked; CRCs are not, as `compute-crc` has no
  CRC-16 of that kind yet.
- The anchor volume descriptor pointer (2) is at sector 256, 256 before
  the last, or the last, of a block size no volume states before it:
  512 bytes to 4 KiB are tried, and its tag's location must be the
  sector it is at. It gives the main and the reserve volume descriptor
  sequences, of partition descriptors (5: a number, a start, a length),
  a logical volume descriptor (6: the block size, the file set's long
  allocation descriptor, the partition maps), pointers onward (3), and
  a terminator (8).
- Partition maps: type 1, physical; type 2, named by an identifier
  after a flags byte: `*UDF Sparable Partition` (a packet length and
  sparing tables, each an entry per moved packet: its first block in
  the partition, and the sector it moved to), `*UDF Virtual Partition`
  (a virtual allocation table, the file at the volume's last block, of
  type 248 in UDF 2.00 and later, a header then a block per entry, or in
  1.50 entries then `*UDF Virtual Alloc Tbl`), and `*UDF Metadata
  Partition` (a metadata file, of type 250, or its mirror, 251, whose
  extents are the partition's blocks).
- The file set descriptor (256) gives the root's entry. File entries
  (261) and extended ones (266): an ICB tag (strategy 4 or 4096, file
  type, and in its flags how data is allocated), the information length,
  times of twelve bytes (a type and a time zone in minutes, then the
  date, and hundredths, hundreds of microseconds, and microseconds),
  extended attributes, then allocation descriptors (short, 8 bytes;
  long, 16; extended, 20; their top two bits recorded, allocated, not
  allocated, or continued in an allocation extent descriptor, 258), or
  the data itself.
- A directory's data is file identifier descriptors (257): flags
  (hidden, directory, deleted, parent), the name's length, the entry's
  long allocation descriptor, implementation use, and the name in
  OSTA's compressed Unicode (a byte 8 or 16, then 8- or 16-bit
  characters), padded to four bytes. A symlink's data is path
  components: the root, `..`, `.`, or a name.

**Reading.** Opening finds the anchor, reads the sequences (the reserve
one if the main one fails), and resolves every partition map into a
translation from a partition's blocks to the volume's: physical,
through the sparing table, through the virtual allocation table, or
through the metadata file's extents. A listing reads every entry's file
entry, as listed kinds are stats' kinds, and a symlink's target; data is
read through runs of blocks adjacent in the volume. Files' ids are their
entries' addresses, which hard links share. Hidden files are shown,
with `FileAttrs::HIDDEN`; deleted ones, the parent, and repeated names
are not. Named streams and extended attributes are not read.

Fixtures are `mkudffs` 2.3's, filled by Linux: revisions 1.02 (short
allocation descriptors) and 2.01 (long, as Linux's `longad` writes
them, and extended entries), of 512-byte blocks, with small files
embedded and a deleted file's identifier; 2.50, a BD-R's, of a virtual
partition, empty, as `mkudffs` makes 2.50 only for write-once media,
which Linux does not write. The differential test also makes volumes of
2 KiB blocks, a CD-RW's sparable partition, and a CD-R's virtual one,
which all read as Linux mounts them. Volumes made here, descriptor by
descriptor, cover metadata partitions, a moved packet, and continued
allocation descriptors; no tool here writes the first two, so they are
read as the specification is read here, untested against another
reader. The structure-aware fuzzer walked 200,000 mutated volumes
clean.

### 17.22 Later

- `filer-fuse`: a FUSE mount of any driver, via `fuser`.
- `filer-wasi-host`: a `wasi:filesystem` host implementation for Wasmtime,
  mapping the capability model one to one.

## 18. Middleware

```rust
// filer-api
pub trait Layer<D: Driver> {
    type Driver: Driver;
    fn layer(&self, inner: D) -> Self::Driver;
}

let driver = Posix::thread_pool(DefaultPool::new())
    .layered(filer_tracing::TracingLayer::default())
    .layered(filer_faults::FaultLayer::from_seed(42)
        .fail_nth(OpKind::Sync, 3, ErrorKind::Io));
```

In priority order:

1. **`filer-tracing`**: one span per op with kind, relative path, bytes,
   duration, fallback taken, and error.
2. **`filer-faults`**: seeded, deterministic error and latency injection by
   op kind and path pattern. Complements `filer-heap`'s crash model for
   drivers that have no durable-state model.
3. **`filer-quota`**: byte and entry quotas per subtree.
4. **`filer-replay`**: record the op stream and completions; replay against
   `filer-heap` to reproduce bugs.

`filer_api::ReadOnly` is a combinator rather than a crate (no state, no
I/O): it rejects mutating ops with `ErrorKind::ReadOnly`. It matters when a
driver is exported without a frontend (`filer-fuse`); inside a program,
`Dir::restrict(Rights::READ)` already does the job.

Rights checks deliberately do not live in middleware: they belong to the
frontend handles that carry them.

## 19. Errors

```rust
pub struct Error(alloc::boxed::Box<Inner>);    // one pointer wide

struct Inner {
    kind: ErrorKind,
    op: OpKind,
    path: Option<RelPathBuf>,          // relative to the handle
    target: Option<RelPathBuf>,        // second path (rename, link)
    label: Option<alloc::boxed::Box<str>>, // the handle's label
    os: Option<RawOsError>,            // Errno(i32) | Windows(u32)
    fallback: Option<Fallback>,        // e.g. "openat2 -> userspace walk"
    source: Option<alloc::boxed::Box<dyn core::error::Error + Send + Sync>>,
}

#[non_exhaustive]
pub enum ErrorKind {
    NotFound, AlreadyExists, NotADirectory, IsADirectory, NotRegularFile,
    NotASymlink, DirectoryNotEmpty, PermissionDenied, RightsDenied,
    EscapeBlocked, SymlinkLoop, NameTooLong, InvalidName, InvalidNameForTarget,
    ForeignNameMismatch, CrossDriver, CrossesDevices, ReadOnly,
    StorageFull, QuotaExceeded, LimitExceeded, Unsupported, Busy, Stale,
    Poisoned,
    Canceled, Interrupted, TimedOut, UnexpectedEof, Moved, Corrupt, Io,
    Other,
}
```

As built (2026-11-01): fields are private, set with `with_*` builders and
read with accessors, so the sketched `ErrorContext` became the `path`,
`target`, and `label` accessors. Boxing keeps `Result<T>` one pointer wide.
`RawOsError` names its numbering (`Errno` or `Windows`) so a recorded code
is never misread on another platform. `Fallback` is a pair of `&'static
str` mechanism names. `OpKind` lists only the read operations so far and
grows with `Op`; each displays as its `snake_case` name.

- `RightsDenied` (library-enforced) is distinct from `PermissionDenied`
  (OS-enforced); `EscapeBlocked` means confinement stopped a resolution
  (`EXDEV` from `openat2`, or the userspace walk).
- `Busy` covers Windows sharing violations and `EBUSY`; `Stale` covers NFS
  `ESTALE`; `Poisoned` covers sync-after-failed-sync
  ([§13.1](#131-sync-failure-poisons-the-handle)); `Moved` is reported by the
  walker when a re-opened ancestor no longer has its recorded identity.
- Context carries paths *relative to the handle they were resolved
  against*, plus the handle's optional label (set with `Dir::with_label`,
  typically the ambient path used at `open_ambient`). Absolute paths are not
  synthesized: in sandboxed programs they can leak information, and they are
  often unknowable.
- `std::io::Error` interop under `std`: `From<Error> for std::io::Error`
  keeps the `Error` as the payload and maps the kind (the
  pattern Dogma.rs uses for `FromPathError::into_io_error`). Kinds without
  a stable `std::io::ErrorKind` counterpart at 1.97 (`SymlinkLoop`, since
  `FilesystemLoop` is unstable; `Poisoned`, `Canceled`, `Moved`, `Io`) map
  to `Other`.
- `Display` reads like `rename "a.toml" -> "b.toml" in <app>: already exists
  (os error 17)`.
- Lesson recorded: `std::io::Error` dropping the path is why `fs-err`
  exists; context is not optional here.

## 20. Walking and recursive operations

Rust's own `std::fs::remove_dir_all` had a symlink race (CVE-2022-21658),
because path-based recursion can be redirected mid-walk. Every recursive
operation here is built on handle-relative descent. The walker and the
operations built on it are generic algorithms in `filer-api`. A driver may
implement the coarse `Op::Walk` natively (one worker job per batch instead
of one per directory); it must then pass the same walker specification cases.

```rust
#[non_exhaustive]
pub struct WalkOpts {
    pub symlinks: WalkSymlinks,     // Never (default) | Follow
    pub cross_mounts: bool,         // default false
    pub on_error: OnError,          // Continue (default) | Abort
    pub max_open_dirs: usize,       // descriptor budget, default 64
    pub hydrate_placeholders: bool, // default false
    pub order: WalkOrder,           // Unspecified | ByName | PostOrder
}
```

- **Descent** opens each child directory relative to its parent handle with
  no-follow semantics, so concurrent renames of ancestors cannot redirect
  the walk.
- **Descriptor budget.** When the stack exceeds `max_open_dirs`, the oldest
  retained ancestors are closed. Re-entering one reopens it by name from the
  nearest retained ancestor and verifies its recorded `FileId`; a mismatch
  yields `ErrorKind::Moved` for that subtree rather than silently walking
  something else. This keeps deep trees from exhausting `RLIMIT_NOFILE`.
- **Loops.** When following symlinks, a set of ancestor `FileId`s detects
  cycles.
- **Mounts.** `MountId` comparison enforces `cross_mounts`.
- **Unknown kinds.** `Entry::kind == None` triggers a no-follow `stat`.
- **Foreign names** are walked and yielded like any other; the walker yields
  paths as `alloc::vec::Vec<EntryName>` components, with a lossy display.
- **Pruning** via a synchronous `filter_entry` callback.

Built on the walker:

- `remove_tree(name)`: post-order, never follows symlinks (removes the link
  itself), handles foreign names, ignores the Windows read-only attribute when
  deleting (§17.2), and retries `Busy` with bounded backoff on Windows.
- `copy_tree(from, to, CopyOpts)`: clones where possible, preserves per
  `Preserve`, optionally preserves hardlinks (via a `FileId` map), skips or
  reports special files, and returns an aggregate `CopyReport`.
- `disk_usage(name)`: sums `allocated` without hydrating placeholders.

## 21. Platform notes

### 21.1 Linux

- Resolution: `openat2` (5.6+) with `RESOLVE_BENEATH`, `RESOLVE_IN_ROOT`,
  `RESOLVE_NO_MAGICLINKS`, `RESOLVE_NO_SYMLINKS`, `RESOLVE_NO_XDEV`. Handle
  `EAGAIN` (retry, then userspace walk) and `ENOSYS` or `EPERM` from seccomp
  (userspace walk). **VERIFY** which errno container runtimes return for
  filtered syscalls today.
- Metadata: `statx` with masks; `STATX_DIOALIGN`, `STATX_MNT_ID_UNIQUE`,
  `STATX_WRITE_ATOMIC` (the last two are `rustix` gaps, §17.1), attribute
  bits (immutable, append, compressed, encrypted, verity, DAX).
- Namespace ops: `renameat2` (`RENAME_NOREPLACE`, `RENAME_EXCHANGE`; support
  varies by filesystem), `O_TMPFILE` + `linkat`.
- Data ops: `FICLONE`, `FICLONERANGE` and `FIDEDUPERANGE` (`rustix` gaps),
  `copy_file_range`,
  `fallocate` (including `FALLOC_FL_PUNCH_HOLE`), `SEEK_DATA` /
  `SEEK_HOLE`, `RWF_ATOMIC` (exposed later, behind `unstable`).
- Locks: `flock` (D27); OFD locks (`F_OFD_SETLK`) once `rustix` exposes
  them (§17.1); never classic `fcntl` locks, including
  `rustix::fs::fcntl_lock`.
- Casefolding: per-directory on ext4 and f2fs; reported in `NamingRules`.
- Sandboxing complement: Landlock (5.13+).
- io_uring: may be disabled by policy; see
  [§17.3](#173-filer-uring).

### 21.2 FreeBSD

`O_RESOLVE_BENEATH` for kernel confinement (present in `rustix`, checked
2026-10-31), Capsicum rights on descriptors (planned, §6.7), `extattr`
namespaces (user and system) for xattrs, `copy_file_range` (block cloning
on ZFS in recent releases, **VERIFY**). Not in CI; best effort until it
is.

### 21.3 macOS

- No `openat2`: userspace walk with `O_NOFOLLOW`.
- `F_FULLFSYNC` for real durability (`rustix::fs::fcntl_fullfsync`);
  `F_BARRIERFSYNC` as an extension (**VERIFY** availability).
- `fclonefileat` for clones on APFS (in `rustix`); `renameatx_np` with
  `RENAME_SWAP` through `rustix`'s `renameat_with(EXCHANGE)`, and
  `RENAME_EXCL` for no-replace (**VERIFY** the `NOREPLACE` mapping).
- No `O_TMPFILE`: atomic writes use named temporaries.
- Flat xattr namespace; `com.apple.*` attributes (quarantine, resource fork)
  are visible and should be preserved by `copy_tree` when `XATTRS` is set.
- APFS is normalization-insensitive and normalization-preserving; HFS+
  stored a decomposed form.
- Dataless files (`SF_DATALESS`) from iCloud and File Provider.
- CI: a macOS job is part of the project from the start (§3.6).

### 21.4 Windows

- Resolution relative to directory handles via `NtCreateFile` (§17.2). No
  kernel "beneath" check, so `Enforcement::Userspace`.
- Long paths are a non-issue for relative opens; ambient roots use `\\?\`.
- Names: reserved device names (`CON`, `NUL`, `COM1`...), trailing dots and
  spaces, and `<>:"/\|?*` plus control characters are rejected with
  `InvalidNameForTarget`. The NT API would accept several of these, but the
  resulting files are unusable from Win32 programs. Case-insensitive by
  default with per-directory case sensitivity.
- Names that are not well-formed UTF-16 (unpaired surrogates) become
  `ForeignName`s holding WTF-8.
- Sharing: open with full share modes including `FILE_SHARE_DELETE`; use
  POSIX semantics for delete and rename.
- Antivirus and indexers transiently open files, so rename and delete need
  bounded retry with backoff on sharing violations (Cargo, rustup, and npm
  all learned this). Default: up to about one second total, configurable,
  reported in `Error::fallback` when it happened (D18).
- Read-only attribute: deletion uses
  `FILE_DISPOSITION_FLAG_IGNORE_READONLY_ATTRIBUTE` instead of clearing it.
- Durability: `NtFlushBuffersFileEx` (data-only) and `FlushFileBuffers`.
- Clones: `FSCTL_DUPLICATE_EXTENTS_TO_FILE` on ReFS and Dev Drive.
- xattrs: none natively in the POSIX sense; alternate data streams can
  stand in for `User` if the driver opts in and reports it.
- IoRing: covers only a small set of operations; not worth a driver until
  it covers namespace operations. IOCP for data I/O is the nearer step.

### 21.5 Network filesystems

NFS has close-to-open consistency, `ESTALE` handles, locks that may be
emulated or absent, and write errors deferred to `close`. SMB has oplocks
and leases. None of this can be hidden. `Features::remote` lets databases
refuse or warn, `ErrorKind::Stale` is distinct, and `close(self)` exists to
surface deferred errors.

### 21.6 Cloud placeholder files

OneDrive Files On-Demand (`FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS`,
`FILE_ATTRIBUTE_RECALL_ON_OPEN`) and iCloud dataless files look like regular
files, but reading them triggers a download or fails offline. Metadata
exposes `FileAttrs::dataless`; walkers, `copy_tree`, and `disk_usage` default
to not hydrating; `Open::allow_hydration()` is explicit.

### 21.7 WebAssembly

- `wasm32-unknown-unknown`: no OS filesystem. In browsers, `filer-whatwg`
  (§17.11) uses the WHATWG File System API; elsewhere (Node, Deno, bare
  hosts) only `filer-heap` and other virtual drivers do anything. Every
  crate and feature must still build there (CI).
- WASI targets: `std` uses wasi-libc's `openat`-style calls over preopens,
  which already behave like capabilities. `filer-wasi` (§17.8) turns
  preopens into `Dir`s via `AmbientAuthority`; CI checks every crate with
  all features on `wasm32-wasip1` and `wasm32-wasip2`.

## 22. Workspace layout and features

### 22.1 Repository layout

```text
filer/
  AGENTS.md  AUTHORS  CHANGES.md  CREDITS.md  DESIGN.md  README.md
  TODO.md  UNLICENSE  VERSION  Cargo.toml  Makefile  .rustfmt.toml
  .cargo/config.toml  .github/workflows/ci.yaml
  lib/
    filer-api/
      src/
        lib.rs                crate docs, gates, re-exports
        path/                 Name, NameBuf, RelPath, RelPathBuf, macros
        error.rs  error_kind.rs  op_kind.rs
        rights.rs  resolve.rs  ambient_authority.rs
        foreign_name.rs  entry_name.rs  link_target.rs  naming_rules.rs
        metadata.rs  meta_mask.rs  file_kind.rs  file_id.rs  timestamp.rs
        file_attrs.rs  change_token.rs  permissions.rs
        durability.rs  io_buf.rs  buf_result.rs
        op.rs  completion.rs  chain.rs  handle.rs  features.rs
        driver.rs  dyn_driver.rs  layer.rs  hybrid.rs  read_only.rs
        spawner.rs  default_pool.rs (std)
        resolver.rs           generic userspace walk (§6.5)
        blocking_resolver.rs  its blocking variant
        dir.rs  file.rs  open.rs  read_dir.rs  entry.rs
        copy.rs  atomic_writer.rs  walker.rs
    filer-posix/   src/ { lib.rs, executor.rs, ops/, sys/ (stopgaps) }
    filer-wasi/    src/ { lib.rs, ops/ }
    filer-windows/ src/ { lib.rs, ops/, sys/ }
    filer-cli/     src/ { main.rs }
    filer-uring/   src/ { lib.rs, ring.rs, sys/ }
    filer-heap/  src/ { lib.rs, inode.rs, crash.rs, harness.rs, ... }
    filer-vfs/  filer-overlay/  filer-pack/  filer-msft/  filer-tar/  filer-zip/
    filer-tracing/  filer-faults/  filer-quota/  filer-replay/
    filer-tx/  filer-tokio/  filer-spec/  filer-fuse/
    <umbrella>/
  tmp/                  stubs reserving names, until they read (D58)
    filer-rar/  filer-7z/  filer-exfat/  filer-btrfs/  ...
```

### 22.2 Features

Every crate follows §3.2. Sketches (only features with code are added, as
code arrives):

```toml
# filer-api
[features]
default = ["all", "std"]
all = ["serde"]
std = ["serde?/std"]                    # DefaultPool, ambient, io interop
serde = ["dep:serde", "bitflags/serde"]
bytes = ["dep:bytes"]
camino = ["std", "dep:camino"]          # Utf8Path conversions
futures = ["dep:futures-core"]
futures-io = ["std", "futures", "dep:futures-io"]
dogma-path = ["dep:dogma-path"]         # §5.7, low priority
unstable = []

# filer-posix (OS crates look alike)
[features]
default = ["all", "std"]
all = []
std = ["filer-api/std", "dep:rustix"]
unstable = ["filer-api/unstable"]       # RWF_ATOMIC and other experiments

[target.'cfg(unix)'.dependencies]
rustix = { workspace = true, optional = true, features = ["fs", "std"] }

# filer-heap
[features]
default = ["all", "std"]
all = []
std = ["filer-api/std"]                 # std::sync::Mutex instead of spin
harness = []                            # crash-point enumeration
```

The umbrella (Q11) forwards component features: `all` covers the `no_std`
components (`heap`, `vfs`, `overlay`, `serde`), and its `std` also
enables the native drivers (`filer-posix` on Unix, `filer-wasi` on WASI,
`filer-windows` on Windows, all target-gated, as `filer-cli` does) so that
a default build is immediately useful (D25). Optional components
(`uring`, `tokio`, `tracing`, `faults`, `tx`, ...) are named features.

## 23. Dependencies

Candidate versions as of 2026-10-31 (crates.io index; recheck before adding,
and confirm each builds at MSRV 1.97 with `default-features = false` and
passes the minimal-versions CI job). Add each to `[workspace.dependencies]`
only when the first item needs it.

| Crate | Version | Used by | Notes |
| --- | --- | --- | --- |
| `rustix` | 1.1 | `filer-posix`, `filer-uring` | gaps in §17.1 |
| `windows-sys` | 0.61 | `filer-windows` | `Wdk_*` features too |
| `io-uring` | 0.7 | `filer-uring` | or `compio-driver` |
| `compio-driver` | 0.12 | `filer-uring` | evaluate in a spike |
| `bitflags` | 2.13 | `filer-api` | `no_std`; optional `serde` |
| `ref-cast` | 1.0 | `filer-api` | path DSTs (D4) |
| `spin` | 0.12 | `filer-pack`, `filer-heap` | `no_std` lock |
| `lzma-rust2` | 0.25 | `decode-lzma`, `decode-xz` | tests' oracle and encoder |
| `async-lock` | 3.4 | `filer-tx`, `filer-quota` | MSRV 1.85; `no_std` |
| `serde` | 1.0 | several | optional |
| `bytes` | 1.12 | `filer-api` | optional |
| `futures-core` | 0.3 | `filer-api` | optional `Stream` |
| `futures-io` | 0.3 | `filer-api` | optional adapters |
| `tracing` | 0.1 | `filer-tracing` | `no_std` capable |
| `tokio` | 1.x | `filer-tokio` | spawner, I/O traits |
| `fuser` | 0.18 | `filer-fuse` | MSRV 1.85; no `libfuse` (D45) |
| `pollster` | 1.0 | tests | tiny `block_on` |
| `tempfile` | 3.27 | tests | dev-dependency |
| `proptest` | 1.12 | tests | dev-dependency; MSRV 1.88 |

Considered and rejected:

- `cap-primitives`, `cap-std`, `cap-fs-ext`: good prior art for confinement
  semantics, but the drivers call `rustix` and `windows-sys` directly
  (D22), which gives full control over flags, error mapping, and features.
- `winx`: see §17.2; its `windows-sys` range lags the workspace version.
- `reflink-copy`: path-based API, incompatible with capability semantics.
- `xattr`: `rustix` already exposes fd-based xattr calls.
- `pathrs` (libpathrs): excellent Linux-only resolver with a C API; a
  reference and a test oracle for `filer-posix`'s resolution, not a
  dependency.
- `tokio::sync` in core crates: see §7.7.

## 24. Testing strategy

Crash consistency that is not tested does not exist. The strategy, in order
of leverage:

1. **`filer-spec`.** A published library crate, the executable
   specification of the driver contract: resolution and confinement
   (including symlink escapes and races), foreign names, rights, every op's
   success and error behavior, chains, cancellation by drop, and feature
   honesty (a reported feature must work; an unreported one must return
   `Unsupported`). `Profile` declares what the environment can create
   (symlinks, FIFOs, foreign names) so cases skip rather than fail. Every
   driver crate runs it in its tests; out-of-tree drivers can too (D26).

   As built (2026-11-01), named `filer-spec` rather than
   `filer-conformance`: `run(profile, setup)` calls the driver's `setup`
   twice (two instances, for isolation cases) to create a described
   `Tree`, then runs 15 read-only cases through the frontend and returns a
   `Report` of pass, skip, or fail verdicts, so one run shows every
   failure. Cases report rather than panic. `block_on` parks the thread
   under `std` (so thread-pool drivers need no spinning) and spins
   without. `filer-heap`'s tests also run it against a driver that
   loosens `Deny`, asserting the specification catches it.
2. **Differential testing.** Generate random op sequences (proptest) and run
   them against `filer-heap` and `filer-posix`; compare observable state.
   Heap personalities let one CI machine imitate case-insensitive, NFD, or
   reflink-less filesystems.
3. **Crash properties.** `filer-heap`'s `harness` feature enumerates crash
   points. For atomic writes and every transaction strategy: for every crash
   point and every `CrashPolicy`, after recovery the observable state equals
   either the full pre-state or the full post-state. Also: sync failure
   poisons, and recovery is idempotent (crash during recovery, recover
   again).
4. **LazyFS.** An optional Linux CI job mounts LazyFS (a FUSE filesystem
   that drops unsynced data on command, used by Jepsen) and runs the crash
   properties against `filer-posix`, validating the heap crash model against a
   real kernel path.
5. **Platform tests.** Behavior only real systems show: `openat2` `EAGAIN`
   and `ENOSYS` handling, the `O_TMPFILE` fallback (test a filesystem
   without it, such as vfat), btrfs or XFS loopback images for reflink and
   dedupe, Windows sharing violations (hold a file open without
   `FILE_SHARE_DELETE` and rename it), case-insensitive directories, APFS
   clones on the macOS job.
6. **Fuzzing.** Pure-Rust fuzz tests, plus `cargo-fuzz` targets under the
   D50 exemption, for on-disk parsers (`filer-ext4`, `filer-fat`,
   `filer-ntfs`, and `filer-zip` have both), name and
   path validation, the journal parser, Windows reparse-data parsing, and
   archive member names.
7. **Miri.** `filer-api`, `filer-heap`, and the routing drivers run under
   Miri (no FFI there). The `sys` modules of `unsafe` crates keep their
   pure logic (buffer bookkeeping, reparse parsing) in functions testable
   under Miri.
8. **Feature matrix.** §3.6, for every crate.

Doctests must pass in the packaged-crate CI job under all four feature
combinations, so examples that touch the OS are gated on `std` and the
relevant target.

## 25. Observability

- The driver boundary is the natural instrumentation point: `filer-tracing`
  sees every op with its kind, relative path, size, duration, and outcome.
- Every fallback taken (userspace walk instead of `openat2`, copy instead of
  clone, named temporary instead of `O_TMPFILE`, Windows retry) is recorded
  in the result (`CopyReport`, `Error::fallback`, receipts) and emitted as a
  tracing event. Silent degradation is the bug class this project exists
  to remove.
- A `Stats` snapshot on `Posix`, `Windows`, and `Uring` (queue depth, ops
  by kind, fallbacks) for debugging.

## 26. Excluded on purpose

- **Current directory and absolute paths** outside the ambient entry points.
- **An implicit seek cursor** in the core file type (adapters only).
- **Memory mapping** in the async core. Page faults block executor threads
  invisibly, and I/O errors arrive as `SIGBUS` (Crotty, Leis, and Pavlo,
  CIDR 2022, catalogue the problems in databases). A separate crate
  (`filer-mmap`) may come later; it would contain `unsafe` and so falls
  under §3.3.
- **Locks as correctness.** Advisory locks for cooperation only.
- **TxF** and any other deprecated kernel transaction API.
- **Text encodings, line endings, user and group names.**
- **Emulation of unsupported features** unless the caller opted into a
  fallback.
- **Watching** (inotify, fanotify, FSEvents, `ReadDirectoryChangesW`) is
  deferred. If added, it must model events as lossy hints with an explicit
  "overflowed, rescan" event, never as a reliable change log.

## 27. Prior art

| Project | Borrow | Avoid or differ |
| --- | --- | --- |
| `cap-std` | confinement, Windows walk | sync only; ARF names |
| `rustix` | safe syscall layer | n/a |
| WASI 0.3 | preopens, USV paths, coarse rights | unreachable names |
| Zig 0.16 `std.Io` | injected I/O implementation | no confinement, byte paths |
| SwiftNIO NIOFileSystem | delayed materialization | still unstable |
| C++ LLFIO (P1031) | race-free handles, caching modes | sync-first stance |
| Go `os.Root` | traversal-resistant API shape | path strings |
| Apache OpenDAL | layers, `ChaosLayer` | object-store semantics |
| tokio-uring, monoio, compio | owned-buffer I/O | runtime coupling |
| Tokio fs + io_uring | transparent backend swap | one-ring contention |
| libpathrs | `openat2` expertise, procfs safety | Linux only |
| PyFilesystem2 | `MountFS`, `MultiFS`, `SubFS` | synchronous |
| Java NIO `SecureDirectoryStream` | relative ops on a dir | platform-limited |
| TxFS (ATC 2018) | simple begin/commit API | kernel patch |
| LazyFS | unsynced-data loss for tests | FUSE dependency |
| ALICE (OSDI 2014) | catalogue of crash bugs | n/a |

Details worth remembering:

- WASI 0.3.0 (June 2026) made nearly all filesystem methods async with a
  completion-based model, and its paths are strings of Unicode scalar
  values; it accepts that some on-disk files are unreachable.
- Zig 0.16 migrated all filesystem operations to `std.Io.Dir` and
  `std.Io.File` with an `Io` parameter; `Threaded` is mature, `Evented`
  (io_uring, kqueue, Dispatch) experimental.
- SwiftNIO's NIOFileSystem delays materialization of transactional
  exclusive-create or truncate opens, using `O_TMPFILE` where available, and
  remains the underscored `_NIOFileSystem` module.
- P1031 met a mostly neutral reception at LEWG-I and did not advance, and
  argues async file I/O matters less than sync: read it for the strongest
  counterargument to this design.
- Rust's own `std::fs::Dir` exists unstable (feature `dirfd`, issue
  #120426) with `open` and `open_file`; its docs warn that some platforms
  give no TOCTOU guarantees.

## 28. Decision log

Status values: **Accepted** (build on it), **Proposed** (default unless
challenged), **Open** (needs a decision before the dependent work starts).
Record changes here with a date and one line of rationale.

| ID | Decision | Status |
| --- | --- | --- |
| D1 | Handles are authority; ambient only via `AmbientAuthority` | Accepted |
| D2 | UTF-8 names; total enumeration via opaque `ForeignName` | Accepted |
| D3 | Path types start in `filer_api::path`, kept self-contained | Accepted |
| D4 | Unsized path newtypes via `ref-cast`; views only as fallback | Accepted |
| D5 | Owned buffers; sealed safe `IoBuf`; zero-fill if safe | Accepted |
| D6 | `'static` futures; submit on first poll; run to end | Accepted |
| D7 | Ops-as-data driver boundary; `#[non_exhaustive]` `Op` | Accepted |
| D8 | Positional I/O only; cursors are adapters | Accepted |
| D9 | Four coarse rights: `READ`, `WRITE`, `MUTATE`, `META` | Accepted |
| D10 | Guarantees reported, never silently emulated | Accepted |
| D11 | `unsafe` only in driver crates that need it, in `sys` | Accepted |
| D12 | `Spawner` trait and `DefaultPool` in `filer-api` | Accepted |
| D13 | `Send` futures by default; `LocalDriver` later | Proposed |
| D14 | Own `no_std` `Error`, `#[non_exhaustive]`, io interop | Accepted |
| D15 | No mmap in the core | Accepted |
| D16 | Transactions in `filer-tx`, `publish = false` until gated | Proposed |
| D17 | Transaction recovery is explicit, never on open | Accepted |
| D18 | Bounded Windows retry on sharing violations, reported | Proposed |
| D19 | Features are per directory handle, cached per mount | Accepted |
| D20 | Multi-crate workspace from the start; placement rule §4.1 | Accepted |
| D21 | Sync locks for state; `async-lock` across awaits; no Tokio | Accepted |
| D22 | `rustix` on Unix; `windows-sys` directly on Windows | Accepted |
| D23 | Sync failure poisons the handle | Accepted |
| D24 | `ForeignName` is never a creation or rename destination | Accepted |
| D25 | Umbrella crate; its `std` enables native drivers | Proposed |
| D26 | `filer-spec` is a published library | Accepted |
| D27 | Unix advisory locks use `flock`; never classic `fcntl` | Proposed |
| D28 | `rustix` gaps: upstream first, confined stopgaps second | Accepted |
| D29 | Versions start at 0.0.0; releases are 0.0.x until 0.1.0 | Accepted |
| D30 | `filer-wasi` and `filer-cli` exist from the start | Accepted |
| D31 | Repository at `github.com/artob/filer` (resolves Q12) | Accepted |
| D32 | `ForeignName`s are minted only via a driver's `DriverKey` | Accepted |
| D33 | `IoBufMut::fill` instead of `spare_zeroed` plus `commit` | Accepted |
| D34 | Handles too are minted and read only via a `DriverKey` | Accepted |
| D35 | The in-`poll` executor is named "blocking", not "inline" | Accepted |
| D36 | Frontend methods are `async fn`s borrowing `self` (§7.2) | Accepted |
| D37 | `filer-system` is the one cross-platform dependency | Accepted |
| D38 | The browser driver is `filer-whatwg`, after its standard | Accepted |
| D39 | Capsicum, Landlock, `unveil`/`pledge` live in `filer-posix` | Accepted |
| D40 | Planned: `filer-uefi`; 9P, SMB, NFS, SFTP; FAT and others | Accepted |
| D41 | Archives are `filer-tar` and `filer-zip`, one per format | Accepted |
| D42 | Byte sources are `ReadAt`: positional reads, length | Accepted |
| D43 | Archive drivers are read-only, permanently | Accepted |
| D44 | Shared archive code lives in `filer_api::archive`, until D59 | Accepted |
| D45 | 100% Rust: no dependency may compile or bundle C code | Accepted |
| D46 | `filer-fat` is a lazy driver; exFAT is `filer-exfat` | Accepted |
| D47 | Async first, no panics, illegal states unrepresentable | Accepted |
| D48 | `Resolver` is async; `BlockingResolver` shares its walk | Accepted |
| D49 | Code pages in `known-codepages`, a support crate to spin out | Accepted |
| D50 | `cargo-fuzz` (C++ libFuzzer) allowed in `fuzz/` crates only | Accepted |
| D51 | Discard is an optional `deallocate` on writable sources | Accepted |
| D52 | Formats implemented only in GPL code are written clean room | Accepted |
| D53 | Checksums drivers share live in `filer_api::checksum` | Accepted |
| D54 | File system drivers share code in `filer-fs` | Accepted |
| D55 | `Dir::names` collects a listing's names; `entries` streams | Accepted |
| D56 | `filer-archive` opens any archive format, by its name for now | Accepted |
| D57 | `filer-volume` opens any file system, detected by its content | Accepted |
| D58 | Stubs reserving names live in `tmp/` until they read something | Accepted |
| D59 | Archive drivers share code in `filer-pack`, where `filer_api::archive` moves | Accepted |
| D60 | Microsoft's on-disk conventions live in `filer-msft`, out of `filer-fs` | Accepted |
| D61 | Codec crates are interim; our own public-domain decoders replace them when they block | Accepted |
| D62 | Our decoders are `decode-<codec>` crates, generic, dependency-free, one sans-I/O shape | Accepted |
| D63 | Decoders are benchmarked by `bench/METHOD.md`; its harness, apart, may build C counterparts | Accepted |
| D64 | Our checksums and hashes are `compute-<algorithm>` crates, generic and dependency-free | Accepted |
| D65 | `compute-*` crates may take CPU instructions behind `intrinsics`, through `compute-multiversion` | Accepted |

Notes on the 2026-10-31 decisions:

- **D3:** path types may later be imported from and re-exported by
  `dogma-path`; keeping `filer_api::path` free of other `filer-api`
  dependencies makes that move non-breaking.
- **D4:** Option A; the compile test passed on 2026-11-01 (§5.3).
- **D11:** crates without `unsafe` declare `#![forbid(unsafe_code)]`; the
  mechanics are in §3.3. Every current crate forbids it, the empty OS
  driver shells included, until one needs a `sys` module.
- **D20:** supersedes the earlier single-crate plan.
- **D21:** the question of Tokio mutexes is answered in §7.7.
- **D22:** supersedes the earlier plan to use `cap-primitives` on Windows;
  `winx` only if its `windows-sys` range is aligned (§17.2).
- **D29:** 0.0.x signals that the API is extremely experimental and
  unstable; supersedes the earlier plan to start at 0.1.0 (§3.1).
- **D30:** reconciles this document with the bootstrapped workspace, which
  already had both crates; supersedes the plan to cover WASI through
  `filer-posix` (§17.8, §17.9).

## 29. Open questions

- **Q1.** Should `Name` forbid C0 control characters even though POSIX
  allows them? Current answer: no; the portability lint flags them.
- **Q2.** Is `MUTATE` on a directory enough, or do callers need "create but
  not delete" (append-only directories, upload drop boxes)? If real use
  cases appear, split into `CREATE` and `REMOVE`.
- **Q3.** Chains: should a failed op optionally not cancel the rest
  (`IOSQE_IO_HARDLINK`)? Not until a use case needs it.
- **Q4.** Should `Metadata` carry numeric owners in the portable type, or
  only via platform extensions?
- **Q5.** Should the umbrella glob-export `filer_api::*` at its root, given
  names like `File` and `Dir` that may collide with user imports?
  `filer-system` does (§17.10); the §3.8 umbrella is still open.
- **Q6.** Optional `miette` diagnostics for `filer_api::Error`?
- **Q7.** Archive support: own small zip and tar readers (`no_std`
  possible) or existing crates (bigger, `std`)? Packaging is settled
  (separate `filer-tar` and `filer-zip`, D41); both have their own
  parsers (§17.7).
- **Q8.** Should `Dir::open_ambient` accept `camino::Utf8Path` (and Dogma.rs
  types, §5.7) directly, or only `std::path::Path`?
- **Q9.** Watching (§26): in scope for 1.0 or never?
- **Q10.** License: Unlicense was carried over from Dogma.rs; confirm.
- **Q11.** Umbrella crate name. `filer` on crates.io is a 0.0.0 placeholder
  owned by an unrelated project (published 2022 by a memflow maintainer,
  checked 2026-10-31). Options: ask the owner to transfer it, or publish
  the umbrella under another name. All `filer-*` names checked were free.

## Appendix A: Glossary

- **Ambient authority:** access derived from global process state (current
  directory, absolute paths) rather than from a handle the code was given.
- **Capability:** an unforgeable handle that both designates a resource and
  authorizes access to it. Here, a `Dir` or `File` with its `Rights`.
- **Chain:** ops submitted together, executed in order, later ops able to
  reference earlier results through placeholder handles.
- **Commit point:** the single durable step after which a transaction is
  logically complete and recovery rolls forward.
- **Crash-atomic:** all-or-nothing after a crash and recovery, without
  promising that concurrent observers never see intermediate states.
- **Combinator:** a driver wrapper in `filer-api` that performs no I/O and
  holds no filesystem state (`DynDriver`, `Hybrid`, `ReadOnly`).
- **Driver:** anything implementing `Driver`; concrete, routing, or
  middleware.
- **Enforcement:** the mechanism confining resolution beneath a handle:
  kernel, userspace walk, or a virtual driver's own resolver.
- **Executor:** how a driver runs blocking calls: on a `Spawner` (thread
  pool) or blocking inside `poll`.
- **Foreign name:** a name found on disk that is not valid UTF-8 (or UTF-16
  on Windows); opaque, usable only with its originating driver.
- **Op:** one driver-level operation as a value; `Completion` is its result.
- **Payload:** the driver-private part of a `Handle` (an `OwnedFd`, a
  Windows `HANDLE`, a heap inode id).
- **Pre-image:** the prior state of an entry, retained to allow rollback.
- **Personality:** a heap-driver configuration imitating a real
  filesystem's naming rules, features, and timestamp granularity.
- **Stopgap:** local `unsafe` covering a gap in a safe dependency until an
  upstream fix lands (§3.3).

## Appendix B: Primitive mapping

Marks: "rustix" = available in `rustix` 1.1.6; "gap" = missing there
(checked 2026-10-31); "ws" = confirmed in `windows-sys` 0.61.2.

Linux (`filer-posix`, `filer-uring`):

- resolve: `openat2` + `RESOLVE_BENEATH` / `RESOLVE_IN_ROOT` (rustix)
- stat: `statx` with `AT_EMPTY_PATH` (rustix); `STATX_DIOALIGN`,
  `STATX_MNT_ID` (rustix); `STATX_MNT_ID_UNIQUE`, `STATX_WRITE_ATOMIC` (gap)
- atomic create: `O_TMPFILE` + `linkat`; `renameat2(RENAME_NOREPLACE)`
  (rustix)
- swap: `renameat2(RENAME_EXCHANGE)` (rustix)
- clone: `FICLONE` (rustix); `FICLONERANGE`, `FIDEDUPERANGE` (gap)
- copy offload: `copy_file_range` (rustix)
- space: `fallocate`; holes: `SEEK_DATA` / `SEEK_HOLE` (rustix)
- durability: `fdatasync`, `fsync`, `syncfs` (rustix)
- locks: `flock` (rustix); OFD `F_OFD_SETLK` (gap); never `fcntl_lock`
- xattrs: `fgetxattr`, `fsetxattr`, `flistxattr`, `fremovexattr` (rustix)
- listing: `getdents64` via `RawDir` (rustix)
- io_uring opcodes (where probed): `OPENAT2`, `STATX`, `READ`, `WRITE`,
  `FSYNC`, `RENAMEAT`, `UNLINKAT`, `MKDIRAT`, `SYMLINKAT`, `LINKAT`,
  `FALLOCATE`, xattr ops, `FTRUNCATE` (**VERIFY** minimum kernels)

macOS (`filer-posix`):

- resolve: userspace walk with `O_NOFOLLOW`
- atomic create: named temporary + `renameatx_np(RENAME_EXCL)`
- swap: `renameatx_np(RENAME_SWAP)` via `renameat_with` (rustix)
- clone: `fclonefileat` (rustix)
- durability: `F_FULLFSYNC` (rustix); `F_BARRIERFSYNC` (**VERIFY**)
- space: `F_PREALLOCATE` (**VERIFY**)
- xattrs: `fgetxattr` and friends, flat namespace (rustix)

FreeBSD (`filer-posix`):

- resolve: `openat` + `O_RESOLVE_BENEATH` (rustix)
- xattrs: `extattr_*_fd` (**VERIFY** rustix)
- copy offload: `copy_file_range`

Windows (`filer-windows`):

- resolve: `NtCreateFile` with `RootDirectory`, `FILE_OPEN_REPARSE_POINT`
  (ws)
- list: `NtQueryDirectoryFile` (ws) or `GetFileInformationByHandleEx`
- rename: `SetFileInformationByHandle(FileRenameInfoEx)` with
  `FILE_RENAME_FLAG_POSIX_SEMANTICS` (ws) and optionally
  `FILE_RENAME_FLAG_REPLACE_IF_EXISTS`
- delete: `FileDispositionInfoEx` with POSIX semantics and
  `FILE_DISPOSITION_FLAG_IGNORE_READONLY_ATTRIBUTE` (ws)
- hard link: `NtSetInformationFile(FileLinkInformation)`
- symlink: reparse point via `FSCTL_SET_REPARSE_POINT`
- clone: `FSCTL_DUPLICATE_EXTENTS_TO_FILE` (ws; ReFS, Dev Drive)
- holes: `FSCTL_SET_SPARSE`, `FSCTL_QUERY_ALLOCATED_RANGES`,
  `FSCTL_SET_ZERO_DATA`
- durability: `NtFlushBuffersFileEx` with `FLUSH_FLAGS_FILE_DATA_SYNC_ONLY`
  (ws), `FlushFileBuffers`
- locks: `LockFileEx`
- identity: `FILE_ID_INFO` (128-bit)

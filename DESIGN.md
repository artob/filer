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
- Every crate without `unsafe` (currently all of them, including the empty
  OS driver shells) also declares `#![forbid(unsafe_code)]` at the crate
  root, marking it unsafe-free in a way no module can override.
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
`-D warnings`, plus a nightly docs.rs-settings build; packaged doctests
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

### 3.7 The umbrella crate

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
| `filer-fat` and others | on-disk formats (planned, low priority) | no |
| `filer-system` | the native driver for the target, plus the API | no |
| `filer-cli` | the `filer` binary over the native driver | no |
| umbrella (Q11) | re-exports and feature forwarding | no |

Existing so far (2026-11-01): `filer-api`, `filer-heap`, and `filer-posix`
(read-only), `filer-system`, `filer-spec`, and as empty skeletons
`filer-wasi`, `filer-whatwg`, `filer-windows`, `filer-tar`, `filer-zip`,
and `filer-cli` (D30, D38, D41). The rest are
created when their first `TODO.md` item starts.

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
             +-- filer-vfs, filer-overlay, filer-tar, filer-zip
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
resolution, under kernel-enforced confinement where available. A separate
`RelPathBuf::normalize_lexically(&str)` may accept `.` components and
redundant separators for convenience, but must still reject `..` and roots.

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
implement four synchronous primitives (`lookup` one name without
following, `kind`, `read_link`, and optionally `crosses_mount`), and the
provided `resolve` method walks a byte path (so non-UTF-8 targets and
foreign names work) under a `Resolve` policy, with `follow_final` choosing
whether a final symlink is followed. It keeps the chain of directories it
entered; `..` pops that chain, so it cannot reach anything outside it.
Confinement is relative to the start handle: `..` above it or an absolute
target is `EscapeBlocked` under `Beneath`, and clamps to or restarts from
it under `InRoot`; any symlink to follow is `SymlinkLoop` under `Deny`
(as `RESOLVE_NO_SYMLINKS` gives `ELOOP`), as is a 41st hop
(`MAX_SYMLINK_HOPS`). A driver with kernel confinement can override
`resolve`. Synchronous primitives fit every executor: a thread-pool
driver runs the whole walk as one job.

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
}
```

- Streams, never a `Vec`. Order is unspecified; sorting is the caller's job.
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
`kind`, `len`, the four timestamps, and `id`, with `MetaMask` bits at the
values above (others are added with their operations and never renumbered).
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
`set_flags` where supported:

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
opens roots. Resolution is the generic `Resolver` (§6.5) on a private
`Walker`, so `openat2` (POSIX-02) is still to come: `lookup` is `statat`
with `AT_SYMLINK_NOFOLLOW`, then directories open with `O_DIRECTORY |
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
construction (`dir`, `file`, `symlink`, taking string or `RelPath`
arguments, creating parents like `mkdir -p`, panicking on mistakes), and
the tree, a `filer_api::tree::Tree<Vec<u8>>` (§17.7), is then immutable
behind an `Arc`, so no locks are needed yet. Directories map raw name
bytes to inodes in a `BTreeMap`, so listings are sorted by bytes, foreign
names will fit, and a `DirCookie` is an entry index. Handle payloads are
inode numbers; `FileId`s are `(0, inode)`. Paths resolve through the
tree's `Resolver` (§6.5), and a following single-entry stat uses the
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

### 17.7 `filer-tar` and `filer-zip`

Read-only archive drivers, one crate per format (D41, answering Q7's
packaging question): they need different dependencies and codec features,
and users rarely need both. Both are virtual drivers like `filer-heap`:
`Enforcement` is `Virtual`, rights are at most `READ`, paths resolve
through the generic `Resolver` (so archive symlinks are confined beneath
the archive root), and no `AmbientAuthority` is needed, because the
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
are features, preferring `no_std` decoders.

**Names and paths.** Non-UTF-8 names become `ForeignName`s. Names with
`..`, absolute or drive prefixes, or backslashes are never resolved
outside the root: quarantined or rejected. Directories missing from the
archive (common in ZIP) are synthesized. Decoding ZIP's legacy CP437
names (no UTF-8 flag, bit 11) to UTF-8 is lossless and planned as a
stretch goal; until then such names are UTF-8 if valid and foreign
otherwise. **Duplicate entries** are resolved the same way in both
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

**Shared structure.** An archive index is an immutable tree like
`filer-heap`'s, with leaves pointing at (offset, size, codec) instead of
bytes. As built (2026-11-01), that tree is `filer_api::tree::Tree<F>`,
generic over the file payload: `Node::{Dir, File(F), Symlink(bytes),
Special(FileKind)}`, policy-free primitives (`add`, `link` returning the
replaced inode, `make_dir`), and a `Resolver` impl. Drivers apply their
own policies (`Heap` panics on duplicates; archives let the later entry
win) and keep their tree private. Creating archives
does not fit the driver model (no in-place rename or removal; ZIP writes
its index last) and belongs to a separate writer API, such as packing a
`Dir` into a TAR or ZIP.

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
driver (`filer-posix` on Unix, later `filer-windows` and `filer-wasi`),
target-gated like `filer-cli`. Being a newtype rather than an alias, it has
one API everywhere (`System::blocking()`, `open_ambient`, and a free
`filer_system::open_ambient`). On targets without a driver yet, including
`wasm32-unknown-unknown`, `System` wraps a private stub that fails every
operation with `Unsupported`, so portable code compiles everywhere and
starts working as drivers land; nothing is emulated. Without `std` it is
just the re-export.

The broader umbrella of §3.7 (re-exporting `filer-heap`, middleware, and
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
- **On-disk formats, low priority:** `filer-fat` (FAT12/16/32 and exFAT),
  and other file systems such as littlefs, over a block device, for
  embedded and `no_std` use and for reading disk images.

### 17.13 Later

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
    StorageFull, QuotaExceeded, Unsupported, Busy, Stale, Poisoned,
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
        dir.rs  file.rs  open.rs  read_dir.rs  entry.rs
        copy.rs  atomic_writer.rs  walker.rs
    filer-posix/   src/ { lib.rs, executor.rs, ops/, sys/ (stopgaps) }
    filer-wasi/    src/ { lib.rs, ops/ }
    filer-windows/ src/ { lib.rs, ops/, sys/ }
    filer-cli/     src/ { main.rs }
    filer-uring/   src/ { lib.rs, ring.rs, sys/ }
    filer-heap/  src/ { lib.rs, inode.rs, crash.rs, harness.rs, ... }
    filer-vfs/  filer-overlay/  filer-tar/  filer-zip/
    filer-tracing/  filer-faults/  filer-quota/  filer-replay/
    filer-tx/  filer-tokio/  filer-spec/  filer-fuse/
    <umbrella>/
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
| `spin` | 0.12 | `filer-heap` | `no_std` lock |
| `async-lock` | 3.4 | `filer-tx`, `filer-quota` | MSRV 1.85; `no_std` |
| `serde` | 1.0 | several | optional |
| `bytes` | 1.12 | `filer-api` | optional |
| `futures-core` | 0.3 | `filer-api` | optional `Stream` |
| `futures-io` | 0.3 | `filer-api` | optional adapters |
| `tracing` | 0.1 | `filer-tracing` | `no_std` capable |
| `tokio` | 1.x | `filer-tokio` | spawner, I/O traits |
| `fuser` | 0.18 | `filer-fuse` | MSRV 1.85 |
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
6. **Fuzzing.** `cargo-fuzz` targets for name and path validation, the
   journal parser, Windows reparse-data parsing, and archive member names.
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
| D12 | `Spawner` trait and `DefaultPool` in `filer-api` | Proposed |
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
  `filer-system` does (§17.10); the §3.7 umbrella is still open.
- **Q6.** Optional `miette` diagnostics for `filer_api::Error`?
- **Q7.** Archive support: own small zip and tar readers (`no_std`
  possible) or existing crates (bigger, `std`)? Packaging is settled
  (separate `filer-tar` and `filer-zip`, D41); the parser choice is open.
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

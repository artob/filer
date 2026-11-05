# Project files
- Don't ask to examine parent directories, stick to the project directory.
- Read [LEGAL.md](LEGAL.md) before planning or touching any on-disk
  format: its sources allowed and forbidden, and its per-format policy
  (exFAT unreleased until its patents expire, mid-2028; never a ReFS or
  APFS writer). A format it does not assess gets assessed there first.
- Read [TODO.md](TODO.md) for remaining enhancements. Make one narrowly scoped,
  atomic change per request; remove completed items after verification.
- When updating `AGENTS.md`, keep in mind that the file is meant to especially
  benefit lesser models than yourself, such as GPT-5.6 Sol, Terra, and Luna;
  but they also have smaller context windows, so be terse and token-efficient.
- Don't update `README.md` casually: beneficial additions require significant
  judgment and discernment, possibly beyond your capabilities. Longer and more
  detailed does *not* in fact equal better, because humans are not LLMs!
- Any documentation you might wish to add to the README likely better belongs
  as inline comments to the modules and/or types in question, where you are
  allowed elaborated length. For example, in Rust code `rustdoc` coverage of
  every public symbol is a worthy goal, but brevity and quality matter.
- Keep to an 80-column wordwrap in comments and Markdown files, where feasible.

# Rust code
- Don't ever directly read the contents of `Cargo.lock`, it can very large.
- Our current MSRV is Rust 1.97; update and enforce everywhere as needed.
- Our crates collect their default features under an `all` feature, and
  their `[features]` should start with the line `default = ["all", "std"]`.
- Our crates are meant to always be buildable with `#![no_std]` and hence
  always export an explicit `std` feature flag. Some of our lower-level crates
  may also have an explicit `alloc` feature, but for many crates it's not
  possible to do anything useful without heap allocations and they hence
  implicitly assume and omit such a feature.
- All references to `std`, `alloc`, and `core` types should always use
  qualified names or explicit, least-power imports. For example, prefer
  `core::error::Error` and `alloc::string::String` over `std` analogs.
- After making changes to a crate, as a last step build its docs as
  docs.rs does, which shows which feature gates each item:
  `RUSTDOCFLAGS="--cfg docsrs" cargo +nightly doc --workspace
  --all-features`. Rustdoc infers those badges from `#[cfg]`, so never
  write `doc(cfg(...))`; every library crate root keeps
  `#![cfg_attr(docsrs, feature(doc_cfg))]`.
- Robustness (D47): async first; no panics on any input (no `unwrap`,
  `expect`, `panic!`, unchecked indexing or arithmetic in library code);
  make illegal states unrepresentable; never ignore an error. Every
  library crate's root denies these by lint (copy `filer-api`'s block
  into new crates); allow one only where it stands, with a `reason`.
- 100% Rust: never add a dependency that compiles or bundles C/C++ (no `cc`,
  `cmake`, `pkg-config`, `bindgen`, or C-wrapping `-sys` crates; CI checks).
  Rust bindings to the OS's own APIs (`libc` on macOS, `windows-sys`) are
  fine. Codecs must be pure-Rust decoders; codec crates are interim
  (D61): never wait on upstream, and when one blocks, write our own
  clean-room, public-domain decoder (`LEGAL.md`) as a `decode-<codec>`
  crate (D62). Sole exemptions, each its own `[workspace]` and never a
  dependency: `cargo-fuzz` targets in a crate's `fuzz/` (D50; keep a
  pure-Rust fuzz test in `tests/` too), and the benchmark harness in
  `bench/` (D63), whose counterparts may be C. Each `decode-*` crate's
  `BENCH.md` holds only what that harness generates (`bench/METHOD.md`).
- Builders maximize usability: paths take `P: TryInto<RelPathBuf>` (plain
  strings or `rel!`), panicking under `#[track_caller]`; ops and driver APIs
  stay strictly typed.

# Workspace
- Rust 2024 crates live in `lib/<crate>/`: `filer-api` owns the
  platform-independent API; backends implement it for Unix (`filer-posix`),
  WASI (`filer-wasi`), Windows (`filer-windows`), and browsers
  (`filer-whatwg`, the WHATWG File System API); `filer-heap` is the
  in-memory driver; `filer-tar`, `filer-zip`, `filer-ar`, and `filer-cpio`
  are archive drivers (no OS, build everywhere, read from `ReadAt` sources,
  read-only forever: never add archive writing; each is a header parser
  over `filer-pack`, where what they share goes, fuzzed through
  `filer-spec`'s `fuzz` and `exercise`), and
  `filer-archive` opens any of them by name (add every new archive crate
  to it as an optional dependency, feature, and re-export);
  `filer-fat` is the FAT12/16/32 driver (read-only for now, writing
  planned); `filer-ext4`, `filer-ntfs`, and `filer-xfs` are read-only file
  system drivers; `filer-volume` opens any of
  them, detected by content (add every new file system crate to it as an
  optional dependency, feature, and re-export); `filer-fs` holds what they
  share (each depends on it, `filer-windows` for `le`; shared code goes
  there, not into one driver): a format
  implements `filer_fs::FileSystem`, its public type wraps
  `filer_fs::Mounted`, and its tests use `filer-spec`'s `Sparse`,
  `Overlay`, `exercise`, and `fuzz`, and its `tests/differential.sh` runs
  a release `<driver>-manifest` example (unique: a workspace build links
  every example into one directory) in parallel jobs through
  `lib/filer-spec/tests/jobs.sh`;
  `filer-msft` holds Microsoft's on-disk conventions (NT times and
  attributes, UTF-16 names as WTF-8, reparse points, DOS times) for every
  driver reading them, file system, archive, or `filer-windows`;
  `known-codepages` decodes DOS code pages for FAT and ZIP names (generic,
  not Filer-specific: to be spun out to `it-is-known/known-codepages`);
  `decode-<codec>` crates (`decode-bzip2`, `decode-deflate`,
  `decode-gzip` and `decode-zlib` over it, `decode-lz4`, `decode-lzma`,
  `decode-xz` over it, `decode-lzo`, `decode-zstd`) are our clean-room decoders (D62): generic, no
  dependencies but each other and `compute-*`, each a `Decoder` with
  `decode`, `finish`, `checkpoint`, and `buffered`, which `filer-pack`
  adapts; `compute-<algorithm>` crates (`compute-crc`, every CRC a type
  and a feature, `compute-adler32`, `compute-sha256`, `compute-xxhash`)
  are our clean-room checksums and hashes (D64): generic, no
  dependencies, each with `new`, `update`, and `finish`;
  `filer_api::checksum` re-exports `compute-crc`'s. Generic checksum and
  hash algorithms go into a `compute-*` crate, never into a decoder or a
  driver; a format's own name hashes and sums (ext4's and XFS's
  directory hashes, tar's header sum) stay in its driver;
  `filer-system` re-exports `filer-api` plus `System`, the target's native
  driver (the one dependency for users); `filer-cli` builds the `filer`
  binary over `filer-system`.
- Stubs that reserve a crate name but read nothing yet live in
  `tmp/<crate>/` (archives: `filer-rar`, `filer-7z`; file systems, each
  depending on `filer-fs`: `filer-udf`, `filer-exfat`, `filer-btrfs`,
  `filer-zfs`, ...; checksums: `compute-blake2`, `compute-blake3`,
  `compute-fletcher`, `compute-sha512`). A stub's crate docs list what
  its format needs. It
  moves to `lib/` with its first implementation, and joins its umbrella
  then (`filer-rar`, `filer-7z`, and `filer-udf` already have).
- New crates must also be added to the README crate table. CI derives its
  package lists from `cargo metadata` and builds every `lib/*/fuzz/`.
- `filer-cli` is binary-only: its `[[bin]]` has `required-features = ["std"]`,
  so `no_std` builds skip it. It has no doctests; CI's packaged-doctest loop
  omits it. `filer-system` depends on each backend only under its `cfg`;
  doctests needing `std` go on `std`-gated items, not the crate root.
- Root `Cargo.toml` owns shared metadata/dependencies. Keep package and internal
  dependency versions aligned with `VERSION`; internal defaults stay disabled.
  Releases are 0.0.x for now: extremely experimental, any one may break.
- Backend crates depend on `filer-api`, never the reverse. Move behavior tests
  with their implementation.
- Every driver runs `filer-spec` (`run_blocking(profile, setup)`) in its
  tests; extend `filer-spec` when adding operations, and keep only
  driver-specific tests in the driver.
- Gate backend items on `cfg(unix)`, `cfg(target_os = "wasi")`,
  `cfg(windows)`, or `cfg(all(target_family = "wasm", target_os = "unknown"))`:
  each backend must still build, empty, on every other target.
- Root `[workspace.lints]` holds shared lints; each crate sets
  `[lints] workspace = true`. Every crate root has `#![forbid(unsafe_code)]`.
  A backend that needs FFI switches to `#![deny(unsafe_code)]` and confines
  `#[allow(unsafe_code)]` to a private `sys` module (`DESIGN.md` §3.3).
  Gate filesystem APIs on `std` and optional interop on matching features.
- Drivers never surface `EINTR`: wrap every system call in a retry, and
  reopen streams that stop after an error (`filer-posix`'s `errno.rs`).

# Checks
For Rust changes, run from the repository root:

```sh
cargo fmt --all -- --check
cargo test --workspace
cargo test --workspace --all-features
cargo check --workspace --no-default-features
cargo check --workspace --no-default-features --features all
cargo doc --workspace --all-features
```

- Also check affected crates individually with defaults disabled: workspace
  feature unification can hide missing gates. Check backends for foreign
  targets, e.g. `x86_64-pc-windows-msvc`, `wasm32-wasip1`, and macOS/Linux.
- `.github/workflows/ci.yaml` builds, builds examples, and tests on Ubuntu,
  macOS, and Windows; it also checks minimal features and a bare-metal
  `no_std` target.
- Build local docs without `--no-deps`: with it, cargo documents a crate
  while rewriting its dependencies' docs, so links into them silently
  render as plain text.
- Stable rustfmt warns about the nightly-only `imports_granularity` setting.

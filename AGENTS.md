# Project files
- Don't ask to examine parent directories, stick to the project directory.
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
- After making changes to a crate, as a last step run `cargo doc` on it.
- Builders maximize usability: paths take `P: TryInto<RelPathBuf>` (plain
  strings or `rel!`), panicking under `#[track_caller]`; ops and driver APIs
  stay strictly typed.

# Workspace
- Rust 2024 crates live in `lib/<crate>/`: `filer-api` owns the
  platform-independent API; backends implement it for Unix (`filer-posix`),
  WASI (`filer-wasi`), Windows (`filer-windows`), and browsers
  (`filer-whatwg`, the WHATWG File System API); `filer-heap` is the
  in-memory driver and `filer-tar`/`filer-zip` are archive drivers (no OS,
  build everywhere, read from `ReadAt` sources); `filer-system` re-exports
  `filer-api` plus `System`, the target's native driver (the one dependency
  for users); `filer-cli` builds the `filer` binary over `filer-system`.
- New crates must also be added to the package lists in
  `.github/workflows/ci.yaml` and to the README crate table.
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
cargo doc --workspace --all-features --no-deps
```

- Also check affected crates individually with defaults disabled: workspace
  feature unification can hide missing gates. Check backends for foreign
  targets, e.g. `x86_64-pc-windows-msvc`, `wasm32-wasip1`, and macOS/Linux.
- `.github/workflows/ci.yaml` builds, builds examples, and tests on Ubuntu,
  macOS, and Windows; it also checks minimal features and a bare-metal
  `no_std` target.
- Stable rustfmt warns about the nightly-only `imports_granularity` setting.

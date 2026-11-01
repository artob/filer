# Filer.rs backlog

Recheck the code and [`DESIGN.md`](DESIGN.md) before starting an item.
Paths are relative to the repository root; crates live in `lib/<crate>/`.
Section numbers (§N) and decision numbers (DN) refer to `DESIGN.md`.
Sections are ordered by priority, and so are the items within each section;
later sections assume earlier ones are done.

## Working method

- Follow `AGENTS.md`. Implement one narrowly scoped, independently
  verifiable item per request. Items are sized to be one atomic commit; if
  one turns out bigger, split it here first and implement only the first
  part. Create commits only when explicitly requested.
- Each item says what "done" means. Add regression coverage alongside
  behavior, in the crate that owns the behavior. After verification, remove
  completed items; for partial work, keep only the remaining substeps.
- Every crate is `#![no_std]`-buildable with an explicit `std` feature;
  `default = ["all", "std"]`; `all` builds on `thumbv7em-none-eabihf`; every
  feature builds alone; all features build on `wasm32-unknown-unknown`
  (§3.2).
- `unsafe` only in the `sys` module of a driver crate that needs it; every
  other crate declares `#![forbid(unsafe_code)]` (§3.3, D11).
- Create a crate when its first item starts, not before. Add a dependency
  to `[workspace.dependencies]` when the first item needs it, with
  `default-features = false`.
- User-visible additions get a `CHANGES.md` entry under "Unreleased".
  Prefer rustdoc over README changes. Keep Markdown and comments at 80
  columns. Run `cargo doc` last after changing a crate.
- When an item changes a design decision, update the decision log (§28) in
  the same change. When an item resolves a **VERIFY** note, rewrite that
  note in `DESIGN.md` with the finding and its date.

## Project bootstrap

The workspace, root files, CI, and five empty crates (`filer-api`,
`filer-posix`, `filer-wasi`, `filer-windows`, `filer-cli`) exist. Versions
start at 0.0.0 and releases stay 0.0.x until the criteria at the end of
this file are met (D29).

- [ ] **BOOT-01: Derive CI package lists.**
  `ci.yaml` hard-codes the package list in the packaged-doctest and
  feature jobs. Derive it from `cargo metadata` (excluding `filer-cli` from
  doctests, which has no library). Done when CI is green and adding a crate
  needs no workflow edit.

## Questions for the maintainer

- [ ] **ASK-01: Umbrella crate name (Q11).**
  `filer` on crates.io is an unrelated 0.0.0 placeholder. Ask whether to
  request a transfer or pick another name. Blocks the umbrella section.

## Verification spikes

Each spike produces findings, not library code: update the matching
**VERIFY** note in `DESIGN.md` (with a date) and add or adjust items here.

- [ ] **VER-02: Remaining `rustix` questions.**
  The 2026-10-31 audit is in §17.1. Still open: `F_BARRIERFSYNC`,
  `F_PREALLOCATE`, `FS_IOC_SETFLAGS`, `RWF_ATOMIC` via `pwritev2`, the
  `NOREPLACE` mapping to `RENAME_EXCL` on Apple targets, FreeBSD
  `extattr_*_fd`, and WASI coverage (for `filer-wasi`, §17.8). Done when
  §17.1, §21, and Appendix B have no `rustix`-related **VERIFY** left.

- [ ] **VER-03: `linkat` of `O_TMPFILE` files on Linux.**
  Whether `AT_EMPTY_PATH` still requires `CAP_DAC_READ_SEARCH` on current
  kernels, and how the `/proc/self/fd` fallback behaves without `/proc`.
  Done when §14.1 step 4 states the attempt order.

- [ ] **VER-04: Syscall filtering in containers and CI.**
  The errno for a seccomp-filtered `openat2` and `io_uring_setup` under
  Docker, Podman, and GitHub-hosted runners. Done when §6.5, §17.3, and
  §21.1 describe the exact fallback triggers.

- [ ] **VER-05: io_uring opcode coverage.**
  Minimum kernel per opcode in Appendix B; whether any upstream `getdents`
  op exists; whether clone or `copy_file_range` can go through io_uring.
  Done when §16.5 and Appendix B are updated.

- [ ] **VER-06: Windows details.**
  Minimum Windows versions for `FileRenameInfoEx` and
  `FileDispositionInfoEx` with POSIX semantics and the ignore-read-only
  flag; how per-directory case sensitivity interacts with
  `OBJ_CASE_INSENSITIVE`; whether flushing a directory handle means
  anything on NTFS and ReFS; how to detect symlink-creation rights; current
  IoRing coverage. Done when §17.2 and §21.4 have no **VERIFY** left.

- [ ] **VER-08: Change tokens.**
  A Linux change cookie through `statx`, if any, and reading a per-file USN
  on NTFS (`FSCTL_READ_FILE_USN_DATA`). Done when §10 lists the token source
  per platform.

- [ ] **VER-09: FreeBSD block cloning.**
  `copy_file_range` behavior on ZFS. Done when §21.2 is updated.

## Upstream work

Per D28, gaps in safe dependencies are fixed upstream first. Each item is
done when the PR is merged and released, or when §17.1 records why not.

- [ ] **UP-01: `rustix`: `STATX_MNT_ID_UNIQUE` and `STATX_WRITE_ATOMIC`.**
  Constants plus the atomic-write fields of `Statx`.
- [ ] **UP-02: `rustix`: `FICLONERANGE` and `FIDEDUPERANGE`.**
  Safe wrappers alongside `ioctl_ficlone`.
- [ ] **UP-03: `rustix`: OFD locks.**
  A safe `F_OFD_SETLK`/`F_OFD_SETLKW` wrapper, and a doc note on
  `fcntl_lock` explaining classic lock semantics.
- [ ] **UP-04: `winx`: `windows-sys` range (optional).**
  Only if Filer.rs decides to depend on `winx` (§17.2).

## `filer-api`: paths

All in the self-contained `filer_api::path` module (D3), using `ref-cast`
with `#[derive(RefCast, RefCastCustom)]` (D4, verified in §5.3).

- [ ] **PATH-04: `RelPathBuf::normalize_lexically`.**
  Accepts `.` components, redundant separators, and (when asked) `\`;
  rejects `..`, roots, and drive prefixes. Done when tests cover POSIX- and
  Windows-style inputs.

- [ ] **PATH-05: Serde.**
  Strings on the wire, validated on input. Done when `serde_test` covers
  success and rejection.

- [ ] **PATH-06: Native and Camino conversions.**
  `TryFrom<&std::path::Path>` under `std` and `TryFrom<&camino::Utf8Path>`
  under a `camino` feature, using native component rules. Done when tests
  cover absolute paths, prefixes, `..`, and non-UTF-8 input on POSIX and
  Windows.

- [ ] **PATH-07: Portability lint.**
  `Name::portability_issues()`: Windows reserved names, trailing dots or
  spaces, Windows-forbidden characters, C0 controls, more than 255 bytes or
  255 UTF-16 units. Done when each issue has a test.

- [ ] **PATH-08: Keep `path` self-contained.**
  A CI check (for example a script that fails if `lib/filer-api/src/path/`
  mentions `crate::` outside `crate::path`) so the D3 follow-up stays a
  non-breaking move. Done when the check runs in CI.

## `filer-api`: core types

No OS access; everything builds without `std` (§7.6).

- [ ] **CORE-02: Rights.**
  `Symlinks` and `Resolve` exist. Still missing: `Rights` (§6.2) via
  `bitflags` and the rights table as rustdoc. Done when attenuation is
  tested and `serde` works under its feature.

- [ ] **CORE-04: Remaining metadata fields.**
  `MetaMask`, `Metadata`, `FileKind`, `FileId`, and `Timestamp` exist with
  kind, length, times, and id. Still missing (§10): `allocated`, `nlink`,
  `FileAttrs`, `ChangeToken`, `Permissions`, `DioAlign`, `AtomicWriteUnit`,
  `MountId`, and `Owner`, each added with the operation that needs it. Done
  when every field exists and `serde` derives exist where meaningful (not
  for values valid only within one driver instance).

- [ ] **CORE-05: Durability, naming rules, features.**
  `Durability` (§13), `NamingRules` with `collides` (§5.6), `Features` and
  its sub-structs (§16.4). Done when `collides` is tested for case- and
  normalization-insensitive rules (normalization via a small table or an
  optional dependency, decided in the item).

- [ ] **CORE-06: Remaining buffers.**
  `IoBuf` (`Vec<u8>`, `Box<[u8]>`), `IoBufMut` (`Vec<u8>`), and `BufResult`
  exist. Still missing (§7.1): `IoBuf` for `String` and `&'static [u8]`,
  `Slice<B>`, and `AlignedBuf`. Done when tests cover slicing and aligned
  reads. `bytes` impls are a separate item (ADP-03).

## `filer-api`: driver boundary

- [ ] **DRV-01: Background closer.**
  `Handle` and `HandleKind` exist (§16.2). Still missing: the
  background-close hook, so dropping the last clone hands the payload to
  the driver instead of closing inline (§8.4). Done when a test driver
  observes the close on its closer. The `Pending` placeholder belongs to
  DRV-03.

- [ ] **DRV-02: Write ops and completions.**
  The read subset of `Op`, `Output`, and `Completion` exists, with
  `ErasedBufMut`. Still missing (§16.1): the write and namespace ops,
  `OpenHow` write modes and creation, `RenameFlags`, and `ErasedBuf` with
  its slot in `Completion`. Done when an erased buffer round-trips to its
  original type for every sealed buffer impl.

- [ ] **DRV-03: Chains.**
  `Driver` with `id` and `submit` exists. Still missing: a default
  sequential `submit_chain`; `Chain` with placeholders (a `Pending` handle
  state, §16.2); cancel-the-rest semantics (§7.3, §16.3). Done when a toy
  driver in tests exercises a failing chain.

- [ ] **DRV-04: `DynDriver` and `Layer`.**
  `ErasedDriver`, `DynDriver` with a blanket impl, the `Layer` trait, and
  `layered()`. Done when a pass-through layer and `DynDriver` both run the
  toy driver's tests.

- [ ] **DRV-05: `Hybrid` and `ReadOnly` combinators.**
  Per §16.5 and §18. Done when `Hybrid` routes by op kind with handle
  translation and `ReadOnly` rejects every mutating `OpKind` (test
  enumerates them).

- [ ] **DRV-06: `Spawner` and `DefaultPool`.**
  The trait, the internal oneshot, and the `std` pool (bounded threads,
  idle timeout, named threads) per §7.5. Done when tests cover saturation
  and shutdown.

- [ ] **DRV-07: Generic userspace resolver.**
  The component-by-component walk of §6.5 in `filer_api::resolver`,
  written against a small trait of primitives (open one component without
  following, read a link, identify a handle), with `Beneath`, `InRoot`,
  `Deny`, mount crossing, and a hop limit of 40. Done when tests against a
  toy primitive set cover every escape case; `filer-heap`,
  `filer-posix`, and `filer-windows` reuse it.

## `filer-heap`

The executable specification and test bed; it comes before any OS driver.

- [ ] **HEAP-01: Crate and volatile core.**
  Create `lib/filer-heap/` (`no_std`, `forbid(unsafe_code)`). Inode table
  and directory maps; open, read, write, close, stat, read-dir, mkdir,
  remove, rename (with flags), link, symlink, readlink; the lock shim
  (§7.7, D21); an injectable `Clock`. Done when it builds on
  `thumbv7em-none-eabihf` and unit tests cover each op.

- [ ] **HEAP-02: Resolution through the generic resolver.**
  Done when escape attempts via absolute targets, `..` in targets, and
  loops fail as specified.

- [ ] **HEAP-03: Personalities and foreign names.**
  Configurable `NamingRules`, feature toggles, timestamp granularity, and
  test-support insertion of foreign names (§17.4). Done when a
  case-insensitive personality behaves like NTFS defaults.

- [ ] **HEAP-04: Durable state and pessimistic crashes.**
  Volatile and durable layers, `Sync` semantics per `Durability` level,
  directory sync, `crash(CrashPolicy::Pessimistic)`. Done when unsynced
  creates, renames, and writes vanish after a crash.

- [ ] **HEAP-05: Reordered and torn crashes.**
  Seeded `Reordered` and `Torn` policies. Done when one seed reproduces one
  post-crash state.

- [ ] **HEAP-06: Fault hooks.**
  `ENOSPC` after N bytes, `EIO` on the Nth sync with fsyncgate semantics,
  short reads and writes, `Busy` on rename. Done when each has a test.

- [ ] **HEAP-07: Crash harness feature.**
  `harness` enumerates crash points over an op sequence under each policy
  and reports the first violation with a reproducible seed (§24). Done when
  a deliberately broken atomic-write sequence is caught.

## `filer-api`: frontend

- [ ] **API-01: `Dir`.**
  Rights checks before submission, `restrict`, `open_dir`, `with_label`,
  default `Resolve` (§6). Done when rights violations return `RightsDenied`
  without reaching the driver (assert with a recording layer).

- [ ] **API-02: `Open` and `File`.**
  `Open<M>` and `File<M, D>` with `read_at`, `read_exact_at`,
  `write_all_at`, `append`, `set_len`, `metadata`, `close`;
  regular-files-only default (§8). Done when tests on `filer-heap` cover
  every mode, and the docs state the cancellation semantics.

- [ ] **API-03: Directory streams.**
  `ReadDir` and `Entry` with an inherent `next()`; `futures-core` `Stream`
  under the `futures` feature (§9). Done when foreign names are yielded and
  `.`/`..` never are.

- [ ] **API-04: Namespace operations.**
  `create_dir`, `remove_file`, `remove_dir`, `rename` (`NoReplace`,
  `Exchange`), `hard_link`, `symlink`, `read_link`, `stat` (follow and
  no-follow), `set_perms`, `set_times`; single-name ops against a resolved
  parent (§6.6). Done when each accepts `EntryName` where §5.5 allows and
  rejects foreign names where it forbids.

- [ ] **API-05: Coarse operations.**
  `read_to_end`, `read_to_string`, and an explicitly named non-atomic
  `write_in_place`. Done when each issues a single submission.

- [ ] **API-06: Ambient entry points.**
  `AmbientAuthority`, `ambient_authority()`, `Dir::open_ambient`,
  `Dir::from_owned_fd` / `from_owned_handle`, under `std` (§6.1). Done
  when docs suggest a `clippy::disallowed_methods` configuration.

## `filer-conformance`

- [ ] **CONF-01: Crate and runner.**
  Create `lib/filer-conformance/` with `run<D: Driver>(Dir<D>, Profile)`
  (§24, D26); first consumer is `filer-heap`'s test suite, using
  `pollster`. Done when it covers API-01 through API-04.

- [ ] **CONF-02: Confinement cases.**
  Absolute and relative symlink escapes, `InRoot`, `Deny`, magic links on
  real drivers, mount crossing. Done when every case asserts
  `EscapeBlocked` or the specified in-root result.

- [ ] **CONF-03: Foreign-name cases.**
  Enumerate, open, stat, remove, and rename-from a foreign name; reject it
  as a destination and across drivers. Done when `filer-heap` passes and
  `Profile` lets real drivers skip what the OS cannot create.

- [ ] **CONF-04: Cancellation by drop.**
  A mutating op dropped after first poll still completes. Done when
  verified on every driver.

- [ ] **CONF-05: Feature honesty.**
  Reported features work; unreported ones yield `Unsupported`. Done when
  verified for every `Features` field.

## `filer-posix`

- [ ] **POSIX-01: Crate, executors, payload.**
  In the existing `lib/filer-posix/` shell: `rustix` as a target-gated
  optional dependency enabled by `std`; `Posix::thread_pool(spawner)` and
  `Posix::inline()`; a public `OwnedFd`-based payload type for
  `filer-uring`. `#![forbid(unsafe_code)]` stays until a stopgap is needed
  (POSIX-10). Done when the crate still builds as an empty shell without
  `std`, on bare metal, on Windows, and on wasm (WASI included).

- [ ] **POSIX-02: Kernel-confined open.**
  `openat2` with each `Resolve` combination; bounded `EAGAIN` retry; the
  `ENOSYS`/`EPERM` switch to the userspace walk (per VER-04). Done when
  CONF-02 passes on Linux.

- [ ] **POSIX-03: Userspace walk.**
  The generic resolver (DRV-07) over `O_NOFOLLOW` primitives; forcible via
  configuration. Done when CONF-02 passes with the walk forced on Linux and
  by default on macOS.

- [ ] **POSIX-04: Data and metadata ops.**
  Read, write, `statx` (`fstat` fallback off Linux), `set_len`, close via
  the background closer. Done when CONF-01 passes in a temporary directory.

- [ ] **POSIX-05: Listing.**
  Batched `RawDir` with `d_type`; `DT_UNKNOWN` handling. Done when a
  directory larger than one batch lists completely.

- [ ] **POSIX-06: Namespace ops.**
  `mkdirat`, `unlinkat`, `renameat_with`, `linkat`, `symlinkat`,
  `readlinkat`, `fchmod`, `utimensat`. Done when API-04's tests pass.

- [ ] **POSIX-07: Feature detection.**
  Per directory, cached per mount (§16.4). Done when CONF-05 passes on
  ext4, tmpfs, and APFS.

- [ ] **POSIX-08: CI wiring.**
  Conformance against both executors on Ubuntu and macOS at MSRV and
  stable. Done when green.

- [ ] **POSIX-09: Advisory locks.**
  `flock`-based `Lock`/`Unlock` (D27); never `fcntl_lock`. Done when two
  descriptors in one process contend as expected and closing an unrelated
  descriptor does not release the lock.

- [ ] **POSIX-10: Stopgap `sys` module (only if needed).**
  For a gap that blocks a needed feature while its UP item is pending:
  `libc`-based wrappers with `// SAFETY:` comments, crate root switched from
  `forbid` to `deny` with `#[allow(unsafe_code)]` on `sys` only (§3.3),
  and the README's crate table updated. Done when the wrapper is tested and
  linked to its UP item for removal.

## `filer-windows`

- [ ] **WIN-01: Crate, payload, ambient roots.**
  In the existing `lib/filer-windows/` shell: target-gated `windows-sys`
  (features for `Win32_Foundation`, `Win32_Storage_FileSystem`,
  `Win32_System_IO`, `Win32_System_Ioctl`,
  `Win32_System_WindowsProgramming`, `Win32_System_SystemServices`,
  `Wdk_Foundation`, `Wdk_Storage_FileSystem`), a `sys` module holding all
  `unsafe` (the crate root switches from `forbid` to `deny`, §3.3), an
  `OwnedHandle` payload, and `open_ambient` via `CreateFileW` with `\\?\`
  and `FILE_FLAG_BACKUP_SEMANTICS`. Done when it builds as an empty shell
  elsewhere.

- [ ] **WIN-02: Relative opens and the userspace walk.**
  `NtCreateFile` with `RootDirectory`, `FILE_OPEN_REPARSE_POINT`, reparse
  data reading, and the generic resolver (§17.2). Done when CONF-02 passes
  on Windows, including junctions.

- [ ] **WIN-03: Data and metadata.**
  Positional read and write, `GetFileInformationByHandleEx` metadata,
  128-bit file IDs. Done when CONF-01 passes on Windows.

- [ ] **WIN-04: Listing.**
  `NtQueryDirectoryFile` in large batches. Done when a large directory
  lists completely.

- [ ] **WIN-05: Namespace ops.**
  Directory creation, relative rename via `FileRenameInfoEx`, delete via
  `FileDispositionInfoEx` (POSIX semantics, ignore read-only), hard links
  via `NtSetInformationFile`, symlinks as reparse points. Done when API-04's
  tests pass on Windows.

- [ ] **WIN-06: Names.**
  `InvalidNameForTarget` for reserved names, trailing dots and spaces, and
  forbidden characters; WTF-8 foreign names for unpaired surrogates. Done
  when tests create and enumerate an unpaired-surrogate name.

- [ ] **WIN-07: Sharing and retry.**
  Full share modes and the bounded `Busy` retry (D18) reported in
  `Error::fallback`. Done when a test holding a handle without
  `FILE_SHARE_DELETE` observes the retry and the final `Busy`.

- [ ] **WIN-08: Reparse-data fuzzing.**
  Keep reparse parsing in pure functions; add a fuzz target. Done when it
  builds in CI and runs under Miri in unit tests.

## Durability and atomic replacement

- [ ] **DUR-01: `sync` and poisoning.**
  Per-driver mapping (§13) and poisoning (D23). Done when `filer-heap`'s
  fsyncgate fault shows a second `sync` returning `Poisoned`.

- [ ] **DUR-02: Directory sync and `sync_filesystem`.**
  `Full` syncs changed parents; `syncfs` on `filer-posix`. Done when a
  recording layer shows the parent sync.

- [ ] **ATOM-01: `AtomicWriter` in `filer-api`.**
  §14 API with receipts reporting the strategy used, first on
  `filer-heap`. Done when a pessimistic crash at every step leaves the old
  or the new file.

- [ ] **ATOM-02: Linux path in `filer-posix`.**
  `O_TMPFILE` + `linkat`, named-temporary fallback, all `Replace` modes
  (§14.1, VER-03). Done when tests pass on tmpfs and on a filesystem
  without `O_TMPFILE` (vfat loopback).

- [ ] **ATOM-03: macOS path in `filer-posix`.**
  Named temporary, `renameat_with`, `F_FULLFSYNC`. Done when tested on the
  macOS job.

- [ ] **ATOM-04: Windows path in `filer-windows`.**
  Named temporary, flush, POSIX-semantics rename. Done when replacement
  works while another handle has the target open with `FILE_SHARE_DELETE`.

- [ ] **ATOM-05: `write_atomic` as one chain.**
  Done when a recording layer shows one submission.

- [ ] **ATOM-06: `Replace::IfUnchanged`.**
  Token check before visibility, race window documented (§14.2). Done when
  a changed target aborts with a precondition error (kind decided and
  recorded in §19).

- [ ] **ATOM-07: `sweep_temporaries`.**
  Done when tested on `filer-heap` after a crash.

## Metadata, attributes, clones, and sparse files

- [ ] **META-01: Full metadata mapping.**
  Every `MetaMask` field in `filer-posix` and `filer-windows`. Done when
  unsupported fields are `None` and tests assert presence where expected.

- [ ] **META-02: Change tokens and `same_file`.**
  Per VER-08. Done when tokens change on write and rename-over.

- [ ] **XATTR-01: Extended attributes.**
  Linux namespaces, macOS flat mapping (§11.1); listing yields foreign
  names; `Features::xattr` populated. Done when conformance covers set,
  get, list, and remove, skipping where unsupported.

- [ ] **XATTR-02: File flags.**
  Read and set `FileAttrs` where supported; `nocow` at creation. Done when
  tested on a filesystem supporting each flag.

- [ ] **COPY-01: `copy_file` on Linux.**
  `CloneMode`, `FICLONE`, `copy_file_range`, sparse-preserving fallback,
  `CopyReport` (§12). Done when a btrfs or XFS loopback reports
  `cloned == bytes`, and ext4 reports zero with `Prefer` and `Unsupported`
  with `Require`.

- [ ] **COPY-02: Range clones and dedupe.**
  After UP-02 (or POSIX-10). Done when misaligned ranges fail before
  submission.

- [ ] **COPY-03: Extents, allocation, holes.**
  Done when a sparse file's extents survive a sparse-preserving copy.

- [ ] **COPY-04: Preservation reporting.**
  Done when copying an xattr-bearing file to a filesystem without xattrs
  reports `XATTRS` in `dropped`.

- [ ] **COPY-05: APFS and ReFS clones.**
  `fclonefileat` and `FSCTL_DUPLICATE_EXTENTS_TO_FILE`. Done when tested on
  the macOS job and on ReFS or Dev Drive (manually if CI lacks it; record
  how).

## Walking and recursive operations

- [ ] **WALK-01: Walker core in `filer-api`.**
  Handle-relative descent, `WalkOpts`, pruning, error policy, order (§20).
  Done when foreign names and `DT_UNKNOWN` are handled.

- [ ] **WALK-02: Descriptor budget.**
  Close and reopen ancestors with identity checks and `Moved`. Done when a
  deep tree walks under a low budget and a swapped ancestor yields `Moved`.

- [ ] **WALK-03: Symlinks and mounts.**
  Done when a symlink cycle terminates and `cross_mounts` is honored.

- [ ] **WALK-04: `remove_tree`.**
  Post-order, never follows links, foreign names, Windows read-only and
  `Busy` handling. Done when a regression test reproduces the
  CVE-2022-21658 pattern (swap a directory for a symlink mid-walk) and
  nothing outside the tree is touched.

- [ ] **WALK-05: `copy_tree`.**
  Clones where possible, `Preserve`, optional hardlink preservation,
  special files skipped and reported, aggregate `CopyReport`. Done when a
  tree with hardlinks and a FIFO copies as specified.

- [ ] **WALK-06: `disk_usage`.**
  Done when it sums `allocated` without hydrating a dataless personality.

- [ ] **WALK-07: Native `Op::Walk` in `filer-posix`.**
  Batched traversal on a worker. Done when it passes the walker
  conformance cases and benchmarks show fewer hops.

## Crash properties

- [ ] **CRASH-01: Atomic writer properties.**
  Old-or-new after every crash point and policy, for every `Replace` mode,
  via HEAP-07. Done when green.

- [ ] **CRASH-02: Differential testing.**
  Proptest op sequences against `filer-heap` and `filer-posix`, comparing
  observable state. Done when it runs in CI with a bounded case count.

- [ ] **CRASH-03: LazyFS job.**
  Optional Linux job running CRASH-01 on LazyFS. Done when it passes, or
  when §24 records why hosted runners cannot run it.

## `filer-tx`

Created with `publish = false` (D16).

- [ ] **TX-01: Crate, API, staging.** `Transaction`, `TxOpts`, `Atomicity`,
  receipts; staging only, with free rollback before commit (§15). Done when
  dropping an uncommitted transaction leaves no visible change.
- [ ] **TX-02: Single-entry strategy.** Done when receipts report `Atomic`.
- [ ] **TX-03: Journal format.** Per §15.2 with CRC32C and a fuzz target.
  Done when torn tails and bit flips are rejected.
- [ ] **TX-04: Journaled strategy.** Idempotent application per §15.3. Done
  when re-applying a fully applied journal changes nothing.
- [ ] **TX-05: Recovery.** `filer_tx::recover` with a report. Done when a
  crash during recovery followed by a second recovery converges.
- [ ] **TX-06: Subtree swap and symlink flip.** Done when readers holding
  the old root keep a consistent view.
- [ ] **TX-07: Locks.** `Lock::Advisory` via OS locks across processes and
  `async-lock` within one (§7.7). Done when two transactions serialize.
- [ ] **TX-08: Preconditions.** `expect_unchanged` aborts before the commit
  point. Done when tested.
- [ ] **TX-09: Post-commit rollback and finalize.** Done when rollback
  restores pre-images and finalize removes them.
- [ ] **TX-10: Crash gate.** All-or-nothing after recovery for every
  strategy and policy. Done when green; then propose `publish = true`.

## `filer-uring`

- [ ] **URING-01: Backend spike.** Prototype open, read, and a linked
  write-then-fsync chain with direct descriptors on `io-uring` and on
  `compio-driver`. Done when §17.3 records the choice and its `unsafe`
  surface.
- [ ] **URING-02: Crate, ring thread, fallback.** Create the crate with a
  `sys` module; dedicated completion thread; opcode probe;
  `Uring::new_or_posix`. Done when CI exercises both setup paths.
- [ ] **URING-03: Basic ops.** `openat2`, read, write, `statx`, fsync,
  close; everything else routed to `Posix`. Done when the conformance suite
  passes.
- [ ] **URING-04: Native chains.** Linked SQEs with direct descriptors. Done
  when `write_atomic` is one submission.
- [ ] **URING-05: Registered buffers.** `PooledBuf`. Done when benchmarks
  show the effect.
- [ ] **URING-06: Benchmarks.** Metadata-heavy small files, large
  sequential, random 4 KiB, versus `Posix`. Done when results and machine
  details are recorded in §17.3.

## Virtual drivers

- [ ] **VFS-01: `filer-vfs` mount table.** Done when the conformance suite
  passes over two `filer-heap` mounts.
- [ ] **VFS-02: Cross-mount behavior.** Done when component-wise resolution
  and `CrossesDevices` are tested.
- [ ] **OVL-01: `filer-overlay` reads.** Done when layered lookup and merged
  listings are tested.
- [ ] **OVL-02: `filer-overlay` writes.** Copy-up, whiteouts, opaque
  directories. Done when the conformance suite passes.
- [ ] **ARC-01: `filer-archive` zip.** Decide Q7 first. Done when a fuzz
  target covers member-name quarantine.
- [ ] **ARC-02: `filer-archive` tar.** Done when tar archives pass the
  read-only conformance subset.

## Middleware

- [ ] **TRACE-01: `filer-tracing`.** One span per op with the fields in §18.
  Done when a test subscriber observes them.
- [ ] **FAULT-01: `filer-faults`.** Seeded, by op kind and path pattern.
  Done when a seed reproduces a fault schedule.
- [ ] **QUOTA-01: `filer-quota`.** Done when byte and entry limits trigger
  `QuotaExceeded`, with reservations held across awaits via `async-lock`.
- [ ] **REPLAY-01: `filer-replay`.** Done when a recorded `filer-posix`
  session replays against `filer-heap` to the same observable state.

## Adapters

- [ ] **ADP-01: `filer-tokio`.** `TokioSpawner`, plus `tokio::io` traits for
  `filer-api`'s cursor. Done when the conformance suite runs on a Tokio
  runtime and `tokio::io::copy` works between cursors.
- [ ] **ADP-02: Cursor and `futures-io`.** `filer-api`'s `futures-io`
  feature. Done when tested.
- [ ] **ADP-03: `bytes` buffers.** `IoBuf` for `Bytes` and `BytesMut`. Done
  when tested without `std`.
- [ ] **ADP-04: `LocalDriver`.** Per D13, once a `!Send` driver exists.

## Platform breadth and exports

- [ ] **PLAT-01: FreeBSD.** At least a cross-check build of `filer-posix`,
  ideally a VM-based CI job. Done when one runs.
- [ ] **PLAT-02: WASI.** Implement the `filer-wasi` shell per §17.8
  (syscall layer per VER-02). Done when a conformance subset passes under
  Wasmtime on `wasm32-wasip1` and `wasm32-wasip2`.
- [ ] **PLAT-03: Landlock helper (idea).** Derive a ruleset from held
  `Dir`s (§6.7). Done when designed in `DESIGN.md`.
- [ ] **EXP-01: `filer-fuse`.** Done when `filer-heap` mounts and
  `ls`/`cat` work in a Linux CI job.
- [ ] **EXP-02: `filer-wasi-host`.** Done when a WASI 0.3 guest lists a
  `filer-heap` directory.

## Umbrella crate

Blocked on ASK-01.

- [ ] **UMB-01: Crate and re-exports.** Re-export `filer-api` at the root
  (or not, per Q5) and component crates as modules. Done when re-export
  tests pass with and without `std`.
- [ ] **UMB-02: Feature forwarding.** `all` covers `no_std` components;
  `std` also enables native drivers (D25). Done when forwarding tests pass
  and every feature builds alone.

## CI and documentation

- [ ] **CI-01: Filesystem fixtures.** Loopback btrfs or XFS (clones), vfat
  (no `O_TMPFILE`, no symlinks), and tmpfs on Ubuntu. Done when platform
  tests select fixtures by environment variable and skip cleanly without.
- [ ] **CI-02: Miri.** For `filer-api`, `filer-heap`, routing drivers, and
  pure parts of `sys` modules. Done when green.
- [ ] **CI-03: Fuzz build check.** Done when fuzz targets build in CI.
- [ ] **CI-04: `cargo-semver-checks`.** Once a release exists, per crate.
- [ ] **DOC-01: Crate overviews.** Rustdoc landing pages with feature-gated
  examples passing the packaged-doctest job in all four feature modes.
- [ ] **DOC-02: Guarantee tables.** `Durability`, `Enforcement`,
  `Atomicity`, `CloneMode`: what each platform delivers, in rustdoc.
- [ ] **DOC-03: README crate table.** Keep the `unsafe` column accurate as
  `sys` modules appear or disappear.

## 0.1.0 release criteria

Not tasks. Experimental 0.0.x releases need none of these (D29); check
them before proposing 0.1.0:

- conformance green for `filer-heap` and both `filer-posix` executors on
  Ubuntu and macOS, and for `filer-windows` on Windows, at MSRV and stable;
- atomic replacement with crash properties green;
- walker and `remove_tree` with the CVE-2022-21658 regression test;
- rustdoc on every public item, no warnings under docs.rs settings;
- every crate builds `no_std` with `all` on `thumbv7em-none-eabihf`, with
  all features on `wasm32-unknown-unknown`, and with every feature alone;
- `filer-tx` and `filer-uring` stay `publish = false` unless TX-10 and
  URING-06 are met;
- `CHANGES.md` describes the release.

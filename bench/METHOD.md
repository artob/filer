# Benchmarking the `decode-*` crates

How each `decode-*` crate's `BENCH.md` is measured. The figures serve two
ends: each decoder's own throughput, memory, and overheads, tracked from
release to release; and how it compares with the main alternatives, the
reference C implementation and the leading pure-Rust decoders, on the same
inputs, machines, and builds. A figure is reported with its machine, its
build, and its confidence interval, or not at all.

The harness that applies this protocol lives beside this file (see
"Harness"), and so far applies only part of it (`TODO.md`, BENCH-04 to
07); until it applies all of it, no `BENCH.md` holds results.

## What is measured

1. **Whole streams**, the headline: a stream held in memory decoded into
   a preallocated buffer large enough for its output. Throughput is in MB/s
   (10^6 bytes a second) of decoded output, and also of packed input.
2. **Streaming**: input given and output taken in pieces of 4 KiB and of
   64 KiB, as callers reading files or sockets do. A counterpart that only
   decodes whole buffers (as libdeflate does) appears in whole-stream
   tables only, marked so.
3. **Small streams**: the time per stream for packed streams of about
   64 B, 1 KiB, and 16 KiB, the decoder's construction included, which
   dominates archives of many small members.
4. **Memory**: peak heap and the count of allocations per stream, by a
   counting allocator, in a pass of its own that is not timed; and the size
   of the decoder value itself.
5. **Checkpoints**, for decoders that offer them: the time to take one
   and to resume from one, and the bytes each holds.
6. **Hardware counters** on Linux: cycles, instructions, IPC, branch
   misses, and L1 data cache misses per output byte, by `perf stat`, to
   explain differences, never to rank.
7. **Code size**: the bytes of `.text` the decoder adds to a minimal
   `#![no_std]` binary, at `opt-level` 3 and `"s"`.

## Inputs

Public corpora, never committed, fetched by the harness and verified
against the SHA-256 that `bench/corpus.toml` records for each file:

- **Silesia**, the standard mixed set: 12 files, 211 MB of text,
  executables, databases, medical and astronomical images, and XML.
- **enwik8**: 100 MB of Wikipedia's XML, large and regular.
- **Canterbury**, large and small sets: classic small files.
- **A Linux kernel source tarball** of a pinned release: what archives of
  source code hold. Its first 100 MB, of Linux 6.12, an LTS release: the
  whole, 1.5 GB, would take hours at bzip2's speed.
- **Executables**: the binaries of a pinned Rust release, as they ship:
  `rustc`, `librustc_driver`, and `cargo` of Rust 1.97.0 for x86-64
  Linux.

And synthetic edge cases, generated from a fixed seed: bytes that do not
compress (stored blocks); zeros (runs); a short pattern repeated
(overlapping matches); and repeats farther apart than small windows reach.

Each is encoded by its codec's reference encoder, at a pinned version, at
levels spanning its range (each `BENCH.md` names them). Encoding happens
once, on one machine, which records the packed files' SHA-256 in
`bench/packed.toml`, beside the manifest; other machines encode again,
on one thread, as deterministically, and check that their bytes match,
so that every machine decodes the same bytes. After every timed run, the
decoded output is checked against the original, verified by its SHA-256:
a run that decodes wrongly is discarded, and its decoder disqualified on
that input.

## Counterparts

Each `BENCH.md` names its codec's counterparts:

- **The reference C implementation**, through its Rust binding: the bar
  to meet.
- **The leading pure-Rust decoders.**
- **Fastest safe configuration**: each runs in the fastest configuration
  it offers for decoding, with that configuration recorded; and also with
  only safe code, where a crate offers that choice.
- **Checksums**: they are verified wherever the format carries them, by
  every decoder that can, as ours always do; a counterpart that cannot is
  marked.
- **Black boxes**: counterparts are built and run, never read, as
  clean-room work requires. Those under copyleft licenses are used only in
  the unpublished harness, and never distributed.

## Machines

Only 64-bit x86 and ARM, each on at least two microarchitectures, one
current and one older or smaller, on bare metal where possible:

- **x86-64**: a current AMD (Zen 4 or later) and a current Intel core,
  each bare metal; and a cloud VM, as the typical server.
- **AArch64**: Apple Silicon (M4) under macOS; and a Neoverse server
  (Graviton 4, or Ampere) under Linux.
- **Optionally**: a Cortex-A76 board, such as the Raspberry Pi 5, as the
  small end.

Results name the CPU model, the cores used, the frequency policy, the
memory, the OS and its kernel, and whether the machine is virtual. Figures
from VMs carry their noise, and are never the headline.

## Builds

- **Toolchain and profile**: a pinned Rust toolchain, at least the crates'
  MSRV, in release with `lto = "fat"`, `codegen-units = 1`, and
  `panic = "abort"`, for ours and the Rust counterparts alike.
- **C counterparts**: built by their `-sys` crates with a pinned C compiler
  and flags, all recorded.
- **Two targets per architecture**: the baseline, which users of
  distributed binaries get (x86-64 with SSE2, AArch64 with NEON), the
  headline; and `target-cpu=native`, the ceiling, reported apart.
- **Runtime dispatch**: CPU-feature dispatch at runtime, as some C
  libraries do, is part of what a library is, and is left on.

## Procedure

- **Environment**: on Linux, the `performance` governor, frequency
  boost off, the harness pinned to an isolated core whose SMT sibling is
  idle, and nothing else running. On macOS, which pins nothing, on mains
  power, Low Power Mode off, and nothing else running. The state is
  recorded either way.
- **Warm caches**: inputs and output buffers are in memory and touched
  before timing, and each pair of decoder and input warms up for at least
  a second. Decoding is measured, not I/O.
- **Samples**: each pair of decoder and input runs in at least 5
  independent processes, of at least 10 timed iterations or half a second,
  whichever is longer. Within each process, the order of decoders is
  randomized and interleaved, so that drift and heat fall on all alike.
- **Statistics**: within a process, the median iteration; across
  processes, the median of those medians, with a 95% bootstrap confidence
  interval, and the fastest run too. Runs continue until the interval's
  half-width is under 1% of the median, or a time budget is spent, and the
  width achieved is reported.
- **Comparisons**: each input's speed ratio, ours to the counterpart's,
  with a confidence interval by paired bootstrap; summarized per level as
  the geometric mean of ratios across inputs, never as the mean of MB/s.
- **Outliers**: they are kept, not trimmed, as medians bound their
  effect. A bimodal distribution, which heat or frequency changes cause,
  is investigated and the run repeated, not averaged.

## Reporting

- **Raw data**: every sample, as JSON lines with its metadata (machine,
  OS, kernel, CPU, frequency policy, toolchain, each crate's version and
  features, compiler flags, the corpus manifest's hash, the commit, and
  the date), under `bench/results/`.
- **Each `BENCH.md`, per machine**:
  - a table per input set, with a row per input and level, a column per
    decoder in MB/s (the median and its interval), and ours relative to
    the reference C implementation;
  - then streaming, small streams, memory, and checkpoints;
  - and a few sentences on where the decoder stands and why, which the
    counters support.
- **Generation**: the tables are generated by the harness, never edited
  by hand. Results older than the release they describe are marked so.

## Regressions

Before each release of a crate, its decoder is measured again, on an x86
machine on bare metal and on Apple Silicon, over the whole corpus. Being
slower on any input by more than its interval and 3% blocks the release
until it is explained. Shared CI runners are too noisy to time anything:
CI only builds the harness.

## Harness

The harness is an unpublished crate in `bench/`, with a `[workspace]` of
its own, outside the crates' workspace, as fuzzers are. It is the one
place where the decoders meet C, through their counterparts' bindings, and
no published crate depends on it. It drives every decoder through one
interface: whole streams into a buffer, and streams in pieces of a given
size. It fetches and verifies the corpus, encodes it once, runs the
procedure above, writes the raw data, and generates the tables.

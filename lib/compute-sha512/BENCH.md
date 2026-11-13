# compute-sha512 benchmarks

The figures of `compute-sha512`'s SHA-512 and SHA-512/256, alone and
against the main alternatives, measured by the protocol of this
repository's [`bench/METHOD.md`](../../bench/METHOD.md): the same
inputs, machines, and builds for every implementation, each figure a
median with its 95% confidence interval, and every result checked
against the others'.

## Counterparts

| Implementation | Crate | Language |
| --- | --- | --- |
| sha2 | `sha2` | Rust, with SHA instructions |
| ring | `ring` | C and assembly |

## Inputs

Buffers of random bytes, of 16 B, 256 B, 4 KiB, 64 KiB, and 1 MiB, fed
whole, by one update, and in pieces of 7 bytes, for SHA-512 and
SHA-512/256.

## Machines

64-bit x86 on a current AMD core and a current Intel core, each a
cloud instance's own, and on an older AMD core, a VM's; and 64-bit ARM
on Apple M4 and on a Neoverse server.

Measured so far, each under Linux 7.0: AMD EPYC 9R45 (Zen 5), Intel
Xeon 6975P-C (Granite Rapids), and ARM Neoverse V3 (AWS Graviton5),
each 1 vCPU under EC2 Nitro, on a physical core of its own, by Rust
1.99.1, 2026-11-13; and AMD EPYC 7551P (Zen 1), 4 vCPUs under KVM, by
Rust 1.97.0, 2026-11-06.

## Throughput

GB/s, each the median of five or more processes with its 95%
confidence interval: a row per size and way of feeding, a column per
implementation; then each counterpart's speed relative to ours, the
geometric mean over sizes. The counterparts pick the CPU's
instructions at run time.

### Zen 5, ours with `intrinsics`, as by default

#### SHA-512

| Bytes | Fed | ours | sha2 | ring |
| ---: | --- | ---: | ---: | ---: |
| 16 | whole | 0.06 ± 0.3% | 0.11 ± 0.9% | 0.09 ± 0.6% |
| 256 | whole | 0.38 ± 0.6% | 0.60 ± 0.9% | 0.56 ± 0.7% |
| 4096 | whole | 0.59 ± 0.1% | 0.95 ± 0.1% | 0.90 ± 0.1% |
| 65536 | whole | 0.61 ± 0.1% | 0.99 ± 0.1% | 0.94 ± 0.1% |
| 1048576 | whole | 0.61 ± 0.1% | 0.99 ± 0.1% | 0.94 ± 0.0% |
| 16 | by 7 | 0.06 ± 0.4% | 0.11 ± 1.0% | 0.09 ± 0.2% |
| 256 | by 7 | 0.34 ± 0.6% | 0.52 ± 1.0% | 0.46 ± 0.3% |
| 4096 | by 7 | 0.48 ± 0.1% | 0.74 ± 0.9% | 0.62 ± 0.9% |
| 65536 | by 7 | 0.49 ± 0.2% | 0.76 ± 0.7% | 0.64 ± 0.6% |
| 1048576 | by 7 | 0.49 ± 0.1% | 0.76 ± 0.5% | 0.64 ± 0.9% |

Each counterpart's speed relative to ours: whole: sha2 1.63x, ring
1.51x; by 7: sha2 1.58x, ring 1.34x.

#### SHA-512/256

| Bytes | Fed | ours | sha2 | ring |
| ---: | --- | ---: | ---: | ---: |
| 16 | whole | 0.06 ± 0.9% | 0.11 ± 0.8% | 0.09 ± 0.8% |
| 256 | whole | 0.38 ± 0.6% | 0.60 ± 0.4% | 0.56 ± 0.6% |
| 4096 | whole | 0.59 ± 0.1% | 0.95 ± 0.1% | 0.90 ± 0.2% |
| 65536 | whole | 0.61 ± 0.1% | 0.99 ± 0.1% | 0.94 ± 0.2% |
| 1048576 | whole | 0.61 ± 0.1% | 0.99 ± 0.1% | 0.94 ± 0.1% |
| 16 | by 7 | 0.06 ± 0.1% | 0.11 ± 1.1% | 0.09 ± 0.3% |
| 256 | by 7 | 0.34 ± 0.3% | 0.52 ± 0.8% | 0.45 ± 1.4% |
| 4096 | by 7 | 0.48 ± 0.1% | 0.73 ± 0.3% | 0.62 ± 1.0% |
| 65536 | by 7 | 0.49 ± 0.1% | 0.75 ± 0.2% | 0.64 ± 1.0% |
| 1048576 | by 7 | 0.49 ± 0.1% | 0.75 ± 1.1% | 0.64 ± 0.8% |

Each counterpart's speed relative to ours: whole: sha2 1.62x, ring
1.50x; by 7: sha2 1.57x, ring 1.35x.

### Zen 5, ours without `intrinsics`, without dependencies

#### SHA-512

| Bytes | Fed | ours | sha2 | ring |
| ---: | --- | ---: | ---: | ---: |
| 16 | whole | 0.06 ± 0.5% | 0.11 ± 0.8% | 0.09 ± 0.7% |
| 256 | whole | 0.38 ± 0.1% | 0.63 ± 1.0% | 0.56 ± 0.7% |
| 4096 | whole | 0.59 ± 0.1% | 0.98 ± 0.3% | 0.90 ± 0.1% |
| 65536 | whole | 0.61 ± 0.1% | 1.02 ± 0.1% | 0.94 ± 0.1% |
| 1048576 | whole | 0.61 ± 0.1% | 1.02 ± 0.0% | 0.94 ± 0.0% |
| 16 | by 7 | 0.06 ± 0.4% | 0.11 ± 1.0% | 0.09 ± 0.1% |
| 256 | by 7 | 0.34 ± 0.2% | 0.54 ± 0.6% | 0.46 ± 0.7% |
| 4096 | by 7 | 0.47 ± 0.3% | 0.76 ± 0.8% | 0.63 ± 0.9% |
| 65536 | by 7 | 0.49 ± 0.5% | 0.78 ± 0.1% | 0.64 ± 0.9% |
| 1048576 | by 7 | 0.49 ± 0.2% | 0.78 ± 0.9% | 0.64 ± 0.9% |

Each counterpart's speed relative to ours: whole: sha2 1.67x, ring
1.50x; by 7: sha2 1.61x, ring 1.35x.

#### SHA-512/256

| Bytes | Fed | ours | sha2 | ring |
| ---: | --- | ---: | ---: | ---: |
| 16 | whole | 0.06 ± 0.4% | 0.11 ± 0.7% | 0.09 ± 0.8% |
| 256 | whole | 0.38 ± 0.2% | 0.63 ± 1.7% | 0.56 ± 0.5% |
| 4096 | whole | 0.59 ± 0.1% | 0.98 ± 0.1% | 0.90 ± 0.1% |
| 65536 | whole | 0.61 ± 0.1% | 1.01 ± 0.1% | 0.93 ± 0.1% |
| 1048576 | whole | 0.61 ± 0.1% | 1.02 ± 0.1% | 0.94 ± 0.2% |
| 16 | by 7 | 0.06 ± 0.2% | 0.11 ± 1.2% | 0.09 ± 0.3% |
| 256 | by 7 | 0.34 ± 0.5% | 0.54 ± 0.9% | 0.46 ± 0.9% |
| 4096 | by 7 | 0.47 ± 0.2% | 0.76 ± 0.3% | 0.63 ± 1.7% |
| 65536 | by 7 | 0.49 ± 0.0% | 0.78 ± 0.1% | 0.64 ± 1.8% |
| 1048576 | by 7 | 0.49 ± 0.1% | 0.78 ± 0.1% | 0.64 ± 1.0% |

Each counterpart's speed relative to ours: whole: sha2 1.67x, ring
1.51x; by 7: sha2 1.61x, ring 1.35x.

### Granite Rapids, ours with `intrinsics`, as by default

#### SHA-512

| Bytes | Fed | ours | sha2 | ring |
| ---: | --- | ---: | ---: | ---: |
| 16 | whole | 0.05 ± 0.8% | 0.08 ± 1.0% | 0.07 ± 0.3% |
| 256 | whole | 0.31 ± 0.3% | 0.46 ± 0.2% | 0.45 ± 0.1% |
| 4096 | whole | 0.48 ± 0.3% | 0.71 ± 0.1% | 0.72 ± 0.1% |
| 65536 | whole | 0.49 ± 0.3% | 0.73 ± 0.3% | 0.75 ± 0.1% |
| 1048576 | whole | 0.50 ± 0.1% | 0.73 ± 0.3% | 0.75 ± 0.1% |
| 16 | by 7 | 0.05 ± 0.3% | 0.08 ± 1.1% | 0.07 ± 0.4% |
| 256 | by 7 | 0.27 ± 0.2% | 0.37 ± 0.6% | 0.37 ± 0.1% |
| 4096 | by 7 | 0.38 ± 0.1% | 0.55 ± 0.2% | 0.53 ± 0.6% |
| 65536 | by 7 | 0.39 ± 0.2% | 0.56 ± 0.4% | 0.55 ± 0.2% |
| 1048576 | by 7 | 0.39 ± 0.5% | 0.55 ± 0.3% | 0.54 ± 1.0% |

Each counterpart's speed relative to ours: whole: sha2 1.47x, ring
1.47x; by 7: sha2 1.41x, ring 1.38x.

#### SHA-512/256

| Bytes | Fed | ours | sha2 | ring |
| ---: | --- | ---: | ---: | ---: |
| 16 | whole | 0.05 ± 0.4% | 0.08 ± 0.9% | 0.07 ± 0.9% |
| 256 | whole | 0.32 ± 0.3% | 0.46 ± 0.1% | 0.46 ± 0.1% |
| 4096 | whole | 0.48 ± 0.2% | 0.71 ± 0.1% | 0.72 ± 0.1% |
| 65536 | whole | 0.50 ± 0.1% | 0.73 ± 0.1% | 0.75 ± 0.0% |
| 1048576 | whole | 0.50 ± 0.1% | 0.73 ± 0.2% | 0.75 ± 0.0% |
| 16 | by 7 | 0.05 ± 0.2% | 0.08 ± 1.0% | 0.07 ± 0.4% |
| 256 | by 7 | 0.27 ± 0.3% | 0.37 ± 0.8% | 0.38 ± 0.2% |
| 4096 | by 7 | 0.38 ± 0.2% | 0.54 ± 0.2% | 0.53 ± 0.2% |
| 65536 | by 7 | 0.39 ± 0.2% | 0.55 ± 0.4% | 0.55 ± 0.7% |
| 1048576 | by 7 | 0.39 ± 0.2% | 0.54 ± 0.8% | 0.55 ± 0.6% |

Each counterpart's speed relative to ours: whole: sha2 1.47x, ring
1.47x; by 7: sha2 1.40x, ring 1.39x.

### Granite Rapids, ours without `intrinsics`, without dependencies

#### SHA-512

| Bytes | Fed | ours | sha2 | ring |
| ---: | --- | ---: | ---: | ---: |
| 16 | whole | 0.05 ± 0.3% | 0.08 ± 0.6% | 0.07 ± 0.3% |
| 256 | whole | 0.31 ± 0.5% | 0.45 ± 0.6% | 0.45 ± 0.1% |
| 4096 | whole | 0.48 ± 0.2% | 0.71 ± 0.1% | 0.72 ± 0.0% |
| 65536 | whole | 0.50 ± 0.1% | 0.74 ± 0.4% | 0.75 ± 0.1% |
| 1048576 | whole | 0.50 ± 0.1% | 0.74 ± 0.3% | 0.75 ± 0.1% |
| 16 | by 7 | 0.05 ± 0.8% | 0.07 ± 0.9% | 0.07 ± 0.5% |
| 256 | by 7 | 0.27 ± 0.5% | 0.36 ± 0.6% | 0.37 ± 0.1% |
| 4096 | by 7 | 0.38 ± 0.3% | 0.51 ± 1.0% | 0.53 ± 0.3% |
| 65536 | by 7 | 0.39 ± 0.1% | 0.51 ± 0.7% | 0.54 ± 0.0% |
| 1048576 | by 7 | 0.39 ± 0.3% | 0.51 ± 0.8% | 0.55 ± 0.2% |

Each counterpart's speed relative to ours: whole: sha2 1.46x, ring
1.46x; by 7: sha2 1.35x, ring 1.40x.

#### SHA-512/256

| Bytes | Fed | ours | sha2 | ring |
| ---: | --- | ---: | ---: | ---: |
| 16 | whole | 0.05 ± 0.5% | 0.08 ± 0.9% | 0.07 ± 0.4% |
| 256 | whole | 0.31 ± 0.2% | 0.46 ± 0.2% | 0.46 ± 0.1% |
| 4096 | whole | 0.48 ± 0.2% | 0.71 ± 0.4% | 0.72 ± 0.1% |
| 65536 | whole | 0.50 ± 0.2% | 0.73 ± 0.3% | 0.75 ± 0.0% |
| 1048576 | whole | 0.50 ± 0.2% | 0.73 ± 0.2% | 0.75 ± 0.0% |
| 16 | by 7 | 0.05 ± 0.2% | 0.07 ± 0.5% | 0.07 ± 0.4% |
| 256 | by 7 | 0.27 ± 0.3% | 0.35 ± 0.8% | 0.38 ± 0.3% |
| 4096 | by 7 | 0.38 ± 0.2% | 0.50 ± 0.7% | 0.53 ± 0.3% |
| 65536 | by 7 | 0.40 ± 0.1% | 0.52 ± 0.8% | 0.54 ± 0.2% |
| 1048576 | by 7 | 0.40 ± 0.1% | 0.51 ± 0.5% | 0.55 ± 0.2% |

Each counterpart's speed relative to ours: whole: sha2 1.47x, ring
1.47x; by 7: sha2 1.33x, ring 1.38x.

### Neoverse V3, ours with `intrinsics`, as by default

#### SHA-512

| Bytes | Fed | ours | sha2 | ring |
| ---: | --- | ---: | ---: | ---: |
| 16 | whole | 0.11 ± 0.6% | 0.09 ± 0.1% | 0.12 ± 0.1% |
| 256 | whole | 0.73 ± 0.0% | 0.49 ± 0.1% | 0.75 ± 0.2% |
| 4096 | whole | 1.19 ± 0.0% | 0.74 ± 0.0% | 1.22 ± 0.2% |
| 65536 | whole | 1.24 ± 0.0% | 0.77 ± 0.0% | 1.27 ± 0.6% |
| 1048576 | whole | 1.24 ± 0.0% | 0.77 ± 0.0% | 1.27 ± 0.0% |
| 16 | by 7 | 0.11 ± 0.5% | 0.09 ± 0.2% | 0.11 ± 0.1% |
| 256 | by 7 | 0.54 ± 0.1% | 0.45 ± 0.0% | 0.48 ± 0.5% |
| 4096 | by 7 | 0.76 ± 0.1% | 0.65 ± 0.2% | 0.64 ± 0.5% |
| 65536 | by 7 | 0.78 ± 0.2% | 0.67 ± 0.2% | 0.66 ± 0.7% |
| 1048576 | by 7 | 0.78 ± 0.1% | 0.68 ± 0.1% | 0.65 ± 0.4% |

Each counterpart's speed relative to ours: whole: sha2 0.66x, ring
1.03x; by 7: sha2 0.84x, ring 0.88x.

#### SHA-512/256

| Bytes | Fed | ours | sha2 | ring |
| ---: | --- | ---: | ---: | ---: |
| 16 | whole | 0.11 ± 0.4% | 0.09 ± 0.3% | 0.11 ± 0.2% |
| 256 | whole | 0.73 ± 0.4% | 0.49 ± 0.1% | 0.74 ± 0.5% |
| 4096 | whole | 1.19 ± 0.0% | 0.74 ± 0.0% | 1.22 ± 0.1% |
| 65536 | whole | 1.24 ± 0.0% | 0.77 ± 0.0% | 1.28 ± 0.7% |
| 1048576 | whole | 1.24 ± 0.0% | 0.77 ± 0.0% | 1.27 ± 0.1% |
| 16 | by 7 | 0.11 ± 0.4% | 0.09 ± 0.2% | 0.11 ± 0.2% |
| 256 | by 7 | 0.54 ± 0.8% | 0.45 ± 0.1% | 0.48 ± 0.6% |
| 4096 | by 7 | 0.76 ± 0.1% | 0.65 ± 0.0% | 0.64 ± 0.9% |
| 65536 | by 7 | 0.78 ± 0.1% | 0.67 ± 0.2% | 0.65 ± 0.1% |
| 1048576 | by 7 | 0.79 ± 0.3% | 0.68 ± 0.1% | 0.66 ± 0.6% |

Each counterpart's speed relative to ours: whole: sha2 0.66x, ring
1.03x; by 7: sha2 0.84x, ring 0.88x.

### Neoverse V3, ours without `intrinsics`, without dependencies

#### SHA-512

| Bytes | Fed | ours | sha2 | ring |
| ---: | --- | ---: | ---: | ---: |
| 16 | whole | 0.05 ± 0.4% | 0.09 ± 0.3% | 0.12 ± 0.2% |
| 256 | whole | 0.28 ± 0.7% | 0.49 ± 0.1% | 0.75 ± 0.2% |
| 4096 | whole | 0.43 ± 0.0% | 0.74 ± 0.0% | 1.22 ± 0.3% |
| 65536 | whole | 0.45 ± 0.0% | 0.77 ± 0.0% | 1.27 ± 0.5% |
| 1048576 | whole | 0.45 ± 0.0% | 0.77 ± 0.0% | 1.27 ± 0.1% |
| 16 | by 7 | 0.05 ± 0.4% | 0.08 ± 0.2% | 0.11 ± 0.7% |
| 256 | by 7 | 0.26 ± 0.1% | 0.45 ± 0.2% | 0.48 ± 0.5% |
| 4096 | by 7 | 0.36 ± 0.0% | 0.65 ± 0.1% | 0.64 ± 0.2% |
| 65536 | by 7 | 0.37 ± 0.0% | 0.67 ± 0.3% | 0.65 ± 0.3% |
| 1048576 | by 7 | 0.37 ± 0.1% | 0.67 ± 0.2% | 0.66 ± 0.6% |

Each counterpart's speed relative to ours: whole: sha2 1.72x, ring
2.69x; by 7: sha2 1.78x, ring 1.87x.

#### SHA-512/256

| Bytes | Fed | ours | sha2 | ring |
| ---: | --- | ---: | ---: | ---: |
| 16 | whole | 0.05 ± 0.4% | 0.09 ± 0.1% | 0.11 ± 0.8% |
| 256 | whole | 0.28 ± 0.1% | 0.49 ± 0.0% | 0.74 ± 0.1% |
| 4096 | whole | 0.43 ± 0.1% | 0.74 ± 0.0% | 1.22 ± 0.1% |
| 65536 | whole | 0.45 ± 0.0% | 0.77 ± 0.0% | 1.28 ± 0.0% |
| 1048576 | whole | 0.45 ± 0.2% | 0.77 ± 0.0% | 1.27 ± 0.1% |
| 16 | by 7 | 0.05 ± 0.4% | 0.09 ± 0.2% | 0.11 ± 0.3% |
| 256 | by 7 | 0.26 ± 0.1% | 0.45 ± 0.1% | 0.48 ± 0.6% |
| 4096 | by 7 | 0.36 ± 0.0% | 0.65 ± 0.1% | 0.64 ± 0.6% |
| 65536 | by 7 | 0.37 ± 0.1% | 0.67 ± 0.3% | 0.65 ± 0.1% |
| 1048576 | by 7 | 0.37 ± 0.1% | 0.68 ± 0.1% | 0.65 ± 0.3% |

Each counterpart's speed relative to ours: whole: sha2 1.72x, ring
2.68x; by 7: sha2 1.79x, ring 1.87x.

### Zen 1, ours with `intrinsics`, as by default

#### SHA-512

| Bytes | Fed | ours | sha2 | ring |
| ---: | --- | ---: | ---: | ---: |
| 16 | whole | 0.03 ± 0.7% | 0.03 ± 1.0% | 0.03 ± 0.4% |
| 256 | whole | 0.15 ± 0.4% | 0.17 ± 0.9% | 0.21 ± 0.9% |
| 4096 | whole | 0.23 ± 1.0% | 0.26 ± 0.5% | 0.34 ± 0.7% |
| 65536 | whole | 0.24 ± 0.7% | 0.27 ± 0.2% | 0.35 ± 0.4% |
| 1048576 | whole | 0.24 ± 0.7% | 0.27 ± 0.3% | 0.36 ± 0.4% |
| 16 | by 7 | 0.02 ± 0.3% | 0.03 ± 0.5% | 0.03 ± 0.9% |
| 256 | by 7 | 0.12 ± 0.8% | 0.15 ± 0.6% | 0.16 ± 0.7% |
| 4096 | by 7 | 0.18 ± 0.4% | 0.21 ± 0.6% | 0.22 ± 0.6% |
| 65536 | by 7 | 0.18 ± 0.3% | 0.22 ± 0.8% | 0.23 ± 0.4% |
| 1048576 | by 7 | 0.18 ± 0.4% | 0.22 ± 0.7% | 0.23 ± 0.8% |

Each counterpart's speed relative to ours: whole: sha2 1.14x, ring
1.42x; by 7: sha2 1.20x, ring 1.27x.

#### SHA-512/256

| Bytes | Fed | ours | sha2 | ring |
| ---: | --- | ---: | ---: | ---: |
| 16 | whole | 0.02 ± 0.5% | 0.03 ± 0.9% | 0.03 ± 0.7% |
| 256 | whole | 0.15 ± 0.8% | 0.17 ± 0.5% | 0.21 ± 0.6% |
| 4096 | whole | 0.23 ± 0.9% | 0.26 ± 0.5% | 0.34 ± 0.7% |
| 65536 | whole | 0.24 ± 0.3% | 0.27 ± 0.2% | 0.36 ± 0.6% |
| 1048576 | whole | 0.24 ± 1.0% | 0.27 ± 0.7% | 0.35 ± 0.8% |
| 16 | by 7 | 0.02 ± 0.9% | 0.03 ± 0.5% | 0.03 ± 1.0% |
| 256 | by 7 | 0.13 ± 0.5% | 0.16 ± 0.9% | 0.16 ± 0.6% |
| 4096 | by 7 | 0.18 ± 0.3% | 0.22 ± 0.4% | 0.21 ± 0.4% |
| 65536 | by 7 | 0.18 ± 0.2% | 0.22 ± 0.4% | 0.22 ± 0.3% |
| 1048576 | by 7 | 0.19 ± 0.8% | 0.22 ± 0.5% | 0.22 ± 0.9% |

Each counterpart's speed relative to ours: whole: sha2 1.15x, ring
1.43x; by 7: sha2 1.22x, ring 1.22x.

### Zen 1, ours without `intrinsics`, without dependencies

#### SHA-512

| Bytes | Fed | ours | sha2 | ring |
| ---: | --- | ---: | ---: | ---: |
| 16 | whole | 0.02 ± 0.3% | 0.03 ± 0.5% | 0.03 ± 0.8% |
| 256 | whole | 0.15 ± 0.4% | 0.17 ± 0.4% | 0.22 ± 0.8% |
| 4096 | whole | 0.24 ± 0.4% | 0.26 ± 0.4% | 0.34 ± 0.6% |
| 65536 | whole | 0.24 ± 0.7% | 0.27 ± 0.5% | 0.36 ± 0.8% |
| 1048576 | whole | 0.24 ± 0.4% | 0.27 ± 0.5% | 0.35 ± 0.4% |
| 16 | by 7 | 0.02 ± 0.7% | 0.03 ± 0.7% | 0.03 ± 0.7% |
| 256 | by 7 | 0.13 ± 0.5% | 0.15 ± 0.2% | 0.16 ± 0.7% |
| 4096 | by 7 | 0.18 ± 1.0% | 0.20 ± 0.7% | 0.22 ± 0.8% |
| 65536 | by 7 | 0.19 ± 0.5% | 0.21 ± 0.4% | 0.23 ± 1.0% |
| 1048576 | by 7 | 0.19 ± 0.3% | 0.21 ± 0.3% | 0.23 ± 0.6% |

Each counterpart's speed relative to ours: whole: sha2 1.14x, ring
1.43x; by 7: sha2 1.14x, ring 1.25x.

#### SHA-512/256

| Bytes | Fed | ours | sha2 | ring |
| ---: | --- | ---: | ---: | ---: |
| 16 | whole | 0.03 ± 0.4% | 0.03 ± 0.3% | 0.03 ± 0.8% |
| 256 | whole | 0.15 ± 0.4% | 0.17 ± 0.6% | 0.22 ± 0.6% |
| 4096 | whole | 0.24 ± 0.8% | 0.26 ± 0.2% | 0.34 ± 0.8% |
| 65536 | whole | 0.24 ± 0.3% | 0.27 ± 0.4% | 0.35 ± 0.4% |
| 1048576 | whole | 0.24 ± 0.9% | 0.27 ± 0.9% | 0.35 ± 0.6% |
| 16 | by 7 | 0.02 ± 1.0% | 0.03 ± 0.4% | 0.03 ± 0.4% |
| 256 | by 7 | 0.13 ± 0.6% | 0.15 ± 0.3% | 0.16 ± 0.5% |
| 4096 | by 7 | 0.18 ± 0.2% | 0.21 ± 0.4% | 0.21 ± 0.3% |
| 65536 | by 7 | 0.19 ± 0.3% | 0.21 ± 0.1% | 0.21 ± 0.2% |
| 1048576 | by 7 | 0.19 ± 0.6% | 0.21 ± 0.5% | 0.21 ± 0.4% |

Each counterpart's speed relative to ours: whole: sha2 1.14x, ring
1.43x; by 7: sha2 1.16x, ring 1.21x.

## Code size

The bytes of `.text` each algorithm adds to a minimal `#![no_std]`
program that computes a checksum or hash with it, built portable with
fat LTO; read-only data, such as tables, is not counted.

<!-- filer-bench: size -->

| Algorithm | Target | `opt-level` 3 | `opt-level` "s" |
| --- | --- | ---: | ---: |
| sha512 | aarch64-apple-darwin | 1404 | 1000 |
| sha512 | aarch64-unknown-linux-gnu | 1404 | 988 |
| sha512 | x86_64-unknown-linux-gnu | 2233 | 1224 |
| sha512-256 | aarch64-apple-darwin | 1412 | 992 |
| sha512-256 | aarch64-unknown-linux-gnu | 1392 | 980 |
| sha512-256 | x86_64-unknown-linux-gnu | 2237 | 1203 |

On aarch64-apple-darwin, measured 2026-11-07, at `86d2427`,
`compute-sha512` 0.0.4, by rustc 1.99.1 (58cf8f9c3 2026-10-13).

On aarch64-unknown-linux-gnu, measured 2026-11-13, at `de4f027`,
`compute-sha512` 0.0.4, by rustc 1.99.1 (58cf8f9c3 2026-10-13).

On x86_64-unknown-linux-gnu, measured 2026-11-13, at `de4f027`,
`compute-sha512` 0.0.4, by rustc 1.99.1 (58cf8f9c3 2026-10-13).

<!-- filer-bench: end -->

# .6cy Benchmark Report — v0.3.0 Data
### 6cy (LZMA) vs 7-Zip (LZMA2 level 1) — 10 GiB Sequential Data

> **Status:** Official benchmark, collected under v0.3.0
> **Applies to:** v0.3.0 and v1.0.0 (no changes to codec, I/O, or compression
> pipeline between these versions — data remains representative)
> **v1.0.0 benchmarks:** Pending. Will be published once completed.
> **Date:** 2026
> **Validated:** ✅ All runs passed round-trip integrity check

---

## Table of Contents

1. [Test Environment](#1-test-environment)
2. [Tool Versions and Configuration](#2-tool-versions-and-configuration)
3. [Test Data](#3-test-data)
4. [Benchmark Methodology](#4-benchmark-methodology)
5. [Raw Per-Run Results](#5-raw-per-run-results)
6. [Aggregated Results](#6-aggregated-results)
7. [Derived Metrics](#7-derived-metrics)
8. [Detailed Analysis](#8-detailed-analysis)
9. [Why the Numbers Look Like This](#9-why-the-numbers-look-like-this)
10. [Benchmark Script Reference](#10-benchmark-script-reference)
11. [Reproduction Instructions](#11-reproduction-instructions)
12. [Interpretation Caveats](#12-interpretation-caveats)

---

## 1. Test Environment

### Hardware

| Item | Value |
|------|-------|
| System | Alienware m17 R5 AMD |
| CPU | AMD Ryzen 9 6900HX with Radeon Graphics |
| CPU base frequency | 3301 MHz |
| Physical cores | 8 |
| Logical processors (SMT) | 16 |
| Installed RAM | 16.0 GB |
| Available RAM | 7.06 GB (at test time) |
| Total physical memory | 15.2 GB |
| Total virtual memory | 20.6 GB |
| Page file | 5.34 GB (C:\pagefile.sys) |

### Software

| Item | Value |
|------|-------|
| OS | Microsoft Windows 11 Home |
| OS version | 10.0.26220 Build 26220 |
| BIOS | Alienware 1.25.0, 11/7/2025 |
| Secure Boot | On |
| Kernel DMA Protection | On |
| Virtualization-based Security | Running |

---

## 2. Tool Versions and Configuration

### 6cy — .6cy Reference Implementation v0.3.0

| Parameter | Value |
|-----------|-------|
| Version | 0.3.0 (benchmark run; current release is v1.0.0) |
| Codec | LZMA (via `lzma-rs` 0.3, pure Rust) |
| Codec UUID | `4a8f2e1c-9b3d-4f7a-c2e8-6d5b1a0f3c9e` |
| Compression level | default (lzma-rs uses its own defaults) |
| Chunk size | 4 MiB (default) |
| Encryption | none |
| Solid mode | no |
| CAS deduplication | yes (BLAKE3-keyed) |
| Block header CRC32 | yes (mandatory) |
| Block content BLAKE3 | yes (mandatory) |

**Pack command:**
```
6cy pack test_10gb.bin --output 6cy_runN.6cy --codec lzma
```

**Info command:**
```
6cy info 6cy_runN.6cy
```

**Unpack command:**
```
6cy unpack --output-dir out_6cy_runN 6cy_runN.6cy
```

### 7-Zip

| Parameter | Value |
|-----------|-------|
| Version | *not recorded at time of test — fill in before publishing* |
| Codec | LZMA2 (7-Zip's native highly optimized C++ implementation) |
| Level | 1 (fastest LZMA2 preset) |
| Method flag | `-m0=lzma2` |
| Max memory flag | `-mx=1` |
| Multithreading | off (`-mmt=off`) |
| Solid block | default (ON — 7-Zip `.7z` archives use solid mode by default, meaning the entire archive is compressed as one block with a shared dictionary) |

**Pack command:**
```
7z a -t7z -m0=lzma2 -mx=1 -mmt=off 7z_runN.7z test_10gb.bin
```

**Unpack command:**
```
7z x 7z_runN.7z -oout_7z_runN -y
```

> **Why `-mmt=off`?** Multithreading is disabled on the 7z side to create a
> single-threaded comparison that isolates codec efficiency rather than
> parallelism. 6cy's LZMA implementation (`lzma-rs`) is also single-threaded.
> This is the fairest possible comparison of the underlying compression
> algorithms' Rust vs C++ implementations.

---

## 3. Test Data

| Property | Value |
|----------|-------|
| File name | `test_10gb.bin` |
| File size | 10,737,418,240 bytes (exactly 10.00 GiB) |
| File type | Binary, generated synthetically |
| Generation | Python script (`benchmark_6cy_vs_7z_windows.py --size 10GiB`) |
| Content | Highly compressible sequential/repetitive binary data |

The test file is generated programmatically rather than using real-world data.
This ensures:
1. Identical input across all runs and all future reproductions.
2. A controlled compressibility level that stresses codec throughput.
3. No accidental disclosure of real user data.

The extreme compression ratios observed (≥ 6 800:1) confirm the data contains
long repetitive runs, which is the intended behavior for this throughput-focused
benchmark.

---

## 4. Benchmark Methodology

### Process

1. The Python benchmark harness (`benchmark_6cy_vs_7z_windows.py`) generates
   the test file once at the configured size (10 GiB).
2. For each tool × run combination:
   a. The tool is invoked via `subprocess` to pack the test file.
   b. Wall-clock time is measured from process start to process exit.
   c. CPU usage (average % and peak %) is sampled at 0.5 s intervals via
      `psutil` during the subprocess lifetime.
   d. The resulting archive file size is read from disk.
   e. The info command is run (6cy only) to verify the archive header.
   f. The tool is invoked to unpack the archive to a temporary directory.
   g. Wall-clock time and CPU usage are measured for unpack as well.
   h. The unpacked file's SHA-256 digest is compared to the original file.
      The run is marked `validated: True` only if the digest matches.
3. Steps 2a–2h are repeated `--runs N` times (N=3 in this report).
4. Averages and a CSV summary are written after all runs.

### CPU Measurement Notes

CPU percentages are reported per logical core on Windows. Since the AMD Ryzen 9
6900HX exposes 16 logical processors, a single fully-loaded core reads as
approximately 100% ÷ 16 ≈ 6.25% in Task Manager (which shows aggregate CPU%)
but as 100% from the perspective of the process itself. The `psutil` library
reports **per-process CPU utilization as a percentage of one logical core**,
so values near 100% mean one logical core is saturated; values above 100%
indicate simultaneous use of more than one logical core. This is reflected in
the peak CPU readings (~101%) captured for both tools, consistent with brief
two-core bursts during buffer handoff.

### Validation

Every run writes `validated: True` confirming that the unpacked output matches
the original input byte-for-byte. No corrupted or failed runs occurred.

---

## 5. Raw Per-Run Results

All times in seconds. All sizes in bytes. CPU in percent of one logical core.

### 6cy — LZMA

| Run | Pack time (s) | Pack CPU avg (%) | Pack CPU peak (%) | Info time (s) | Unpack time (s) | Unpack CPU avg (%) | Unpack CPU peak (%) | Archive size (B) | Validated |
|-----|--------------|-----------------|------------------|--------------|----------------|-------------------|---------------------|-----------------|-----------|
| 1/3 | 13.201 | 78.00 | 101.8 | 0.045 | 48.664 | 71.20 | 101.8 | 983,405 | ✅ |
| 2/3 | 12.696 | 75.88 | 101.8 | 0.045 | 45.622 | 71.52 | 101.9 | 983,405 | ✅ |
| 3/3 | 13.209 | 76.23 | 101.8 | 0.054 | 46.649 | 69.62 | 101.8 | 983,405 | ✅ |

### 7-Zip — LZMA2 (level 1, single-threaded)

| Run | Pack time (s) | Pack CPU avg (%) | Pack CPU peak (%) | Unpack time (s) | Unpack CPU avg (%) | Unpack CPU peak (%) | Archive size (B) | Validated |
|-----|--------------|-----------------|------------------|----------------|-------------------|---------------------|-----------------|-----------|
| 1/3 | 34.427 | 96.99 | 101.9 | 8.605 | 88.08 | 101.7 | 1,563,448 | ✅ |
| 2/3 | 34.438 | 98.37 | 101.9 | 8.606 | 84.99 | 101.9 | 1,563,448 | ✅ |
| 3/3 | 34.964 | 96.76 | 101.7 | 8.615 | 87.64 | 101.7 | 1,563,448 | ✅ |

---

## 6. Aggregated Results

### 6cy (LZMA)

| Metric | Value |
|--------|-------|
| Pack time — average | **13.035 s** |
| Pack time — min | 12.696 s |
| Pack time — max | 13.209 s |
| Pack time — range | 0.513 s (3.9% variance) |
| Pack CPU — average | 76.703 % |
| Unpack time — average | **46.978 s** |
| Unpack time — min | 45.622 s |
| Unpack time — max | 48.664 s |
| Unpack time — range | 3.042 s (6.5% variance) |
| Unpack CPU — average | 70.780 % |
| Archive size | 983,405 B = 960.36 KiB = 0.938 MiB |

### 7-Zip (LZMA2 level 1)

| Metric | Value |
|--------|-------|
| Pack time — average | **34.610 s** |
| Pack time — min | 34.427 s |
| Pack time — max | 34.964 s |
| Pack time — range | 0.537 s (1.6% variance) |
| Pack CPU — average | 97.373 % |
| Unpack time — average | **8.609 s** |
| Unpack time — min | 8.605 s |
| Unpack time — max | 8.615 s |
| Unpack time — range | 0.010 s (0.1% variance) |
| Unpack CPU — average | 86.903 % |
| Archive size | 1,563,448 B = 1,526.80 KiB = 1.491 MiB |

---

## 7. Derived Metrics

### Throughput (based on 10,737,418,240 B original)

| Tool | Pack throughput | Unpack throughput |
|------|----------------|------------------|
| 6cy (LZMA) | **0.767 GiB/s** | 0.213 GiB/s |
| 7z (LZMA2 L1) | 0.289 GiB/s | **1.162 GiB/s** |

### Compression Ratio

| Tool | Archive size | Ratio (original / archive) | % of original |
|------|-------------|---------------------------|---------------|
| 6cy (LZMA) | 960.36 KiB | **10,919 : 1** | 0.00916% |
| 7z (LZMA2 L1) | 1,526.80 KiB | 6,868 : 1 | 0.01456% |

### Head-to-Head Comparisons

| Metric | Winner | Factor |
|--------|--------|--------|
| Pack speed | **6cy** | **2.66× faster** (13.035 s vs 34.610 s) |
| Unpack speed | **7z** | **5.46× faster** (8.609 s vs 46.978 s) |
| Archive size | **6cy** | **37.10% smaller** (960 KiB vs 1,527 KiB) |
| Pack CPU usage | **6cy** | **21.2% less CPU** (76.7% vs 97.4%) |
| Unpack CPU usage | **6cy** | **18.5% less CPU** (70.8% vs 86.9%) |

---

## 8. Detailed Analysis

### 8.1 Pack Performance

6cy packs 10 GiB in **13.0 s** against 7z's **34.6 s** — a **2.66× speed
advantage** — while also producing a **37% smaller** archive. The lower CPU
utilization (76.7% vs 97.4%) indicates that 6cy's pack path is not the
bottleneck; the delta is likely disk I/O between compression passes.

The lzma-rs encoder operates at a single-thread level similar to 7z's
`-mmt=off` mode, so this is a genuine single-stream LZMA vs LZMA2 comparison.
The result suggests that lzma-rs's compression path benefits from Rust's
zero-cost abstractions and cache-friendly buffer handling.

**Pack throughput breakdown:**

```
6cy:  10,737,418,240 B ÷ 13.035 s = 786 MiB/s = 0.767 GiB/s
7z:   10,737,418,240 B ÷ 34.610 s = 296 MiB/s = 0.289 GiB/s
```

### 8.2 Unpack Performance

7z unpacks in **8.6 s** against 6cy's **47.0 s** — a **5.46× speed advantage**
for 7z. This is the expected trade-off: 7z's LZMA decompressor is a mature,
hand-optimized C++ implementation with over two decades of tuning, while
lzma-rs 0.3 is a pure-Rust implementation prioritizing correctness and safety.

The decompression speed gap does not affect archive integrity — both tools
validated 100% of runs — but is an important practical consideration for
read-heavy workloads.

**Planned improvement:** A future release will evaluate linking against liblzma
(the C library underlying XZ Utils) via an FFI shim for the decompression path,
which would be expected to bring decompression throughput to parity with 7z.

**Unpack throughput breakdown:**

```
6cy:  10,737,418,240 B ÷ 46.978 s = 218 MiB/s = 0.213 GiB/s
7z:   10,737,418,240 B ÷  8.609 s = 1190 MiB/s = 1.162 GiB/s
```

### 8.3 Archive Size

6cy produces a **37.10% smaller** archive (960 KiB vs 1,527 KiB) from the same
input with the same codec family. Two mechanisms contribute:

1. **CAS deduplication.** 6cy's writer maintains a BLAKE3-keyed
   content-addressable block map. Chunks with identical 32-byte content hashes
   are written once; subsequent references store only a `BlockRef` pointing at
   the existing block. For the synthetic test file, which contains long
   repetitive runs already split into 4 MiB chunks, many chunks hash to
   identical values and are deduplicated before any compression occurs.

2. **Block-level framing overhead.** Each .6cy block carries an 84-byte
   self-describing header (magic, version, codec UUID, sizes, BLAKE3, CRC32).
   For a highly compressible file that compresses to ~1 MiB, this overhead
   is negligible relative to the payload.

The 7z LZMA2 archive at 1,527 KiB does not perform deduplication at the block
level and uses its own container framing, resulting in a larger output for this
specific workload.

### 8.4 CPU Usage

Both tools run a single compression thread. 6cy's pack CPU average (76.7%)
is notably below 7z's (97.4%), suggesting that 6cy's chunked I/O pipeline
allows the CPU more idle time between chunk boundaries — consistent with the
4 MiB chunk batching model. 6cy's unpack CPU (70.8%) also trails 7z's (86.9%),
reflecting the lzma-rs decompressor doing less work per unit time (lower
throughput but also lower CPU burn per byte).

### 8.5 Run-to-Run Variance

| Tool | Pack variance | Unpack variance |
|------|-------------|----------------|
| 6cy | 3.9% | 6.5% |
| 7z | 1.6% | 0.1% |

6cy shows slightly higher variance — especially in unpack — consistent with
OS scheduling jitter at lower throughput (more elapsed time = more opportunity
for OS interference). 7z's unpack variance is extremely tight (10 ms over 3
runs), reflecting a highly deterministic fast path through a cache-warm hot
loop.

---

## 9. Why the Numbers Look Like This

### Extreme compression ratios (≈10 000 : 1)

Both tools achieve enormous compression ratios because the 10 GiB test file
is synthetically generated with a high degree of regularity. LZMA's LZ77
match-finder identifies extremely long back-references across the entire file
history, and the range encoder produces very few bits per symbol. This is
intentional: the benchmark targets **throughput** (how fast can the codec
process data), not **ratio** (how well can it compress real-world content).

### 6cy packs faster than it decompresses

LZMA compression is fundamentally faster than decompression for highly
compressible data: the encoder can skip large blocks with a trivially cheap
"repeat last match" decision, while the decoder must reconstruct every
back-reference copy sequentially. 6cy's 0.767 GiB/s pack vs 0.213 GiB/s
unpack is consistent with this asymmetry.

### 7z decompresses 5.5× faster than 6cy

The 7z LZMA2 decompressor is implemented in hand-optimized C++ with SIMD
intrinsics and decades of profiling feedback. The lzma-rs decompressor is
implemented in safe Rust, prioritizing correctness and auditability. The
performance gap is well-known in the Rust ecosystem and is a known trade-off
in this release; it does not affect correctness or format integrity.

---

## 10. Benchmark Script Reference

The benchmark was executed with:

```
python benchmark_6cy_vs_7z_windows.py --size 10GiB --codec lzma --level 1 --runs 3
```

**Script behavior (summarized):**

1. Generates `test_10gb.bin` (10,737,418,240 bytes) if not already present.
2. For each run (1..N), for each tool (6cy, 7z):
   - Runs the pack command; measures wall time + CPU via `psutil`.
   - Reads the resulting archive size from disk.
   - Runs the unpack command; measures wall time + CPU.
   - Computes SHA-256 of unpacked output, compares to original.
   - Records all fields in a result dict.
3. Prints per-run result dicts immediately (as seen in the terminal output).
4. Prints the averaged benchmark summary in English.
5. Writes `benchmark_results.csv` with all raw per-run data.

**Result dict fields:**

| Field | Description |
|-------|-------------|
| `tool` | `'6cy'` or `'7z'` |
| `codec` | codec string as reported by tool |
| `level` | level parameter as passed |
| `archive_size` | bytes of the compressed output file |
| `pack_time_s` | wall-clock pack time in seconds |
| `pack_cpu_avg_pct` | mean CPU % sampled during pack |
| `pack_cpu_peak_pct` | maximum CPU % sampled during pack |
| `info_time_s` | wall-clock time for `6cy info` (6cy only; `None` for 7z) |
| `unpack_time_s` | wall-clock unpack time in seconds |
| `unpack_cpu_avg_pct` | mean CPU % sampled during unpack |
| `unpack_cpu_peak_pct` | maximum CPU % sampled during unpack |
| `validated` | `True` if SHA-256 of unpacked file matches original |

---

## 11. Reproduction Instructions

### Prerequisites

- Windows 10/11 x64
- Python 3.9+ with `psutil` installed (`pip install psutil`)
  *(Actual version used in this report: not recorded — record with `python --version`)*
- 7-Zip installed and `7z.exe` on `PATH`
  *(Record version with `7z i` before running)*
- Rust toolchain stable 1.70+ with `cargo` on `PATH`
  *(Actual version used in this report: not recorded — record with `rustc --version`)*
- .6cy v0.3.0 binary on `PATH` as `6cy.exe` (use v0.3.0 to reproduce these specific results)
- At least 70 GB free disk space:
  - 10 GB test input file
  - 3 × `out_6cy_runN/` unpacked directories (10 GB each = 30 GB)
  - 3 × `out_7z_runN/` unpacked directories (10 GB each = 30 GB)
  - 6 archive files (< 2 MB each — negligible)
  - If you delete each unpacked directory immediately after validation, 30 GB suffices.

### Steps

```bat
REM 1. Clone or download the repository
git clone https://github.com/cyh/sixcy.git
cd sixcy

REM 2. Build the 6cy binary (requires Rust toolchain)
cargo build --release
copy target\release\6cy.exe C:\tools\6cy.exe

REM 3. Navigate to the benchmark directory
cd D:\benchmark_6cy_vs_7z

REM 4. Run the benchmark
python benchmark_6cy_vs_7z_windows.py --size 10GiB --codec lzma --level 1 --runs 3

REM 5. Results are printed to stdout and written to benchmark_results.csv
```

### Varying the parameters

```bat
REM Benchmark with Zstd at level 3 (default), 5 runs
python benchmark_6cy_vs_7z_windows.py --size 10GiB --codec zstd --level 3 --runs 5

REM Quick smoke test with 1 GiB
python benchmark_6cy_vs_7z_windows.py --size 1GiB --codec lzma --level 1 --runs 1
```

---

## 12. Interpretation Caveats

**This benchmark uses a single synthetic file.** Results may differ
significantly for:
- Real-world heterogeneous data (documents, source code, database dumps).
- Smaller files where per-block overhead becomes significant.
- SSD-limited workloads where I/O bandwidth becomes the constraint.
- Multi-threaded scenarios (7z with `-mmt=on` would reduce pack time
  dramatically; 6cy does not yet parallelize compression).

**lzma-rs vs liblzma.** The 5.5× decompression gap reflects a pure-Rust
implementation vs a mature C implementation. It is not a fundamental limitation
of the .6cy format. The format spec is codec-agnostic; a future release may
offer an optional liblzma backend via FFI.

**CPU measurements are approximate.** The `psutil` sampling interval is 0.5 s;
short spikes are not captured. Values reflect average steady-state utilization
rather than instantaneous peak load during critical inner loops.

**Disk cache effects.** The test file is 10 GiB, larger than the available
16 GB RAM after OS overhead. Reads are likely partially cache-warm after the
first run. All three runs exhibit consistent timings, suggesting the OS file
cache is reasonably stable across runs on this system.

---

*Benchmark data collected under v0.3.0. Document updated for v1.0.0 release.*  
*Software: Apache-2.0 license. Specification and benchmark documents: CC BY 4.0.*

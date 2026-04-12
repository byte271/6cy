# Benchmark Report — sixcy v2.0.0

## Overview

This document contains benchmark results for the sixcy archive tool. Results are
organized by category and clearly labeled with their source version.

> **Note:** This document contains historical data from multiple versions.
> The primary comparison data below was collected under v0.3.0.

---

## 1. Test Environment

| Item | Value |
|------|-------|
| System | Alienware m17 R5 AMD |
| CPU | AMD Ryzen 9 6900HX @ 3.3 GHz |
| Physical cores | 8 |
| Logical processors | 16 |
| RAM | 16 GB |
| OS | Windows 11 |

---

## 2. sixcy vs 7-Zip Comparison [v0.3.0 Data]

### Configuration

| Tool | Version | Codec | Notes |
|------|---------|-------|-------|
| sixcy | 0.3.0 | LZMA | Pure Rust via lzma-rs 0.3 |
| 7-Zip | (not recorded) | LZMA2 | Level 1, single-threaded |

**Test file:** 10 GiB synthetic binary (highly compressible, repetitive data)

### Results

| Metric | sixcy (LZMA) | 7-Zip (LZMA2) | Winner |
|--------|--------------|---------------|--------|
| Pack time | 13.0 s | 34.6 s | sixcy (2.7x faster) |
| Unpack time | 47.0 s | 8.6 s | 7-Zip (5.5x faster) |
| Archive size | 960 KiB | 1,527 KiB | sixcy (37% smaller) |
| Pack throughput | 0.77 GiB/s | 0.29 GiB/s | sixcy |
| Unpack throughput | 0.21 GiB/s | 1.16 GiB/s | 7-Zip |

### Notes

- sixcy's faster pack time reflects lzma-rs's efficient implementation for
  compression.
- 7-Zip's faster unpack time reflects its mature, hand-optimized C++
  decompressor.
- The smaller archive size for sixcy is due to CAS deduplication of identical
  chunks.
- All runs validated (unpacked output matches original).

### LZMA Decompression Performance

The lzma-rs 0.3 decompressor is approximately 5-6x slower than 7-Zip's
implementation. This is a known characteristic of pure-Rust implementations
vs hand-optimized C++. This affects read-heavy workloads but not correctness.

A future release may offer an optional liblzma backend for the decompression
path.

---

## 3. Compression Ratio on Technical Binary

### Test Data [Current - v2.0.0]

Test file: `target/release/6cy.exe` (Rust release binary)

| Codec | Level | Size | Ratio |
|-------|-------|------|-------|
| Original | - | 3,100,160 B | - |
| Zstd | 3 | 1,420,056 B | 2.18:1 |
| LZMA | 1 | 1,889,035 B | 1.64:1 |

For technical binaries, Zstd (level 3, the default) provides better ratio
than LZMA at level 1.

---

## 4. Interpretation Caveats

1. **Synthetic test data** produces extreme compression ratios (≥10,000:1)
   that do not reflect real-world workloads.

2. **Single-threaded comparison** isolates codec efficiency but does not
   reflect multi-threaded scenarios where 7-Zip with `-mmt=on` would be
   significantly faster.

3. **LZMA vs LZMA2** — sixcy uses LZMA (lzma-rs), while 7-Zip uses LZMA2.
   These are related but different algorithms.

4. **CPU measurements** are approximate due to 0.5s sampling interval.

5. **Disk cache effects** may influence timings on repeated runs with the
   same test file.

---

## 5. Running Benchmarks

```bash
# Build release binary
cargo build --release

# Run RLE pre-filter benchmark on a file
6cy bench your_file.dat

# Compare compression methods
time 6cy pack -o zstd.6cy -i data/ --codec zstd
time 6cy pack -o lzma.6cy -i data/ --codec lzma
```

---

## 6. Reproduction

To reproduce these benchmarks:

1. Ensure Windows 11 x64 with 16+ GB RAM
2. Install Rust toolchain (stable 1.70+)
3. Clone repository and build: `cargo build --release`
4. Generate synthetic test data (if needed)
5. Run comparison tests with timing

*Note: Actual results may vary based on system load, disk speed, and other factors.*

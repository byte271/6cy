<div align="center">

# .6cy Container Format

**v1.0.0** · Reference implementation in Rust

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Spec License: CC BY 4.0](https://img.shields.io/badge/spec-CC%20BY%204.0-green.svg)](LICENSE-SPEC)

</div>

---

**.6cy** is a binary archive format built around four hard guarantees:

- **Every block is self-describing.** Magic, version, codec UUID, sizes, and two
  independent checksums live in each 84-byte block header. A reader can parse
  any single block in isolation.
- **Checksums are mandatory.** A CRC32 covers the header; a BLAKE3 covers the
  content. Neither can be disabled. Corruption is caught before any allocation.
- **Codec identity is frozen.** Each codec is identified by a permanent 128-bit
  UUID stored verbatim on disk. Short numeric IDs are an in-process optimization
  only and are never written to files.
- **No runtime negotiation.** The superblock declares all required codec UUIDs
  upfront. A decoder either has every one or fails immediately — no fallback,
  no partial decode.

---

## Benchmark — 6cy (LZMA) vs 7-Zip (LZMA2 level 1)

> **These numbers are from v0.3.0.** v1.0.0 benchmarks have not yet been run.
> v1.0.0 contains no changes to the core codec, I/O, or compression pipeline,
> so these figures remain representative. Updated numbers will be published once
> v1.0.0 benchmarks are completed.

Tested on **AMD Ryzen 9 6900HX** (8C/16T, 3301 MHz), 16 GB RAM,
Windows 11 Home, 10 GiB synthetic binary file, 3 runs each.
Full methodology in [`BENCHMARK.md`](BENCHMARK.md).

| Metric | **6cy LZMA** | 7z LZMA2 L1 |
|--------|-------------|-------------|
| Pack time (avg) | **13.0 s** | 34.6 s |
| Unpack time (avg) | 47.0 s | **8.6 s** |
| Archive size | **960 KiB** | 1 527 KiB |
| Pack throughput | **0.767 GiB/s** | 0.289 GiB/s |
| Unpack throughput | 0.213 GiB/s | **1.162 GiB/s** |
| Pack CPU (avg) | **76.7 %** | 97.4 % |
| Compression ratio | **10 919 : 1** | 6 868 : 1 |

**6cy packs 2.66× faster** and produces a **37% smaller** archive.  
**7z decompresses 5.46× faster** — 7z's LZMA2 decompressor is a mature
hand-optimized C++ implementation; `lzma-rs` is pure Rust (correctness-first).
A future release will evaluate an optional liblzma FFI backend for the
decompression path.

---

## Features

### Core format

- **Content-addressable deduplication** — identical 4 MiB chunks are written
  once; subsequent references cost only an 84-byte `BlockRef`. No codec pass
  for duplicate chunks.
- **Four codecs** — Zstd (default), LZ4, Brotli, LZMA. Each identified by a
  frozen UUID; short IDs never leave the process.
- **Solid mode** — multiple files compressed together as one block for maximum
  ratio on small/similar files.
- **AES-256-GCM block encryption** — Argon2id key derivation (64 MiB, 3 passes).
  The archive UUID serves as the KDF salt so the same password yields a
  different key for every archive.
- **Chunked streaming** — files of any size are split into configurable chunks
  (default 4 MiB). Random access spans chunk boundaries correctly.
- **Reconstructible index** — the FILE INDEX is written last. If it is missing
  or corrupt, `6cy scan` rebuilds the file list by reading only block headers
  forward from byte 256, without decompressing any payload.
- **Plugin C ABI** — third-party codecs load via a frozen C ABI
  (`plugin_abi/sixcy_plugin.h`, ABI version 1). Explicit buffer contracts,
  declared thread safety, no shared allocator.

### GUI (6cy Archive Suite desktop app — v1.0.0)

The desktop application (`sixcy-app`, built with Tauri + React) ships nine
panels. All panels added or promoted to full feature status in v1.0.0 are
marked **NEW**.

| Panel | Description |
|-------|-------------|
| **Pack** | Drag-and-drop or browse files → compress into `.6cy`. Codec selector (Zstd/LZ4/Brotli/LZMA/None), per-codec level bounds enforced, chunk size, solid mode, encryption password. Keyboard shortcut ⌘/Ctrl+Enter. |
| **Unpack** | Extract archive with BLAKE3 integrity verification. Shows file count, total size, and per-file extraction log. |
| **List** | Tabular archive contents with filename filter, compression ratio per file, total original/compressed size summary. |
| **Info** | Superblock fields, required codec UUIDs, root BLAKE3 hash — all copyable to clipboard. |
| **Scan** | Index-bypass block header scan. Reconstructs file list without INDEX block. Filename filter on results. |
| **Recover** *(NEW)* | Index-bypass full recovery. Animated block-health grid (healthy / corrupt / truncated / unknown codec). Summary card showing quality rating (Full / Partial / HeaderOnly / Catastrophic), per-category block counts, health-score progress bar, and recoverable MiB salvaged. |
| **Optimize** *(NEW)* | Re-compress all blocks at a chosen Zstd level (1–19). Before/after size and savings percentage displayed as stat cards. |
| **Merge** *(NEW)* | Combine multiple `.6cy` archives into one with cross-archive CAS deduplication. Drag-to-reorder source list. Output codec selector. |
| **Bench** *(NEW)* | RLE pre-filter benchmark. Drag-drop or browse any file; measures encode time, decode time, savings percentage, and round-trip correctness. Persists run history table across benchmarks within the session. |

**Additional GUI improvements in v1.0.0:**

- **Dark mode** — full dark theme, persists across sessions.
- **Recent archives** — last 10 used paths surfaced as a collapsible
  quick-picker in every panel.
- **Keyboard navigation** — Alt+1–9 switches panels; ⌘/Ctrl+Enter triggers
  Pack.
- **Copy-to-clipboard** — one-click copy on UUIDs, archive paths, and BLAKE3
  root hashes in the Info panel.
- **Log export** — terminal output can be saved to a `.txt` file.
- **Codec tooltips** — hovering a codec badge shows a description of its
  strengths.
- **Input validation** — level and chunk-size fields are clamped to valid
  ranges per codec; invalid values are rejected before any backend call.
- **Binary prefix fix** — `fmtBytes` now uses correct binary thresholds
  (2³⁰ for GiB, 2²⁰ for MiB) instead of the SI values (10⁹, 10⁶) that were
  previously mislabelled as binary units.

---

## Project Layout

```
Sixcy_CAS/
├── Cargo.toml                   # version 1.0.0, Apache-2.0
├── LICENSE                      # Apache-2.0 (code)
├── LICENSE-SPEC                 # CC BY 4.0 (spec.md)
├── README.md                    # this file
├── BENCHMARK.md                 # detailed benchmark report (v0.3.0 data)
├── CHANGELOG.md                 # version history
├── CONTRIBUTING.md              # how to contribute
├── SECURITY.md                  # threat model and disclosure policy
├── spec.md                      # binary format specification (CC BY 4.0)
├── plugin_abi/
│   └── sixcy_plugin.h           # frozen C ABI for codec plugins
└── src/
    ├── main.rs                  # CLI (6cy binary)
    ├── lib.rs                   # crate root + re-exports
    ├── archive.rs               # high-level Archive API
    ├── block.rs                 # block header encode/decode
    ├── superblock.rs            # superblock (offset 0, 256 bytes)
    ├── plugin.rs                # Rust wrapper for C plugin ABI
    ├── perf.rs                  # parallel chunk compression, write buffer, RLE pre-filter
    ├── codec/mod.rs             # frozen UUID registry + built-in codecs
    ├── crypto/mod.rs            # AES-256-GCM + Argon2id
    ├── index/mod.rs             # FileIndex, BlockRef
    ├── io_stream/mod.rs         # SixCyWriter, SixCyReader, scan_blocks
    └── recovery/
        ├── mod.rs               # RecoveryMap + re-exports
        └── scanner.rs           # extract_recoverable, BlockHealth, RecoveryReport
```

---

## Getting Started

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) stable (1.70+)
- No C toolchain required — all dependencies are pure Rust (except `zstd`,
  which wraps the zstd C library)

### Build

```bash
git clone https://github.com/cyh/sixcy.git
cd sixcy
cargo build --release
# binary: target/release/6cy  (Linux/macOS)
# binary: target\release\6cy.exe  (Windows)
```

---

## CLI Reference

### `pack` — create an archive

```bash
# Single file, Zstd (default)
6cy pack -o archive.6cy -i file.bin

# Multiple files, LZMA codec
6cy pack -o archive.6cy -i a.bin -i b.bin -i c.bin --codec lzma

# Solid block (all inputs compressed together)
6cy pack -o archive.6cy -i *.txt --codec zstd --solid

# Encrypted (AES-256-GCM, Argon2id key derivation)
6cy pack -o archive.6cy -i secret.bin --password "my passphrase"

# Custom chunk size (default 4096 KiB = 4 MiB)
6cy pack -o archive.6cy -i huge.bin --chunk-size 8192

# Full options
6cy pack --output archive.6cy \
         --input file1.bin --input file2.bin \
         --codec lzma \
         --level 3 \
         --chunk-size 4096 \
         --solid \
         --password "secret"
```

**Available codecs:** `zstd` (default) · `lz4` · `brotli` · `lzma` · `none`

### `unpack` — extract an archive

```bash
# Extract to current directory
6cy unpack archive.6cy

# Extract to specific directory
6cy unpack archive.6cy -C output/

# Extract encrypted archive
6cy unpack archive.6cy -C output/ --password "my passphrase"
```

### `list` — list contents

```bash
6cy list archive.6cy
# Name                       Size    Compressed  Chunks  First block hash
# readme.txt                 4096          1024       1  a1b2c3...
# data.bin              10485760       2097152       3  deadbe...
```

### `info` — archive metadata

```bash
6cy info archive.6cy
# ── .6cy Archive ─────────────────────────────────────────
#   Path           archive.6cy
#   Format version 3
#   UUID           550e8400-e29b-41d4-a716-446655440000
#   Encrypted      false
#   Index offset   41943296 B
#   Index size     2048 B
#   Files          5
#   Root hash      a3f2...
#   Required codecs (2):
#     4a8f2e1c-9b3d-4f7a-c2e8-6d5b1a0f3c9e (lzma)
#     b28a9d4f-5e3c-4a1b-8f2e-7c6d9b0e1a2f (zstd)
```

### `scan` — reconstruct index from block headers

```bash
# Recover file list without the INDEX block (partial/truncated archives)
6cy scan archive.6cy
# Scan recovered 3 file(s) from block headers:
#   id=00000000  chunks=3  size=12582912  name=file_00000000
#   id=00000001  chunks=1  size=4096      name=file_00000001
```

### `recover` — index-bypass full recovery *(new in v1.0.0)*

Performs a full forward scan of all block headers and reassembles every
readable file into a new archive, even when the INDEX block, RecoveryMap, and
all directory structure are absent or corrupt.

```bash
6cy recover damaged.6cy -o recovered.6cy

# Encrypted source
6cy recover damaged.6cy -o recovered.6cy --password "my passphrase"
```

Output reports:

- Total blocks scanned
- Healthy / corrupt / truncated / unknown-codec block counts
- Recovery quality: `Full` · `Partial` · `HeaderOnly` · `Catastrophic`
- Recoverable MiB salvaged
- Files written to the output archive

### `optimize` — re-compress at maximum ratio *(promoted to full feature in v1.0.0)*

```bash
6cy optimize archive.6cy -o archive_max.6cy          # Zstd level 19 (default)
6cy optimize archive.6cy -o archive_max.6cy --level 9
```

### `merge` — combine archives *(new in v1.0.0)*

Merges two or more `.6cy` archives into a single output archive. Files from
each source are namespaced under the source archive stem to avoid collisions.
Cross-archive CAS deduplication is applied — identical chunks are written only
once regardless of which source archive they came from.

```bash
6cy merge part1.6cy part2.6cy part3.6cy -o merged.6cy
6cy merge part1.6cy part2.6cy -o merged.6cy --codec zstd
```

### `bench` — RLE pre-filter benchmark *(new in v1.0.0)*

Measures how much a run-length encoding pre-filter reduces a file before the
entropy coder stage. Reports encode time, decode time, savings percentage, and
round-trip correctness.

```bash
6cy bench input.bin
# ── RLE pre-filter benchmark ─────────────────────────────
#   Input size:   10485760 B
#   Encoded size: 3379200 B  (32.2% of original)
#   Encode time:  4 ms
#   Decode time:  2 ms
#   Round-trip:   ✓ correct
```

---

## Library API

Add to `Cargo.toml`:

```toml
[dependencies]
sixcy = { path = "." }   # or version once published to crates.io
```

### Create an archive

```rust
use sixcy::archive::{Archive, PackOptions};
use sixcy::codec::CodecId;

// Default options: Zstd level 3, 4 MiB chunks, no encryption
let mut ar = Archive::create("output.6cy", PackOptions::default())?;
ar.add_file("readme.txt", b"Hello, world!")?;
ar.add_file_with_codec("data.bin", &data, CodecId::Lzma)?;
ar.finalize()?;  // MUST be called — writes INDEX block and patches superblock
```

### Solid blocks

```rust
ar.begin_solid(CodecId::Zstd)?;
ar.add_file("a.txt", &a)?;
ar.add_file("b.txt", &b)?;
ar.add_file("c.txt", &c)?;
ar.end_solid()?;   // flushes the combined block
ar.finalize()?;
```

### Encryption

```rust
let opts = PackOptions {
    password: Some("my passphrase".into()),
    ..PackOptions::default()
};
let mut ar = Archive::create("secret.6cy", opts)?;
ar.add_file("private.bin", &data)?;
ar.finalize()?;

// Open
let mut ar = Archive::open_encrypted("secret.6cy", "my passphrase")?;
let data = ar.read_file("private.bin")?;
```

### Read an archive

```rust
let mut ar = Archive::open("output.6cy")?;

// List all files
for info in ar.list() {
    println!("{}: {} bytes ({} blocks)", info.name, info.original_size, info.block_count);
}

// Read a whole file
let data = ar.read_file("readme.txt")?;

// Random access (spans chunk boundaries)
let mut buf = [0u8; 4096];
let n = ar.read_at("data.bin", 1_048_576, &mut buf)?;

// Extract everything
ar.extract_all("./output/")?;
```

### Metadata

```rust
println!("UUID:      {}", ar.uuid());
println!("Root hash: {}", ar.root_hash_hex());
```

### Index reconstruction (no INDEX block)

```rust
use sixcy::io_stream::SixCyReader;
use std::fs::File;

let mut reader = SixCyReader::new(File::open("partial.6cy")?)?;
let reconstructed = reader.scan_blocks()?;
for record in &reconstructed.records {
    println!("{}: {} bytes", record.name, record.original_size);
}
```

### Index-bypass recovery *(new in v1.0.0)*

```rust
use sixcy::recovery;
use std::fs::File;

let mut src = File::open("damaged.6cy")?;
let mut dst = File::create("recovered.6cy")?;

// Pass None for key if the archive is not encrypted
let report = recovery::extract_recoverable(&mut src, &mut dst, None)?;

println!("Quality:   {:?}", report.quality);
println!("Healthy:   {}", report.healthy_blocks);
println!("Corrupt:   {}", report.corrupt_blocks);
println!("Recovered: {} file(s)", report.index.records.len());
println!("Salvaged:  {:.2} MiB", report.recoverable_bytes as f64 / 1_048_576.0);
```

`BlockHealth` variants:

| Variant | Meaning |
|---------|---------|
| `Healthy` | Header CRC32 and payload BLAKE3 both pass |
| `HeaderCorrupt` | Header CRC32 fails; block skipped |
| `TruncatedPayload { declared: u32, available: u64 }` | Header valid; fewer bytes follow than `comp_size` declares |
| `UnknownCodec { uuid_hex: String }` | Header valid; codec UUID not in this build's registry |

`RecoveryQuality` variants:

| Variant | Condition |
|---------|-----------|
| `Full` | ≥ 95% of blocks healthy |
| `Partial` | ≥ 50% and < 95% of blocks healthy |
| `HeaderOnly` | ≥ 1 block scanned but no healthy DATA blocks reconstructed |
| `Catastrophic` | < 50% of blocks healthy, or zero blocks scanned |

---

## Block Header Layout (v1, 84 bytes)

All fields are little-endian.

```
[ 0]  4 B  magic            0x424C434B  ("BLCK")
[ 4]  2 B  header_version   = 1
[ 6]  2 B  header_size      = 84
[ 8]  2 B  block_type       0=Data  1=Index  2=Solid
[10]  2 B  flags            0x0001=Encrypted
[12] 16 B  codec_uuid       frozen 16-byte UUID (LE field order)
[28]  4 B  file_id          0xFFFFFFFF for Solid/Index blocks
[32]  8 B  file_offset      byte offset in decompressed file
[40]  4 B  orig_size        uncompressed bytes
[44]  4 B  comp_size        on-disk bytes
[48] 32 B  content_hash     BLAKE3 of uncompressed plaintext
[80]  4 B  header_crc32     CRC32([0..80])  ← verified first, always
```

---

## Codec UUIDs (frozen forever)

| Codec  | UUID |
|--------|------|
| None   | `00000000-0000-0000-0000-000000000000` |
| Zstd   | `b28a9d4f-5e3c-4a1b-8f2e-7c6d9b0e1a2f` |
| LZ4    | `3f7b2c8e-1a4d-4e9f-b6c3-5d8a2f7e0b1c` |
| Brotli | `9c1e5f3a-7b2d-4c8e-a5f1-2e6b9d0c3a7f` |
| LZMA   | `4a8f2e1c-9b3d-4f7a-c2e8-6d5b1a0f3c9e` |

UUIDs are never reused. A deprecated codec keeps its UUID permanently.

---

## Running Tests

```bash
cargo test
```

## Running Benchmarks

```bash
cargo bench
```

---

## Security

See [`SECURITY.md`](SECURITY.md) for the threat model, hardening details, and
the vulnerability disclosure policy.

---

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md).

---

## License

The **reference implementation** (all `.rs` files, `Cargo.toml`,
`plugin_abi/sixcy_plugin.h`) is licensed under the
**[Apache License 2.0](LICENSE)**.

The **format specification** (`spec.md`) is licensed under
**[CC BY 4.0](LICENSE-SPEC)** — you may implement the format in any language
and share implementations freely, provided you attribute the original
specification.

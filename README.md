<div align="center">

# .6cy Container Format

**v0.3.0** · Reference implementation in Rust

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

---

## Project Layout

```
sixcy/
├── Cargo.toml                   # version 0.3.0, Apache-2.0
├── LICENSE                      # Apache-2.0 (code)
├── LICENSE-SPEC                 # CC BY 4.0 (spec.md)
├── README.md                    # this file
├── BENCHMARK.md                 # detailed benchmark report
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
    ├── codec/mod.rs             # frozen UUID registry + built-in codecs
    ├── crypto/mod.rs            # AES-256-GCM + Argon2id
    ├── index/mod.rs             # FileIndex, BlockRef
    ├── io_stream/mod.rs         # SixCyWriter, SixCyReader, scan_blocks
    └── recovery/mod.rs          # RecoveryMap + checkpoints
```

---

## Getting Started

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) stable (1.70+)
- No C toolchain required — all dependencies are pure Rust

### Build

```bash
git clone https://github.com/byte271/6cy.git
cd 6cy
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

### `optimize` — re-compress at maximum ratio

```bash
6cy optimize archive.6cy -o archive_max.6cy          # Zstd level 19 (default)
6cy optimize archive.6cy -o archive_max.6cy --level 19
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

## Container Format Specification

See [`spec.md`](spec.md)

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md).

---

## License

The **reference implementation** (all `.rs` files, `Cargo.toml`,
`plugin_abi/sixcy_plugin.h`) is licensed under the
**[Apache License 2.0](LICENSE)**.

The **format specification** (`spec.md`) is licensed under
**CC BY 4.0** — you may implement the format in any language
and share implementations freely, provided you attribute the original
specification.




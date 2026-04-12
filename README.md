# sixcy — .6cy Container Format

A Rust implementation of the `.6cy` archive format featuring:

- **Self-describing blocks**: Every block carries its own magic, version, codec UUID, sizes, and integrity checksums
- **Mandatory integrity verification**: CRC32 header checks + BLAKE3 content hashes on every block
- **Multiple codecs**: Zstd (default), LZ4, Brotli, LZMA, and passthrough (none)
- **AES-256-GCM encryption** with Argon2id key derivation
- **Content-defined chunking** (Gear64 CDC) for improved deduplication
- **Reed-Solomon FEC** for recovery from missing/corrupt blocks
- **Merkle proofs** for efficient integrity verification
- **Shamir Secret Sharing** for key fragmentation
- **Basic steganography** utilities for hiding data in image LSBs

## Quick Start

```bash
# Pack files into an archive
6cy pack -o archive.6cy -i file1.txt file2.txt

# List archive contents
6cy list archive.6cy

# Extract files
6cy unpack archive.6cy -C ./output_dir

# View archive info
6cy info archive.6cy

# Create encrypted archive
6cy pack -o secure.6cy -i sensitive.dat --password "mypassword"

# Use FEC for error recovery (10 data shards, 4 parity shards)
6cy pack -o resilient.6cy -i data/ --fec 10/4

# Use content-defined chunking for deduplication
6cy pack -o dedup.6cy -i data/ --cdc --merkle

# Recover from damaged archive
6cy recover damaged.6cy -o recovered.6cy

# Merge archives
6cy merge archive1.6cy archive2.6cy -o merged.6cy
```

## CLI Commands

| Command | Description |
|---------|-------------|
| `pack` | Create archive from files/directories |
| `unpack` | Extract files from archive |
| `list` | List archive contents |
| `info` | Show archive metadata |
| `scan` | Scan blocks without index |
| `recover` | Extract recoverable data from damaged archive |
| `optimize` | Re-compress at different level |
| `merge` | Combine multiple archives |
| `bench` | Benchmark RLE pre-filter |
| `vanta` | Steganography subcommands |

## Library Usage

```rust
use sixcy::Archive;

let opts = sixcy::PackOptions {
    default_codec: sixcy::CodecId::Zstd,
    level: 3,
    use_cdc: true,
    build_merkle: true,
    ..Default::default()
};

let mut ar = Archive::create("data.6cy", opts)?;
ar.add_file("readme.txt", b"Hello, world!")?;
ar.finalize()?;
```

## Architecture

```
src/
├── archive.rs      # High-level Archive API
├── block.rs        # Block format (84-byte headers)
├── superblock.rs   # Archive header (512 bytes)
├── codec/          # Compression codec registry
├── crypto/         # AES-256-GCM + Argon2id
├── cdc.rs          # Gear64 content-defined chunking
├── fec.rs          # Reed-Solomon forward error correction
├── merkle.rs       # Binary Merkle tree + proofs
├── sharding.rs     # Shamir's Secret Sharing
├── stego.rs        # LSB steganography
├── chaos.rs        # Block scrambling utilities
├── io_stream/      # Archive reader/writer
├── index/          # File index management
├── recovery/       # Index-bypass recovery scanner
├── perf.rs        # Performance utilities
└── plugin.rs      # C ABI for codec plugins
```

## Format Version

Current: **v2.0.0** (`format_version = 2`)

The `.6cy` format is documented in `spec.md`.

## License

- Code: Apache-2.0
- Specification: CC BY 4.0

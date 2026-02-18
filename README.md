
> [!IMPORTANT]
> 6cy is under active development and is not yet stable.
> The file format and APIs may change without notice.

> [!NOTE]
> This project is intended for benchmarking, research, and ecosystem prototyping.
> It is not recommended for production use yet.

# Sixcy: High-Performance Streaming Container

Sixcy is a modern, efficient, and robust container format designed for high-performance data storage and transmission. It is built with a focus on streaming efficiency, data recoverability, and flexible compression strategies.

## Key Features

*   **Streaming-First Design**: Optimized for single-pass read and write operations, making it ideal for network streams and large-scale data processing.
*   **Data Recoverability**: Includes self-describing blocks and periodic checkpoints to ensure data can be recovered even if the archive is truncated or partially corrupted.
*   **Codec Polymorphism**: Supports multiple compression algorithms (Zstd, LZ4, etc.) within a single archive, allowing for block-level optimization based on data type.
*   **Plugin Architecture**: Supports third-party and proprietary compression algorithms through a well-defined plugin interface, enabling closed-source extensions to integrate seamlessly.
*   **Metadata-First Indexing**: Provides a centralized index for fast random access and efficient file listing without scanning the entire archive.
*   **Content-Addressable Storage (CAS)**: Each block is identified by a BLAKE3 hash of its uncompressed content. Duplicate files within an archive are automatically deduplicated — only one copy of the compressed data is stored, regardless of how many files reference it.
*   **Solid Mode**: Multiple files can be packed into a single compressed solid block for improved compression ratios on correlated data.
*   **Multi-File Packing**: The `pack` command accepts multiple input files in a single invocation.
*   **Rust Reference Implementation**: A high-performance, memory-safe implementation that serves as the canonical reference for the Sixcy specification.

## Project Structure

*   `src/`: Core library and CLI implementation.
    *   `lib.rs`: Library entry point.
    *   `superblock.rs`: Fixed header management.
    *   `block.rs`: Data block encoding and decoding.
    *   `codec/`: Compression algorithm implementations and plugin interface.
    *   `index/`: File index and metadata management.
    *   `recovery/`: Data recovery and checkpointing logic.
    *   `io_stream/`: High-level streaming I/O interfaces.
*   `tests/`: Integration tests for format validation.
*   `benches/`: Performance benchmarks for compression and I/O.
*   `spec.md`: Detailed binary format specification.
*   `security.md`: Public security profile and threat model.

## Benchmarks

Preliminary benchmark results are available in `BENCHMARK.md`.

These measurements were collected using the Canterbury Corpus to evaluate
compression ratio and throughput under real-world text and binary workloads.

## Getting Started

### Prerequisites

Go to [Environmental_preparation.md](Environmental_preparation.md) to view Environmental preparation

### Building

```bash
cargo build --release
```

The compiled CLI tool will be available at `target/release/6cy`.

> [!NOTE]
> What This Build Provides
The open-source build includes:
- .6cy container format implementation
- Streaming I/O engine
- Recovery logic
- Standard codecs (Zstd / LZ4)
- Plugin ABI for external codecs
- CAS deduplication engine (BLAKE3)
- Solid mode compression

> [!NOTE]
> What Is NOT Included

- Proprietary codecs and optimization
- components are not part of
- the open-source tree. They can be
- integrated via the plugin interface.

## Start

```bash
cd C:\sixcy\target\release
```

**Windows run**

```bash
.\6cy.exe --help
```
> [!NOTE]
> Before running any commands, make sure you are in this directory

**Linux&MacOS run**

```bash
chmod +x 6cy
./6cy --help
```

You will see pack, unpack, list, info, optimize

## Usage Examples

**Pack multiple files:**
```bash
6cy pack file1.txt file2.bin image.png --output archive.6cy --codec zstd
```

**Pack with solid mode** (better compression for many small, similar files):
```bash
6cy pack *.log --output logs.6cy --codec zstd --solid
```

**Unpack an archive:**
```bash
6cy unpack archive.6cy -C ./output_dir
```

**List archive contents** (shows block hash prefixes, sizes, and compression):
```bash
6cy list archive.6cy
```

**Show archive metadata:**
```bash
6cy info archive.6cy
```

**Optimize an existing archive** (re-compresses with Zstd, deduplicates):
```bash
6cy optimize old.6cy --output optimized.6cy
```

## Documentation

*   **Specification**: The full binary format specification is available in [spec.md](spec.md).
*   **Security**: Security considerations and reporting procedures are detailed in [security.md](security.md).

## Release Roadmap

v0.1.x (current)
Provides the reference implementation of the .6cy container format,
streaming engine, CAS deduplication, solid mode, and plugin interface.

This version is intended for:
- Format validation
- Integration testing
- Research and benchmarking

v0.2.0 (planned)
Will introduce the official runtime package and extended codec support.

If you are looking for a ready-to-use build, please wait for the v0.2.0 release.


## License

This project uses different licenses for the specification and the implementation:

*   **Specification (`spec.md`)**: Licensed under [CC-BY-4.0](https://creativecommons.org/licenses/by/4.0/).
*   **Implementation (`src/`, `tests/`, `benches/`, etc.)**: Licensed under [Apache-2.0](LICENSE).

---
*Sixcy - Speed, Safety, and Flexibility in Data Containment.*

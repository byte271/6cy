
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

## Getting Started

### Prerequisites

*   [Rust](https://www.rust-lang.org/tools/install) (latest stable version)

### Building

```bash
cargo build --release
```

The compiled CLI tool will be available at `target/release/6cy`.

## Documentation

*   **Specification**: The full binary format specification is available in [spec.md](spec.md).
*   **Security**: Security considerations and reporting procedures are detailed in [security.md](security.md).

## License

This project uses different licenses for the specification and the implementation:

*   **Specification (`spec.md`)**: Licensed under [CC-BY-4.0](https://creativecommons.org/licenses/by/4.0/).
*   **Implementation (`src/`, `tests/`, `benches/`, etc.)**: Licensed under [Apache-2.0](LICENSE).

---
*Sixcy - Speed, Safety, and Flexibility in Data Containment.*

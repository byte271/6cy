# Sixcy (.6cy) Container Format Specification
Version: 1.0.0
License: [CC-BY-4.0](https://creativecommons.org/licenses/by/4.0/)

## 1. Introduction
Sixcy is a high-performance container format designed for streaming data with robust recovery and flexible compression. This specification defines the binary layout and interoperability requirements for the .6cy format.

## 2. Binary Layout
A Sixcy file is composed of a fixed-size Superblock followed by a sequence of Data Blocks.

### 2.1 Superblock
The Superblock (offset 0) contains global metadata.

| Field | Type | Size | Description |
| :--- | :--- | :--- | :--- |
| Magic | `[u8; 4]` | 4 | `.6cy` |
| Version | `u32` | 4 | Format version |
| UUID | `[u8; 16]` | 16 | Archive unique ID |
| Index Offset | `u64` | 8 | Offset to File Index |
| Recovery Offset | `u64` | 8 | Offset to Recovery Map |
| Feature Bitmap | `u64` | 8 | Enabled features |

### 2.2 Data Block
Each block is self-describing, allowing for mixed compression types.

| Field | Type | Size | Description |
| :--- | :--- | :--- | :--- |
| Block Magic | `[u8; 4]` | 4 | `6cyB` |
| Payload Size | `u32` | 4 | Compressed size |
| File ID | `u64` | 8 | Associated file ID |
| File Offset | `u64` | 8 | Original file offset |
| Codec ID | `u8` | 1 | Compression algorithm |
| Level | `i8` | 1 | Compression level |
| Flags | `u16` | 2 | Block flags |
| Checksum | `u32` | 4 | CRC32C of payload |

## 3. Codec and Plugin Interface
Sixcy supports a wide range of compression algorithms. While basic codecs are included in the reference implementation, the format supports third-party extensions via a plugin interface.

### 3.1 Standard Codecs
| Codec | ID | Description |
| :--- | :--- | :--- |
| None | 0 | No compression |
| Zstd | 1 | Zstandard |
| Lz4 | 2 | LZ4 |

### 3.2 Plugin Interface (ABI/JSON Manifest)
To support proprietary or high-value compression algorithms without disclosing source code, Sixcy implementations should support a plugin architecture.

**Plugin Manifest (`plugin.json`)**:
```json
{
  "name": "custom-codec",
  "version": "1.0.0",
  "codec_id": 128,
  "capabilities": ["compress", "decompress"]
}
```

**ABI Requirements**:
Plugins must export the following functions (C-compatible):
*   `sixcy_codec_id()`: Returns the `u8` ID.
*   `sixcy_compress(input, input_len, output, output_max_len, level)`: Returns compressed size.
*   `sixcy_decompress(input, input_len, output, output_max_len)`: Returns decompressed size.

## 4. Security and Integrity
Sixcy archives should be parsed using safe memory patterns. Implementations must verify the `Checksum` field for every block before processing the payload.

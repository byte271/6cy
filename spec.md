# Sixcy (.6cy) Container Format Specification
Version: 1.1.0
License: [CC-BY-4.0](https://creativecommons.org/licenses/by/4.0/)

## 1. Introduction
Sixcy is a high-performance container format designed for streaming data with robust recovery and flexible compression. This specification defines the binary layout and interoperability requirements for the .6cy format.

## 2. Binary Layout
A Sixcy file is composed of a fixed-size Superblock followed by a sequence of Data Blocks, a FileIndex, and a RecoveryMap.

### 2.1 Superblock
The Superblock (offset 0) contains global metadata. All integer fields are little-endian.

| Field | Type | Size | Description |
| :--- | :--- | :--- | :--- |
| Magic | `[u8; 4]` | 4 | `.6cy` |
| Version | `u32` | 4 | Format version (currently `1`) |
| UUID | `[u8; 16]` | 16 | Archive unique ID (UUIDv4) |
| Index Offset | `u64` | 8 | Absolute byte offset to the FileIndex |
| Recovery Offset | `u64` | 8 | Absolute byte offset to the RecoveryMap |
| Index Size | `u64` | 8 | Byte length of the serialized FileIndex |
| Codec Count | `u32` | 4 | Number of entries in the Required Codecs list |
| Required Codecs | `[u16; N]` | 2×N | Codec IDs that must be available to decompress this archive |

> **Note:** The superblock is written at offset 0. Writers reserve a 128-byte placeholder before writing any blocks and seek back to write the final superblock after `finalize()`.

### 2.2 Data Block
Each block is self-describing, enabling mixed compression types within a single archive.

| Field | Type | Size | Description |
| :--- | :--- | :--- | :--- |
| Block Magic | `u32` | 4 | `0x424C434B` (`BLCK`) |
| Payload Size | `u32` | 4 | Compressed payload size in bytes |
| File ID | `u32` | 4 | Associated file record ID |
| File Offset | `u64` | 8 | Byte offset within the original (uncompressed) file |
| Codec ID | `u16` | 2 | Compression algorithm (see §3.1) |
| Level | `i8` | 1 | Compression level |
| Flags | `u16` | 2 | Block-specific flags (reserved, set to 0) |
| Checksum | `u32` | 4 | CRC32 of the compressed payload |
| Content Hash | `[u8; 32]` | 32 | BLAKE3 hash of the **uncompressed** payload |
| Payload | `[u8; N]` | N | Compressed data |

### 2.3 FileIndex
The FileIndex is a JSON-serialized structure stored immediately after the last Data Block. Its location and size are recorded in the Superblock.

Each `FileIndexRecord` contains:

| Field | Type | Description |
| :--- | :--- | :--- |
| `id` | `u32` | Unique file identifier |
| `parent_id` | `u32` | Parent directory ID (0 = root) |
| `name` | `string` | File name |
| `block_refs` | `BlockRef[]` | Ordered list of block references for this file |
| `original_size` | `u64` | Original (uncompressed) file size |
| `compressed_size` | `u64` | Total compressed size across all blocks |
| `metadata` | `map<string,string>` | Arbitrary key-value metadata |

Each `BlockRef` contains:

| Field | Type | Description |
| :--- | :--- | :--- |
| `hash` | `[u8; 32]` | BLAKE3 content hash (matches `Content Hash` in the block header) |
| `offset` | `u64` | Absolute byte offset of the block header in the archive |
| `archive_id` | `string?` | Optional — for cross-archive referencing (v2+) |

The `FileIndex` root object also contains:

| Field | Type | Description |
| :--- | :--- | :--- |
| `records` | `FileIndexRecord[]` | All file records |
| `root_hash` | `[u8; 32]` | BLAKE3 Merkle root of all `BlockRef.hash` values |

> **Backward compatibility:** Readers should also accept the legacy `offsets: [u64]` field in place of `block_refs` and convert each offset to a stub `BlockRef` with a zeroed hash.

### 2.4 RecoveryMap
The RecoveryMap is a JSON-serialized list of periodic checkpoints stored after the FileIndex. Each checkpoint records the archive byte offset and the last successfully written file ID at that point, enabling partial recovery of truncated archives.

## 3. Codec and Plugin Interface
Sixcy supports a wide range of compression algorithms. While basic codecs are included in the reference implementation, the format supports third-party extensions via a plugin interface.

### 3.1 Standard Codecs
| Codec | ID | Description |
| :--- | :--- | :--- |
| None | 0 | No compression |
| Zstd | 1 | Zstandard (default) |
| Lz4 | 2 | LZ4 |
| Brotli | 3 | Brotli |

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
*   `sixcy_codec_id()`: Returns the `u16` ID.
*   `sixcy_compress(input, input_len, output, output_max_len, level)`: Returns compressed size.
*   `sixcy_decompress(input, input_len, output, output_max_len)`: Returns decompressed size.

## 4. Content-Addressable Storage (CAS) and Deduplication
Every Data Block carries a BLAKE3 hash of its uncompressed content in the `Content Hash` field. Writers use this hash as a key into an in-memory deduplication table: if identical content has already been written, the new `BlockRef` points to the existing block offset instead of writing a second copy. Readers verify the hash after decompression to detect silent corruption.

The `root_hash` field in the FileIndex is the BLAKE3 hash of all `BlockRef.hash` values concatenated in record/block order, enabling remote integrity verification without reading block payloads.

## 5. Security and Integrity
Sixcy archives should be parsed using safe memory patterns. Implementations must:
1. Verify the `Checksum` (CRC32) field for every block before decompressing.
2. Verify the `Content Hash` (BLAKE3) field after decompressing each block.
3. Validate the `root_hash` in the FileIndex for whole-archive integrity.

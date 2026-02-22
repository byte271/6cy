# .6cy Container Format Specification

**Version:** 0.3.0  
**Status:** Draft  
**License:** [CC BY 4.0](LICENSE-SPEC) — Creative Commons Attribution 4.0 International

> © 2026 Cyh. You are free to share and adapt this specification provided you
> give appropriate credit. See `LICENSE-SPEC` for the full license text.

---

## Table of Contents

1. [Introduction](#1-introduction)
2. [Definitions and Conventions](#2-definitions-and-conventions)
3. [Archive Layout](#3-archive-layout)
4. [Superblock](#4-superblock)
5. [Block Header](#5-block-header)
6. [Block Types](#6-block-types)
7. [Codec Registry](#7-codec-registry)
8. [Encryption](#8-encryption)
9. [File Index](#9-file-index)
10. [Recovery Map](#10-recovery-map)
11. [Block Reconstruction Algorithm](#11-block-reconstruction-algorithm)
12. [Integrity Verification](#12-integrity-verification)
13. [Plugin ABI](#13-plugin-abi)
14. [Version History](#14-version-history)

---

## 1. Introduction

The **.6cy container format** is a binary archive format designed around five
core properties:

1. **Self-describing blocks.** Every block carries its own magic number,
   version, codec UUID, sizes, and two independent integrity checksums. A
   reader can process any single block in isolation without a directory or index.

2. **Mandatory, non-optional integrity.** Both a per-header CRC32 and a
   per-content BLAKE3 hash are always present and always verified. There is no
   flag to disable them and no fast path that skips them.

3. **Frozen codec identity via UUID.** Codecs are identified by a permanently
   assigned 128-bit UUID stored verbatim in every block header. Short numeric
   IDs are an in-process dispatch optimization only; they are never written to
   disk or used for codec negotiation.

4. **No runtime negotiation.** The superblock declares every required codec
   UUID upfront. A decoder either supplies all of them or fails immediately
   before reading any block. There is no partial decode, no fallback codec,
   and no runtime codec advertisement.

5. **Reconstructible index.** The FILE INDEX block is written at the end of
   the archive. Because every DATA block header embeds the file ID, file
   offset, and original size, the complete block list can be reconstructed by
   reading headers sequentially forward without the index and without
   decompressing any payload.

---

## 2. Definitions and Conventions

### Byte order

**All multi-byte numeric fields are little-endian.** This is a hard,
non-negotiable choice encoded into the format version. A hypothetical
big-endian variant of this format would carry a different magic number.

### UUID encoding

UUIDs are stored as 16 raw bytes in **little-endian RFC 4122 field order**.
The four bytes of `time_low` are stored with byte 3 first; the two bytes of
`time_mid` are stored with byte 1 first; and so on. Fields are matched
byte-for-byte; no byte-swapping is performed at runtime.

### Notation

```
[offset]  width  field_name   — description
```

Offsets are absolute from the start of the containing structure. Widths use
`B` for bytes. `LE u32` means an unsigned 32-bit integer in little-endian
byte order.

---

## 3. Archive Layout

A `.6cy` archive is a flat binary file with the following structure:

```
Offset    Size     Region
────────────────────────────────────────────────────────────────────
       0    256 B   SUPERBLOCK   (always at offset 0; padded to 256 B)
     256  variable  DATA and SOLID blocks (any order; zero or more)
variable  variable  INDEX block  (last substantial block; always present)
variable  variable  RECOVERY MAP (8-byte LE length prefix + JSON payload)
────────────────────────────────────────────────────────────────────
```

The superblock is patched in-place at offset 0 during `finalize()`. All other
regions are append-only. Readers are not required to parse the recovery map to
decode any file; it exists solely to accelerate partial-archive recovery.

---

## 4. Superblock

The superblock is always at offset 0 and is exactly 256 bytes on disk.

### 4.1 Layout

```
[ 0]  4 B   magic                ".6cy" (4 ASCII bytes)
[ 4]  4 B   format_version       LE u32 = 3
[ 8] 16 B   archive_uuid         unique per archive; Argon2id KDF salt for encryption
[24]  4 B   flags                LE u32 — see §4.2
[28]  8 B   index_offset         LE u64 — absolute byte offset of the INDEX block header
[36]  8 B   index_size           LE u64 — compressed INDEX payload size in bytes
[44]  2 B   required_codec_count LE u16 — N
[46] N×16 B required_codec_uuids N × 16 raw UUID bytes (LE field order each)
[46+N×16]  4 B  header_crc32     LE u32 — CRC32 of buf[0 .. 46+N×16]
[50+N×16] ..   zero padding       to reach exactly 256 bytes
```

**Maximum codec count:** 13 distinct non-None codecs per superblock
(⌊(256 − 50) / 16⌋ = 13).

### 4.2 Superblock Flags

| Bit | Mask | Meaning |
|-----|------|---------|
| 0 | `0x0000_0001` | At least one block is AES-256-GCM encrypted |
| 1–31 | — | Reserved; MUST be zero on write; ignored on read |

### 4.3 Required Codec UUIDs

Lists every codec UUID that appears in any DATA or SOLID block. The `None`
codec (all-zero UUID) is never listed. A decoder MUST check this list
immediately after parsing the superblock. If any UUID is absent from the
decoder's registry, the decoder MUST return an error and MUST NOT read any
block.

### 4.4 CRC32

`header_crc32` is CRC32 (IEEE 802.3) of bytes `[0 .. 46+N×16)`. A reader MUST
verify this before reading any other field. Mismatch is a fatal error.

### 4.5 Format Version Policy

| `format_version` | Status |
|-----------------|--------|
| 1 | v0.1.x — incompatible with v3 readers |
| 2 | v0.2.x — incompatible with v3 readers |
| 3 | v0.3.0+ — current |

A v3 reader MUST reject `format_version < 3` or `format_version > 3`.

---

## 5. Block Header

Every block begins with an 84-byte header.

### 5.1 Layout

```
[ 0]  4 B   magic            LE u32 = 0x424C434B ("BLCK")
[ 4]  2 B   header_version   LE u16 = 1
[ 6]  2 B   header_size      LE u16 = 84
[ 8]  2 B   block_type       LE u16 — see §6
[10]  2 B   flags            LE u16 — see §5.2
[12] 16 B   codec_uuid       16 raw bytes, LE UUID field order
[28]  4 B   file_id          LE u32
[32]  8 B   file_offset      LE u64 — byte offset in decompressed file
[40]  4 B   orig_size        LE u32 — uncompressed payload bytes
[44]  4 B   comp_size        LE u32 — on-disk bytes
[48] 32 B   content_hash     BLAKE3 of uncompressed plaintext
[80]  4 B   header_crc32     LE u32 — CRC32 of buf[0..80]
```

Total: **84 bytes**.

### 5.2 Block Header Flags

| Bit | Mask | Meaning |
|-----|------|---------|
| 0 | `0x0001` | Payload is AES-256-GCM encrypted |
| 1–15 | — | Reserved |

### 5.3 `file_id`

Sequential 0-based file index for DATA blocks. Sentinel `0xFFFF_FFFF` for SOLID
and INDEX blocks.

### 5.4 `header_crc32`

CRC32 of `buf[0..80]`. MUST be verified before seeking by `comp_size`.

### 5.5 `content_hash`

BLAKE3 of the **uncompressed, unencrypted plaintext**. Serves as the CAS key
and is verified after decompression. A hash mismatch after decompression is
always a fatal error.

### 5.6 Forward Compatibility

Future header versions may extend the header. Use `header_size` as the payload
offset rather than the fixed value 84. If `header_size < 84`, the header is
malformed.

---

## 6. Block Types

| `block_type` | Name | Description |
|-------------|------|-------------|
| 0 | DATA | One contiguous chunk of one file |
| 1 | INDEX | Compressed FILE INDEX; written last; `file_id = 0xFFFF_FFFF` |
| 2 | SOLID | Multiple files concatenated; `file_id = 0xFFFF_FFFF` |
| 3+ | — | Reserved; MUST be rejected |

---

## 7. Codec Registry

### 7.1 Frozen UUIDs

| Codec | UUID (canonical) | LE bytes [0..3] |
|-------|-----------------|-----------------|
| None | `00000000-0000-0000-0000-000000000000` | `00 00 00 00` |
| Zstd | `b28a9d4f-5e3c-4a1b-8f2e-7c6d9b0e1a2f` | `4f 9d 8a b2` |
| LZ4 | `3f7b2c8e-1a4d-4e9f-b6c3-5d8a2f7e0b1c` | `8e 2c 7b 3f` |
| Brotli | `9c1e5f3a-7b2d-4c8e-a5f1-2e6b9d0c3a7f` | `3a 5f 1e 9c` |
| LZMA | `4a8f2e1c-9b3d-4f7a-c2e8-6d5b1a0f3c9e` | `1c 2e 8f 4a` |

UUIDs are **never reused**. A deprecated codec retains its UUID permanently.

### 7.2 Short IDs

Short numeric IDs are an **in-process** dispatch optimization. They are never
written to disk or transmitted. A reader with no short-ID table functions
correctly by matching `codec_uuid` bytes directly.

### 7.3 Unknown UUID

If a block UUID is in `required_codec_uuids` but unknown to the decoder, the
decoder should have already failed at superblock parse time. If a block UUID
is NOT in `required_codec_uuids`, the reader MAY skip this block.

### 7.4 Compression Levels

| Codec | Level range | Notes |
|-------|------------|-------|
| None | — | Ignored |
| Zstd | 1–19 | Default: 3 |
| LZ4 | — | Ignored |
| Brotli | 0–11 | Clamped; default: 3 |
| LZMA | — | Implementation-defined |

---

## 8. Encryption

### 8.1 Algorithm

AES-256-GCM. Nonce is 96 bits (12 bytes), randomly generated per block.

### 8.2 Payload Layout

```
[  0] 12 B   nonce
[ 12]  n B   ciphertext
[12+n] 16 B  GCM authentication tag
```

### 8.3 Key Derivation

```
key = Argon2id(
    password = UTF-8 password string,
    salt     = archive_uuid.as_bytes() [16 bytes],
    m        = 65536 KiB,
    t        = 3,
    p        = 1,
    tag_len  = 32
)
```

### 8.4 Decode Sequence

1. Verify `header_crc32`.
2. Read `comp_size` bytes.
3. If `FLAG_ENCRYPTED`: decrypt + authenticate. Fatal on tag failure.
4. Decompress using `codec_uuid`. Fatal on unknown UUID.
5. Verify `BLAKE3(output) == content_hash`. Fatal on mismatch.

### 8.5 INDEX Block

The INDEX block is **never encrypted**, even when all DATA blocks are.

---

## 9. File Index

Stored as a Zstd-compressed JSON payload inside an INDEX block.

### 9.1 FileIndex JSON

```json
{
  "records": [
    {
      "id":              <u32>,
      "parent_id":       <u32>,
      "name":            <string>,
      "block_refs":      [ <BlockRef>, ... ],
      "original_size":   <u64>,
      "compressed_size": <u64>,
      "metadata":        { <string>: <string> }
    }
  ],
  "root_hash": [<u8 × 32>]
}
```

### 9.2 BlockRef JSON

```json
{
  "content_hash":   [<u8 × 32>],
  "archive_offset": <u64>,
  "intra_offset":   <u64>,
  "intra_length":   <u64>
}
```

`intra_offset` and `intra_length` are zero for normal DATA blocks.
For SOLID-block members, they define the byte range within the decompressed
solid payload that belongs to this file.

### 9.3 `root_hash`

BLAKE3 Merkle root over all `content_hash` values in record-order, block-order.

---

## 10. Recovery Map

Appended after the INDEX block:

```
[0]  8 B   payload_len   LE u64
[8]  n B   JSON payload
```

```json
{
  "checkpoints": [
    {
      "archive_offset": <u64>,
      "last_file_id":   <u32>,
      "timestamp":      <i64>
    }
  ]
}
```

Each checkpoint is written after a complete file is packed.

---

## 11. Block Reconstruction Algorithm

When the INDEX block is missing or corrupt, reconstruct the block list by
forward-scanning from offset 256:

```
pos ← 256
for each block header H read at pos:
    verify H.header_crc32
    if H.block_type == INDEX: stop
    if H.block_type == DATA:
        record (H.file_id, H.file_offset, H.orig_size, H.content_hash, pos)
    pos ← pos + 84 + H.comp_size

group records by file_id
sort each group by file_offset
synthesise name = "file_{file_id:08x}"
```

Solid-block file contents cannot be recovered without the INDEX.

---

## 12. Integrity Verification

### 12.1 Encode Order

1. `content_hash = BLAKE3(plaintext)`
2. `compressed = codec.compress(plaintext)`
3. If encrypted: `on_disk = AES-GCM-encrypt(compressed)`, set `FLAG_ENCRYPTED`
4. `header_crc32 = CRC32(header_bytes[0..80])`
5. Write header + on_disk payload

### 12.2 Decode Order

1. Read 84-byte header
2. `CRC32(buf[0..80]) == header_crc32` — **fatal** on mismatch
3. `buf[0..4] == BLOCK_MAGIC` — **fatal** on mismatch
4. Read `comp_size` bytes
5. If `FLAG_ENCRYPTED`: AES-GCM decrypt — **fatal** on tag failure
6. Decompress via `codec_uuid` — **fatal** if UUID unknown
7. `BLAKE3(decompressed) == content_hash` — **fatal** on mismatch

---

## 13. Plugin ABI

See `plugin_abi/sixcy_plugin.h` for the complete C header.

### 13.1 Entry Point

```c
const SixcyCodecPlugin *sixcy_codec_register(void);
```

Returns a static pointer valid for the process lifetime.

### 13.2 Thread Safety

Both `fn_compress` and `fn_decompress` MUST be reentrant. No global mutable
state permitted.

### 13.3 Memory Model

No shared allocator. Host pre-allocates via `fn_compress_bound`. Plugin never
calls host `malloc`/`free`.

### 13.4 ABI Versioning

New fields append at end only. `abi_version > SIXCY_PLUGIN_ABI_VERSION`
causes host rejection. Current ABI version: **1**.

---

## 14. Version History

| Format version | Release | Summary |
|---------------|---------|---------|
| 1 | v0.1.x | Initial: Zstd + LZ4, simple superblock |
| 2 | v0.2.x | Encryption, solid mode, Brotli + LZMA, CAS deduplication |
| 3 | v0.3.0 | Frozen codec UUIDs on disk; block header CRC32; superblock CRC32; no-negotiation codec check; reconstructible index; plugin C ABI v1 |

---

*Specification license: [CC BY 4.0](LICENSE-SPEC)*  
*Reference implementation license: [Apache-2.0](LICENSE)*

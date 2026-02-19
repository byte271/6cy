# Changelog

All notable changes to the `.6cy` container format and reference implementation
are documented here.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).  
Versioning follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [0.3.0] — 2026

### Format changes (format_version bumped 2 → 3)

> Archives produced by v0.3.0 are **not backward compatible** with v0.1.x or
> v0.2.x readers. v0.3.0 readers refuse to open v1 or v2 archives.

#### Block header — fully self-describing (84 bytes, up from 61)

- Added `header_version` (u16, = 1) — enables future layout changes.
- Added `header_size` (u16, = 84) — allows forward-skipping unknown extensions.
- Added `block_type` (u16) — DATA / INDEX / SOLID now encoded in the header,
  not inferred from context. Readers can classify blocks without prior state.
- Added `orig_size` (u32) — uncompressed size embedded in the header. Readers
  can allocate the exact output buffer before decompressing.
- Replaced `block_size` (u32) with `comp_size` (u32) — renamed for clarity.
- **Added `header_crc32` (u32, LE)** — mandatory CRC32 over the first 80 bytes
  of the header. Checked before any seek or allocation. Cannot be disabled.
- `codec_uuid` (16 bytes) replaces `codec_id` (u16) — frozen UUID identity
  on disk; short IDs are in-process only and never written.
- Removed `level` (i8) from the block header — levels are a write-time
  parameter and are irrelevant to decoders.

#### Superblock — format_version 3

- Field `format_version` replaces `version`; renamed for clarity.
- Field `archive_uuid` replaces `uuid`; renamed for clarity.
- `required_codecs` (list of u16 short IDs) replaced by
  `required_codec_uuids` (list of 16-byte UUIDs) — authoritative identifiers
  only; no short IDs on disk.
- Added `header_crc32` (u32) to the superblock — covers the variable-length
  body including the codec UUID list.
- Superblock size fixed at **256 bytes** (up from 128).
- `recovery_map_offset` removed from superblock; recovery map is now always
  immediately after the INDEX block.

#### Codec identity — UUID-primary

- Five codec UUIDs are frozen in the specification:
  - None: `00000000-0000-0000-0000-000000000000`
  - Zstd: `b28a9d4f-5e3c-4a1b-8f2e-7c6d9b0e1a2f`
  - LZ4: `3f7b2c8e-1a4d-4e9f-b6c3-5d8a2f7e0b1c`
  - Brotli: `9c1e5f3a-7b2d-4c8e-a5f1-2e6b9d0c3a7f`
  - LZMA: `4a8f2e1c-9b3d-4f7a-c2e8-6d5b1a0f3c9e`
- Codec availability check moved to superblock open time — fails immediately
  if any required UUID is unavailable. No partial decode, no runtime
  negotiation, no fallback.

#### File Index — BlockRef

- `offset` renamed to `archive_offset` — unambiguous field name.
- `hash` renamed to `content_hash` — matches block header field name.
- Removed legacy `offsets: Vec<u64>` backward-compatibility shim.

### Added

- **`src/plugin.rs`** — safe Rust wrapper around the C plugin ABI.
- **`plugin_abi/sixcy_plugin.h`** — frozen C ABI (ABI version 1) for
  third-party codec plugins. Specifies: entry point, thread safety contract,
  memory model (no shared allocator, explicit buffer pairs), ABI versioning
  policy, and return code table.
- **`6cy scan` subcommand** — reconstructs the file list by forward-scanning
  block headers without reading the INDEX block. Useful for partial/truncated
  archives.
- **`SixCyReader::scan_blocks()`** — programmatic index reconstruction API.
- **`BlockType` enum** — `Data`, `Index`, `Solid` — replacing implicit type
  inference from context.
- **`src/archive.rs`** — high-level `Archive` API for embedding in other
  programs (`Archive::open`, `create`, `open_encrypted`, `add_file`,
  `begin_solid`/`end_solid`, `read_file`, `read_at`, `extract_all`, `list`,
  `stat`, `uuid`, `root_hash_hex`).
- **`BENCHMARK.md`** — official benchmark report: 6cy (LZMA) vs 7z (LZMA2 L1)
  on a 10 GiB file, 3 runs, AMD Ryzen 9 6900HX, Windows 11.
- **`CHANGELOG.md`** (this file).
- **`CONTRIBUTING.md`** — contribution guidelines.
- **`SECURITY.md`** — threat model and vulnerability disclosure policy.
- **`LICENSE`** updated to Apache-2.0.
- **`LICENSE-SPEC`** — CC BY 4.0 for `spec.md`.

### Changed

- `Cargo.toml`: version bumped to `0.3.0`; license field set to `Apache-2.0`.
- `spec.md` fully rewritten to reflect format version 3.
- `README.md` fully rewritten for v0.3.0.
- `security_profile.md` replaced by `SECURITY.md`.
- `6cy info` now prints required codec UUIDs alongside human-readable names.
- `6cy optimize` now uses the `Archive` API.
- `DEFAULT_CHUNK_SIZE` remains 4 MiB; `DEFAULT_COMPRESSION_LEVEL` remains 3.

### Fixed

- `read_at` now correctly spans chunk boundaries — previously returned short
  data silently when a requested range crossed a block boundary.
- CAS-deduplicated files now show a non-zero `compressed_size` in index.
- `CodecId::None` is no longer added to `required_codec_uuids` — the None
  codec requires no decoder capability.
- Block magic validated on every read (header CRC32 catches corruption earlier).

---

## [0.2.0] — 2026 *(development milestone — never released)*

> **This version was not published.** It represents an internal development
> phase between v0.1.1 and v0.3.0. No binary or crate was released under
> this version number. All features and fixes developed here are first
> available to users in **v0.3.0**.

### Format changes (format_version 1 → 2)

#### New features

- **AES-256-GCM block-level encryption** with Argon2id key derivation.
  - Key = `Argon2id(password, salt=archive_uuid)`.
  - Per-block random 12-byte nonce prepended to ciphertext.
  - `FLAGS_ENCRYPTED = 0x0001` in block header flags.
- **Solid mode** — multiple files compressed together as a single block.
  - `BlockRef` extended with `intra_offset` and `intra_length`.
  - `FLAGS_SOLID = 0x0002` in block header flags.
- **Extended codec support** — Brotli (ID 3) and LZMA (ID 4) added.
- **Chunked multi-block streaming** — files larger than `chunk_size` are
  automatically split into multiple sequential blocks.
- **CAS deduplication** — identical chunks are written once; subsequent
  references store only a `BlockRef`.

#### Bug fixes (from v0.1.1)

- `aes-gcm` dependency: added `features = ["getrandom"]`.
- `argon2` 0.5 API: `hash_raw()` replaces `hash_password_into()`.
- `aes-gcm` 0.10 API: `new_from_slice()`, `generate_nonce()` from `AeadCore`.
- Superblock reserved to 128 bytes (later extended to 256 in v0.3.0).
- Block magic validated on read.
- `read_at` spans block boundaries correctly.

#### Added

- `PackOptions` builder for the writer.
- `--password`, `--solid`, `--codec`, `--chunk-size` CLI flags.
- `RecoveryMap` with per-file checkpoints.

---

## [0.1.1] — 2026

### Fixed

- Multi-file archive decompression: `unpack` command failed on archives
  containing more than one file due to a file ID assignment bug.
- `SixCyWriter::with_options()` constructor (added in this release).

---

## [0.1.0] — 2026

### Added

- Initial release.
- Binary format: superblock + data blocks + file index (JSON).
- Codecs: Zstd (default), LZ4.
- CLI: `pack`, `unpack`, `list`, `info`.
- BLAKE3 content hash per block.
- Rust reference implementation (`sixcy` crate).

# Changelog

All notable changes to the `.6cy` container format and reference implementation
are documented here.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).  
Versioning follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [1.0.0] — 2026-02-21

### Summary

First stable release. No format changes — archives produced by v0.3.0 are
fully readable by v1.0.0 and vice versa (`format_version = 3` unchanged).
This release promotes the desktop GUI to feature-complete status and ships
three new CLI subcommands (`recover`, `merge`, `bench`).

> **Benchmark note:** All published benchmark data (see `BENCHMARK.md`) was
> collected under v0.3.0. v1.0.0 contains no changes to the core codec, I/O,
> or compression pipeline. Updated benchmark numbers will be published in a
> subsequent release once v1.0.0 benchmarks are completed.

### Added — CLI

- **`6cy recover`** — index-bypass full recovery. Forward-scans all block
  headers and reassembles every readable file into a new archive. Works when
  the INDEX block, RecoveryMap, and all directory structure are absent or
  corrupt. Reports per-block health (`Healthy` / `HeaderCorrupt` /
  `TruncatedPayload` / `UnknownCodec`) and a summary quality rating
  (`Full` / `Partial` / `HeaderOnly` / `Catastrophic`). Accepts optional
  `--password` for encrypted sources.
- **`6cy merge`** — merge two or more `.6cy` archives into a single output
  with cross-archive CAS deduplication. Files from each source are
  namespaced under the source archive stem. Output codec is configurable.
- **`6cy bench`** — RLE pre-filter benchmark. Measures encode time, decode
  time, savings percentage, and round-trip correctness for any input file.

### Added — Library API

- **`recovery::extract_recoverable(src, dst, key)`** — programmatic
  index-bypass recovery. Returns a `RecoveryReport` containing:
  - `total_scanned`, `healthy_blocks`, `corrupt_blocks`,
    `truncated_blocks`, `unknown_codec_blocks`
  - `quality: RecoveryQuality` (`Full` / `Partial` / `HeaderOnly` /
    `Catastrophic`)
  - `recoverable_bytes: u64`
  - `index: FileIndex` — the reconstructed file index
  - `block_log: Vec<ScannedBlock>` — per-block diagnostic records
  - `summary() -> String` — human-readable summary line
- **`recovery::BlockHealth`** enum:
  - `Healthy`
  - `HeaderCorrupt`
  - `TruncatedPayload { declared: u32, available: u64 }`
  - `UnknownCodec { uuid_hex: String }`
- **`perf::rle_encode(data: &[u8]) -> Vec<u8>`** — RLE pre-filter encoder
  used by the `bench` subcommand.
- **`perf::rle_decode(data: &[u8]) -> Option<Vec<u8>>`** — RLE pre-filter
  decoder; returns `None` on malformed input.

### Added — Desktop GUI (sixcy-app v1.0.0)

All panels in the 6cy Archive Suite desktop application have been updated.
Three panels are new; six existing panels received improvements.

#### New panels

- **Recover** — animated block-health grid (each block rendered as a
  coloured cell: teal=healthy, orange=corrupt, amber=truncated,
  grey=unknown). Per-category stat cards (healthy / corrupt / truncated /
  unknown-codec counts). Health-score progress bar. Quality rating chip.
  Recoverable MiB displayed. Supports encrypted archives via password field.
- **Optimize** — re-compress all blocks at a chosen Zstd level (1–19).
  Before/after size and savings percentage displayed as stat cards after
  completion. Accepts recent archives from the quick-picker.
- **Merge** — multi-source archive list with add/remove. Drag-to-reorder
  rows (HTML5 drag events). Output codec selector (Zstd/LZMA/LZ4/Brotli/
  None). Stat cards showing sources merged, total files, and output archive
  size.
- **Bench** — drag-drop or browse any input file. Displays encode time,
  decode time, savings percentage, input size, and encoded size. Persists a
  run-history comparison table (up to 8 entries) across benchmarks within
  the same session.

#### Improvements to existing panels

- **Pack** — per-codec level bounds enforced in UI (Zstd 1–19, LZMA 1–9,
  LZ4 1–12, Brotli 0–11, None 0–0); level clamped automatically on codec
  switch. Keyboard shortcut ⌘/Ctrl+Enter to pack. Post-pack stat cards
  (archive size, ratio, status). Animated spinner during operation. Codec
  badge row in drop zone for at-a-glance codec overview.
- **Unpack** — post-unpack stat cards (files extracted, total size, BLAKE3
  integrity status).
- **List** — filename filter with live match count. Total original /
  compressed size and overall ratio in a summary bar. Zero-denominator
  guard on per-file ratio display.
- **Info** — copy-to-clipboard buttons on archive path, archive UUID,
  individual codec UUIDs, and root BLAKE3 hash.
- **Scan** — filename filter on recovered file list; empty-state message
  when filter matches nothing.

#### Cross-panel GUI improvements

- **Dark mode** — full dark theme driven by CSS custom properties;
  persists across sessions via `localStorage`.
- **Recent archives** — last 10 used archive paths shown as a collapsible
  quick-picker in every panel that accepts an archive input.
- **Keyboard navigation** — Alt+1–9 instantly switches between all nine
  panels. Key hint shown in header bar.
- **Log export** — "↓ Export log" button in every terminal widget saves
  the full session log (with timestamps) to a `.txt` file via an object URL
  download.
- **Codec tooltips** — hovering any codec badge renders a plain-English
  description of the codec's speed/ratio trade-off via the native `title`
  attribute.
- **Animated busy states** — spinning `⬡` icon replaces static button
  text during all async Tauri invocations.
- **Stat cards** — consistent `StatCard` component used across Pack,
  Unpack, Optimize, Merge, Recover, and Bench panels for post-operation
  result display.

### Fixed — GUI

- **`fmtBytes` binary prefix mismatch** — the v0.3.0 implementation used
  SI thresholds (10⁹ for "GiB", 10⁶ for "MiB") but labelled them with
  binary unit suffixes. Corrected to exact binary thresholds
  (2³⁰ = 1 073 741 824 for GiB, 2²⁰ = 1 048 576 for MiB,
  2¹⁰ = 1 024 for KiB). Affects all size displays across all panels.
- **Unclamped numeric inputs** — level and chunk-size text inputs
  previously allowed any value; out-of-range values were silently passed to
  the backend and could cause Rust panics or unexpected behaviour. Inputs
  are now clamped to valid ranges at the React layer before any Tauri
  invocation.
- **Codec level not re-clamped on codec switch** — switching from Zstd
  (max level 19) to LZMA (max level 9) while level was set above 9 left
  the stale value in state, causing a backend validation error on the next
  pack attempt. Level is now clamped to `[lMin, lMax]` in a `useEffect`
  triggered by codec change.
- **Drag-and-drop listener leak in Pack panel** — `getCurrentWindow().
  onDragDropEvent()` returns a promise. If the Pack panel unmounted before
  the promise resolved, the resulting unlisten function was never stored or
  called, leaking the event listener for the remainder of the window's
  lifetime. Fixed with a `mounted` flag: if the component has already
  unmounted when the promise resolves, the unlisten function is called
  immediately.
- **Division by zero in List panel ratio column** — per-file compression
  ratio `originalSize / compressedSize` was computed without guarding
  against zero-byte files or store-only blocks (compressed size = 0),
  producing `Infinity` in the UI. Both operands are now checked before
  division.
- **Version string mismatch** — sidebar showed `v0.3.0` while all other
  version vectors (package.json, Cargo.toml, tauri.conf.json) had been
  bumped. All version strings are now `1.0.0`.

### Changed

- `Cargo.toml` (Sixcy_CAS): `version` bumped to `1.0.0`.
- `Cargo.toml` (sixcy-app): `version` bumped to `1.0.0`.
- `tauri.conf.json`: `version` bumped to `1.0.0`; added `publisher`,
  `copyright`, `category`, `shortDescription`, `longDescription`, and
  `bundle.windows.webviewInstallMode` (`embedBootstrapper`) for Microsoft
  Store MSIX packaging.
- `README.md` fully rewritten for v1.0.0. Project layout table updated
  to include `perf.rs` and `recovery/scanner.rs`. All new CLI subcommands
  documented with examples. Library API section extended with recovery API.
  Benchmark table annotated as v0.3.0 data.
- `SECURITY.md`: version scope updated to cover v1.0.0; security changelog
  table extended.
- `BENCHMARK.md`: title and header clarified — data is from v0.3.0;
  v1.0.0 benchmarks are pending.

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

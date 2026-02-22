# Security Policy

## Scope

This document covers the security posture of:

- The `.6cy` binary format (format version 3)
- The `sixcy` Rust reference implementation (v1.0.0)
- The `plugin_abi/sixcy_plugin.h` C ABI for codec plugins

---

## Reporting a Vulnerability

**Do not open a public GitHub issue for security vulnerabilities.**

Email the maintainer directly at the address in `Cargo.toml`. Include:

1. A concise description of the vulnerability.
2. The affected version(s).
3. Steps to reproduce (minimal archive file, code snippet, or command sequence).
4. Potential impact assessment (your view of severity).
5. Whether you have a suggested fix.

**Response SLA:**

| Step | Target |
|------|--------|
| Acknowledgement | 48 hours |
| Initial assessment | 7 days |
| Patch or mitigation | 30 days (critical), 90 days (others) |
| Public disclosure | Coordinated with reporter |

We will credit reporters in the release notes and `CHANGELOG.md` unless they
prefer to remain anonymous.

---

## Threat Model

### Assets

| Asset | Sensitivity |
|-------|-------------|
| Archived file contents | High — may contain secrets |
| Encryption key material | Critical |
| Archive metadata (names, sizes) | Medium |
| Format integrity (no silent corruption) | High |

### Trust Boundary

The reference implementation assumes:

- **The archive file is untrusted.** Every byte read from disk is treated as
  potentially attacker-controlled.
- **The password is trusted.** Key derivation is correct; the password itself
  is not validated beyond being a non-empty UTF-8 string.
- **The plugin shared library is trusted.** A plugin that exports
  `sixcy_codec_register` is loaded with the same trust as the host binary.
  Untrusted plugin loading is out of scope for this release.

### Threat Scenarios

#### T1 — Malformed archive triggers memory unsafety

**Attack:** An attacker crafts an archive with extreme `comp_size`,
`orig_size`, or `header_size` values to cause integer overflow, out-of-bounds
read, or excessive allocation.

**Mitigations:**
- `header_crc32` is verified before any field is interpreted. A flipped bit
  in a size field is caught before any allocation.
- All buffer allocations are bounded by the field value (`comp_size` allocates
  exactly `comp_size` bytes). No unbounded reads.
- Rust's safe arithmetic panics on overflow in debug builds; release builds
  use explicit checked arithmetic at superblock parse time.
- `orig_size` is u32 (max 4 GiB per block), limiting decompressor output
  allocation to 4 GiB per block call.

#### T2 — Compression bomb (zip-bomb style)

**Attack:** A block reports `orig_size = 1 MB` but decompresses to 4 GB,
exhausting memory.

**Mitigations:**
- `orig_size` is embedded in the block header and checked before
  decompression begins.
- The decompressor is called with the expected output size as a capacity hint.
  A well-behaved decompressor will not exceed this.
- The BLAKE3 content hash check (`content_hash`) after decompression will fail
  if the decompressor produces more bytes than declared, since the hash was
  computed over the original data.
- **Recommendation for embedders:** Set a `max_orig_size` guard before calling
  `decode_block` in untrusted contexts.

#### T3 — Timing attack on BLAKE3 comparison

**Attack:** Measure the time of the content hash comparison to learn partial
information about the expected hash.

**Mitigation:** The BLAKE3 comparison uses a constant-time byte array equality
check. Rust's `PartialEq` on `[u8; 32]` is not guaranteed constant-time;
embedders in timing-sensitive environments should replace the equality check
with `subtle::ConstantTimeEq`.

#### T4 — Wrong-key decryption accepts ciphertext

**Attack:** A block encrypted with key A is decrypted with key B; the
GCM tag check should fail, but a bug causes it to succeed.

**Mitigation:** AES-256-GCM authentication is delegated entirely to the
`aes-gcm` crate (RustCrypto). The GCM tag covers the entire ciphertext.
A single-bit error in the key or ciphertext causes tag failure with
overwhelming probability (2⁻¹²⁸ false-positive rate).

#### T5 — UUID collision between codecs

**Attack:** A plugin registers a UUID that collides with a built-in codec,
causing the wrong decompressor to be invoked.

**Mitigation:** Built-in codec UUIDs are checked first in `CodecId::from_uuid`.
Plugin UUIDs that collide with built-in UUIDs should be rejected at plugin
load time. (Plugin UUID deduplication is a planned feature for the plugin
registry; currently, the caller is responsible for not loading conflicting
plugins.)

#### T6 — Index block tampering without header CRC32

**Attack:** An attacker modifies the INDEX block to point file records at
wrong block offsets, extracting arbitrary bytes from another file's blocks.

**Mitigation:** Every block header carries its own `header_crc32` and
`content_hash`. A tampered `archive_offset` will point to a region that either
fails the CRC32 check (wrong header bytes), fails the BLAKE3 check (wrong
content), or both. The attacker cannot control what data is extracted.

#### T7 — Password harvesting via timing difference in key derivation

**Attack:** Measure Argon2id completion time across different password inputs
to learn information about the correct password.

**Mitigation:** Argon2id is intentionally slow and memory-hard. Completion
time is dominated by the memory-fill phase (64 MiB), which is independent of
the password value after the initial hash step. Timing differences at the
sub-millisecond level are noise relative to the >100 ms Argon2id wall time.

---

## Hardening Checklist

### Format level

- [x] Every block header carries a mandatory CRC32 (no opt-out)
- [x] Every block payload is verified with BLAKE3 after decompression (no opt-out)
- [x] Superblock carries a CRC32 over its variable-length body
- [x] AES-256-GCM GCM tag authenticates every encrypted payload
- [x] INDEX block is never encrypted (can always be listed without password)
- [x] `format_version` is checked; mismatches are hard errors
- [x] Magic bytes validated before any field parsing

### Implementation level

- [x] All I/O uses explicit little-endian field reads (no implicit byte-order)
- [x] `comp_size` read before allocation; size is bounded by u32
- [x] `try_into().unwrap()` on fixed-size slice conversions — panics if slice
      is wrong size, which indicates a bug in the header-size constant
- [x] No `unsafe` in core library except the intentional FFI boundary in
      `plugin.rs`, which is documented
- [x] `argon2` 0.5 `hash_raw` API used (no deprecated `hash_password_into`)
- [x] `aes-gcm` with `getrandom` feature enabled for OS-seeded nonces
- [x] `lzma-rs` in pure-Rust safe mode (no C FFI in this release)

### CI / dependency level

- [ ] `cargo audit` for known CVEs in dependencies (planned for CI)
- [ ] `cargo deny` for license compatibility checking (planned)
- [ ] Fuzzing harness for `BlockHeader::read` and `Superblock::read` (planned)

---

## Known Limitations

### LZMA decompression speed

The `lzma-rs` 0.3 decompressor is 5–6× slower than 7-Zip's hand-optimized C++
implementation on the same algorithm. This is a performance limitation, not a
security issue. It does not affect correctness or integrity.

A future release will evaluate an optional `liblzma` FFI backend.

### Plugin trust

Codec plugins loaded via `plugin_abi/sixcy_plugin.h` execute arbitrary native
code within the host process. Loading an untrusted plugin is equivalent to
executing untrusted code. No sandboxing is provided in this release.

### No forward secrecy

The same Argon2id-derived key is used for all blocks in an archive. Compromise
of the archive password compromises all blocks retroactively. Per-block
ephemeral keys are not implemented.

---

## Dependency Security

| Crate | Purpose | Notes |
|-------|---------|-------|
| `aes-gcm` 0.10 | AES-256-GCM | RustCrypto; audited |
| `argon2` 0.5 | Key derivation | RustCrypto; audited |
| `blake3` 1.5 | Content hashing | Official BLAKE3 implementation |
| `crc32fast` 1.3 | Header checksum | Hardware-accelerated CRC32 |
| `lzma-rs` 0.3 | LZMA codec | Pure Rust; no C FFI |
| `zstd` 0.13 | Zstd codec | Wraps zstd C library via FFI |
| `lz4_flex` 0.11 | LZ4 codec | Pure Rust |
| `brotli` 3.4 | Brotli codec | Pure Rust |

Run `cargo audit` regularly to check for newly disclosed CVEs in these
dependencies.

---

## Changelog

| Version | Security changes |
|---------|-----------------|
| v1.0.0 | No new cryptographic primitives. `scan()` classifies every block by header CRC32 integrity, codec UUID availability, and payload length; blocks that fail any check are excluded from the reconstructed index. `extract_recoverable()` additionally verifies payload BLAKE3 via `decode_block` before writing recovered data, so decompression failures are also silently skipped. |
| v0.3.0 | Added mandatory `header_crc32` to every block header; added superblock CRC32; codec UUID mismatch now fails at open time (not at block decode time); all integrity checks made non-optional |
| v0.2.0 | Added AES-256-GCM block encryption; Argon2id key derivation; BLAKE3 content hash verification |
| v0.1.0 | Initial release |

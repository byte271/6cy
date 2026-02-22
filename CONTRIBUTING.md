# Contributing to sixcy / .6cy

Thank you for your interest in contributing. This document covers the process,
standards, and expectations for contributions to both the reference
implementation and the format specification.

---

## Table of Contents

1. [What can I contribute?](#1-what-can-i-contribute)
2. [Before you start](#2-before-you-start)
3. [Development setup](#3-development-setup)
4. [Coding standards](#4-coding-standards)
5. [Commit messages](#5-commit-messages)
6. [Pull request process](#6-pull-request-process)
7. [Specification changes](#7-specification-changes)
8. [Adding a codec](#8-adding-a-codec)
9. [Reporting bugs](#9-reporting-bugs)
10. [Security vulnerabilities](#10-security-vulnerabilities)
11. [License agreement](#11-license-agreement)

---

## 1. What can I contribute?

**Code contributions (Apache-2.0)**

- Bug fixes in the reader, writer, CLI, or crypto module
- Performance improvements (especially LZMA decompression throughput)
- New built-in codec integrations with a frozen UUID assigned
- Test coverage additions (unit tests, integration tests, property-based tests)
- Platform-specific build fixes (Windows, macOS, Linux, WASM)
- FFI bindings (Python, C, Go, …)

**Specification contributions (CC BY 4.0)**

- Clarifications and editorial improvements to `spec.md`
- Formal grammar or pseudo-code additions
- Translations of `spec.md` (must retain CC BY 4.0 header)

**Documentation and benchmarks**

- Improvements to `BENCHMARK.md`, `README.md`, `CHANGELOG.md`
- Additional benchmark scenarios (real-world data, multi-file archives, …)
- Corrections to measured numbers (must include raw data + methodology)

---

## 2. Before you start

- Open a GitHub issue before beginning any non-trivial change. This avoids
  duplicate work and lets design questions be settled before code is written.
- For specification changes, open an issue tagged `spec` and allow at least
  one week for discussion before opening a pull request.
- For new codec UUIDs, a UUID must be assigned in the issue before any
  implementation begins. Once assigned, a UUID is permanent.

---

## 3. Development setup

```bash
# Clone
git clone https://github.com/cyh/sixcy.git
cd sixcy

# Build (debug)
cargo build

# Build (release)
cargo build --release

# Run all tests
cargo test

# Run integration tests only
cargo test --test integration_test

# Run benchmarks
cargo bench

# Check formatting
cargo fmt --check

# Run linter
cargo clippy -- -D warnings
```

### Minimum Rust version

Rust stable 1.70 or later. We do not use nightly features.

---

## 4. Coding standards

### Rust style

- Run `cargo fmt` before every commit. The project uses the default `rustfmt`
  configuration (no `rustfmt.toml` overrides).
- Run `cargo clippy -- -D warnings`. All warnings are treated as errors in CI.
- No `unsafe` code in the core library (`src/`) without a documented safety
  invariant and a comment explaining why the safe alternative is insufficient.
  The `plugin.rs` FFI wrapper is the sole intentional `unsafe` boundary.

### Error handling

- Use `thiserror` for library error types. No `unwrap()` or `expect()` in
  library code paths that can be reached from user input.
- Use `?` for propagation. Match and convert at module boundaries.

### I/O

- All numeric writes use explicit `.to_le_bytes()` — never `byteorder` write
  traits that could silently change byte order.
- All numeric reads use explicit `u32::from_le_bytes(buf[n..n+4].try_into())`.

### Tests

- Every bug fix must include a regression test that would have caught the bug.
- New public API functions must have at least one unit test and, where
  meaningful, a property-based test using `proptest`.

### Documentation

- All public API items (`pub fn`, `pub struct`, `pub enum`, `pub const`) must
  have `///` doc comments.
- Internal implementation details go in `//` comments or module-level `//!`
  docs.

---

## 5. Commit messages

Follow the [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/)
format:

```
<type>(<scope>): <summary>

[optional body]

[optional footer: BREAKING CHANGE / Fixes #N]
```

**Types:** `feat`, `fix`, `perf`, `refactor`, `test`, `docs`, `chore`, `ci`

**Scope** (optional): `block`, `superblock`, `codec`, `crypto`, `index`,
`io_stream`, `archive`, `plugin`, `cli`, `spec`, `bench`

**Examples:**

```
feat(codec): add Brotli codec with frozen UUID 9c1e5f3a-...
fix(io_stream): read_at now spans chunk boundaries correctly
perf(block): eliminate intermediate Vec allocation in decode_block
docs(spec): clarify solid block intra_offset semantics
BREAKING CHANGE: format_version bumped to 3; v2 archives rejected
```

---

## 6. Pull request process

1. Fork the repository and create a branch from `main`:
   ```bash
   git checkout -b fix/my-fix
   ```

2. Make your changes. Ensure:
   - `cargo fmt --check` passes
   - `cargo clippy -- -D warnings` passes
   - `cargo test` passes

3. Update `CHANGELOG.md` under the `[Unreleased]` section.

4. Open a pull request against `main`. Fill in the PR template completely.

5. A maintainer will review within 7 days. Expect at least one round of
   feedback. Please respond to review comments within 14 days or the PR may
   be closed.

6. PRs are merged via **squash merge** to keep the main branch history linear.

---

## 7. Specification changes

Changes to `spec.md` that affect the binary format require a `format_version`
bump and MUST:

- Be discussed in a GitHub issue for at least one week before implementation.
- Include concrete justification for why the change cannot be expressed within
  the current format version.
- Update the version history table in `spec.md §14`.
- Update `CHANGELOG.md` with a clear "Format changes" subsection.
- Include a migration note explaining what happens to existing archives.

Editorial changes (grammar, examples, clarifications that do not affect
byte-level behavior) can be submitted directly as PRs.

---

## 8. Adding a codec

1. Open an issue requesting a UUID assignment. Provide: codec name, crate name
   and version, a brief description, and why it belongs in the built-in registry
   rather than as a plugin.

2. Once a UUID is assigned in the issue, the UUID is permanent. It will be
   added to the frozen registry table in `spec.md §7.1` even if the codec is
   later deprecated.

3. Implement the `Codec` trait in `src/codec/mod.rs`. Add the UUID constant and
   update `CodecId::from_uuid()`, `CodecId::uuid()`, `CodecId::name()`, and
   `CodecId::from_name()`.

4. Register in `get_codec()` and `get_codec_by_uuid()`.

5. Add to the CLI `parse_codec()` function in `src/main.rs`.

6. Add to the `required_codecs` table in `spec.md §7.1`.

7. Add unit tests covering compress → decompress round-trip and an empty-input
   edge case.

---

## 9. Reporting bugs

Open a GitHub issue with:

- **Title:** one-line description of the bug
- **Version:** `6cy --version` output
- **OS:** operating system and version
- **Steps to reproduce:** minimal commands or code
- **Expected behavior:** what you expected
- **Actual behavior:** what happened (include full error output)
- **Archive file:** if the bug involves a specific archive, attach or link it
  (remove sensitive data first)

For crashes or panics, include the full stack trace (`RUST_BACKTRACE=1`).

---

## 10. Security vulnerabilities

**Do not open a public GitHub issue for security vulnerabilities.**

See [`SECURITY.md`](SECURITY.md) for the responsible disclosure process.

---

## 11. License agreement

By submitting a pull request, you confirm that:

- You have the right to submit the contribution under the Apache License 2.0
  (for code) or CC BY 4.0 (for specification changes).
- You grant the project maintainer a perpetual, worldwide, non-exclusive,
  no-charge, royalty-free, irrevocable license to reproduce, distribute,
  and sublicense your contribution as part of this project.
- Contributions to `spec.md` are submitted under CC BY 4.0.

No Contributor License Agreement (CLA) signature is required beyond the act
of submitting a pull request.

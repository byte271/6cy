# Environmental Preparation

This document lists all environment requirements and step-by-step setup instructions to build and run **6cy** from source on Windows, Linux, macOS and lightweight environments (WSL / iSH / Alpine). Follow the section for your platform.

> Note: the commands below assume you have a working network connection and permission to install packages on the machine.

---

## Quick checklist (minimum)
- Rust toolchain (`rustup`, `rustc`, `cargo`). (https://rust-lang.org/tools/install/)
- Native C/C++ build tools / linker for your platform (MSVC on Windows, `build-essential`/`clang` on Linux, Xcode Command Line Tools on macOS). (https://learn.microsoft.com/en-us/cpp/build/building-on-the-command-line?view=msvc-170)
- pkg-config and development headers for any native deps (e.g. OpenSSL) if your crate dependencies require them.

---

## Verify after setup (always)
Run these to confirm:

```bash
rustc --version
cargo --version
```

Expected: both print a version string.

---

## 1) Install Rust (all platforms)

Recommended: use `rustup` (official installer). This installs `rustc`, `cargo`, and `rustup` (toolchain manager).

```bash
# Unix-like (Linux, macOS, WSL)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
# then follow prompts, or use -y to auto-install
```

Windows users can download and run the `rustup-init.exe` from the official site.

After install, open a new shell and verify:

```bash
rustc --version
cargo --version
```

Optional: install useful components:

```bash
rustup component add clippy rustfmt
cargo install cargo-edit
```

---

## 2) Windows (native) required for MSVC toolchain

### Required: Visual Studio Build Tools (Desktop development with C++)
Install **Visual Studio Build Tools** or full Visual Studio with the **Desktop development with C++** workload. This provides `link.exe`, the MSVC runtime and SDKs required by the default Rust MSVC toolchain. If `link.exe` is missing you will get linker errors.

Options:
- Download Visual Studio Installer choose **Build Tools for Visual Studio**enable **Desktop development with C++** (MSVC, Windows SDK).
- Or install full Visual Studio Community and add the same workload.

Verify:

```powershell
where link
```

If `link.exe` is found and `rustc --version` works, you're ready.

**Notes**
- Use the *MSVC* toolchain on Windows: `rustup default stable-x86_64-pc-windows-msvc`.
- If you want GNU toolchain (MinGW), install MinGW-w64 and use `*-pc-windows-gnu` targets but MSVC is recommended for most Rust on Windows.

---

## 3) WSL (Windows Subsystem for Linux)

If you prefer Linux tooling on Windows, install WSL (Ubuntu recommended), then follow the Linux instructions below inside WSL. `rustup` works inside WSL the same as on native Linux.

---

## 4) Linux (Debian / Ubuntu / derivatives)

Install system build tools and common dev packages:

```bash
sudo apt update
sudo apt install -y build-essential pkg-config curl ca-certificates git
# If your project depends on OpenSSL (many crates do):
sudo apt install -y libssl-dev
# For clang/LLVM if needed:
sudo apt install -y clang llvm
```

`build-essential` provides `gcc`, `make`, `ld` and basic toolchain.

On Fedora:

```bash
sudo dnf groupinstall -y "Development Tools"
sudo dnf install -y pkgconfig openssl-devel
```

On Arch Linux:

```bash
sudo pacman -Syu --needed base-devel pkgconf openssl
```

After system deps are installed, run `rustup` to install Rust and then `cargo build --release`.

---

## 5) macOS

Install Xcode Command Line Tools:

```bash
xcode-select --install
```

This installs `clang`, `make`, and other essential developer tools. After that, install `rustup` and proceed as above.

If your crates require OpenSSL:

```bash
brew install openssl@3 pkg-config
# sometimes you need to set:
export PKG_CONFIG_PATH="$(brew --prefix openssl@3)/lib/pkgconfig"
```

---

## 6) Alpine / iSH (lightweight) NOTES & caveats

- iSH is an emulated environment and may not be suitable for heavy native builds (Rust compilation is often slow and may fail due to missing glibc vs musl differences).
- On Alpine, install build tools:

```sh
apk update
apk add build-base curl git pkgconfig openssl-dev ca-certificates
```

- You may prefer cross-compiling on a normal Linux / Windows host and only run lightweight tests on iSH.

---

## 7) Common native libraries & pkg-config

Many Rust crates link to system C libraries (OpenSSL, libz, zstd, lz4). Install relevant dev packages:

- Debian/Ubuntu:
  ```bash
  sudo apt install -y libssl-dev zlib1g-dev liblz4-dev libzstd-dev pkg-config
  ```
- Fedora:
  ```bash
  sudo dnf install -y openssl-devel zlib-devel lz4-devel zstd-devel pkgconfig
  ```
- Arch:
  ```bash
  sudo pacman -S --needed openssl zlib lz4 zstd pkgconf
  ```

If `pkg-config` cannot find a library, set `PKG_CONFIG_PATH` to the correct `pkgconfig` directory.

---

## 8) Cross-compilation & extra Rust targets

If you need to build for other targets (aarch64, windows-msvc/aarch64), install targets via `rustup`:

```bash
# example: add windows aarch64 msvc target
rustup target add aarch64-pc-windows-msvc

# add musl target for static Linux binaries
rustup target add x86_64-unknown-linux-musl
```

Cross-building may require additional linker toolchains (e.g., `musl` toolchain, mingw, or Windows SDK for MSVC targets).

---

## 9) Useful developer tools (optional but recommended)

```bash
# security and license checks
cargo install cargo-audit
cargo install cargo-deny
cargo install cargo-license

# code quality
rustup component add clippy rustfmt
```

`cargo-audit` helps detect vulnerable dependencies.

---

## 10) Build & run (project steps)

From project root:

```bash
# debug build
cargo build

# release build
cargo build --release

```

If you prefer to place binary in PATH:

```bash
# install into cargo bin (user local)
cargo install --path .
# then run:
6cy --help
```

---

## 11) Troubleshooting (common errors)

- `link.exe not found` Install Visual Studio Build Tools with Desktop C++ workload on Windows.
- `pkg-config: command not found` or `could not find OpenSSL` Install `pkg-config` and `libssl-dev` (or `openssl` via Homebrew on macOS).
- SSL/CA issues on Alpine or minimal systems `ca-certificates` package required.

---

## 12) Security & hygiene

- Do **not** embed tokens or secrets in your git history. Revoke any tokens accidentally exposed immediately.  
- Prefer running `cargo-audit` before publishing artifacts.  
- Use a clean build environment to reproduce CI builds.

---

## 13) CI / reproducible builds (recommended)

On GitHub Actions, CI steps should include:
- Checkout
- Install required system packages (Ubuntu images have `build-essential`; macOS needs `xcode-select --install` done via image)
- Install Rust via `rust-toolchain` file / `rustup`
- `cargo build --release`
- `cargo test`
- `cargo audit` (security)

---

## 14) Example: full install script (Ubuntu/Debian)

```bash
sudo apt update
sudo apt install -y build-essential pkg-config curl ca-certificates git libssl-dev zlib1g-dev
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source $HOME/.cargo/env
rustc --version
cargo --version
cargo build --release
```

---

## References & official docs
- Rust installer / rustup: official instructions. (https://rust-lang.org/tools/install/)
- Visual Studio / MSVC Build Tools: install Desktop development with C++ workload. (https://learn.microsoft.com/en-us/cpp/build/building-on-the-command-line?view=msvc-170)
- Xcode Command Line Tools: official Apple docs. (https://developer.apple.com/documentation/xcode/installing-the-command-line-tools/)
- OpenSSL detection and pkg-config notes for Rust crates.

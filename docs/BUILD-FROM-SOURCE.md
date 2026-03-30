# Building 4DA from Source

This guide walks you through compiling 4DA from source code. Building from source is the strongest way to verify that the application does exactly what the source code says -- no hidden modifications, no tampered binaries.

---

## Why Build from Source

Most software asks you to download a pre-built binary and trust the publisher. 4DA is source-available precisely so you don't have to. By building from source, you verify:

- The binary you run matches the public source code
- No code was added between the source repository and the distributed installer
- All dependencies are the versions declared in the lock files

If you just want to verify a downloaded binary, see [VERIFY-DOWNLOADS.md](VERIFY-DOWNLOADS.md). This guide is for building the entire application yourself.

---

## Prerequisites

### All Platforms

| Tool | Version | Purpose |
|------|---------|---------|
| [Rust](https://rustup.rs/) | 1.93+ (pinned in `rust-toolchain.toml`) | Backend compilation |
| [Node.js](https://nodejs.org/) | 20 LTS or later | Frontend tooling |
| [pnpm](https://pnpm.io/) | 9.15.0 (pinned in `package.json`) | Package manager |
| [Git](https://git-scm.com/) | Any recent version | Source code checkout |

After installing Rust via rustup, the correct toolchain version is selected automatically from `src-tauri/rust-toolchain.toml`.

Install pnpm after Node.js:

```bash
corepack enable
corepack prepare pnpm@9.15.0 --activate
```

### Windows

- **Visual Studio Build Tools 2022** with the "Desktop development with C++" workload
- WebView2 is bundled automatically during the build (via `embedBootstrapper` in `tauri.conf.json`)

### macOS

- **Xcode Command Line Tools**: `xcode-select --install`
- Minimum deployment target: macOS 10.15 (Catalina)
- For code signing (optional): Apple Developer ID certificate

### Linux (Debian/Ubuntu)

```bash
sudo apt update
sudo apt install -y \
  libwebkit2gtk-4.1-dev \
  libgtk-3-dev \
  libappindicator3-dev \
  librsvg2-dev \
  libssl-dev \
  pkg-config \
  build-essential \
  curl \
  wget \
  file
```

### Linux (Fedora/RHEL)

```bash
sudo dnf install -y \
  webkit2gtk4.1-devel \
  gtk3-devel \
  libappindicator-gtk3-devel \
  librsvg2-devel \
  openssl-devel \
  pkg-config \
  gcc \
  gcc-c++
```

---

## Quick Start

Five commands from zero to a running build:

```bash
# 1. Clone the repository
git clone https://github.com/runyourempire/4DA.git
cd 4DA

# 2. Checkout a specific release (recommended)
git checkout v1.0.0

# 3. Install frontend dependencies
pnpm install

# 4. Build the production application
pnpm run tauri build

# 5. Find your installer
# Windows: src-tauri/target/release/bundle/nsis/
# macOS:   src-tauri/target/release/bundle/dmg/
# Linux:   src-tauri/target/release/bundle/appimage/
```

---

## Detailed Build Steps

### 1. Clone and Checkout

```bash
git clone https://github.com/runyourempire/4DA.git
cd 4DA
```

To build a specific release (recommended for verification):

```bash
git checkout v1.0.0
```

To build the latest development code:

```bash
git checkout main
```

### 2. Verify Dependency Lock Files

The repository includes pinned dependency files that ensure reproducible builds:

- `Cargo.lock` -- pins every Rust crate to an exact version
- `pnpm-lock.yaml` -- pins every npm package to an exact version

Verify these files are present and unmodified:

```bash
git status
```

Both files should show no modifications.

### 3. Install Frontend Dependencies

```bash
pnpm install
```

This installs all JavaScript/TypeScript dependencies from `pnpm-lock.yaml`. The `--frozen-lockfile` flag is implied by CI but optional for local builds.

### 4. Build the Frontend

```bash
pnpm run build
```

This runs TypeScript compilation (`tsc`) and Vite bundling. Output goes to `dist/`.

### 5. Build the Rust Backend + Bundle

```bash
pnpm run tauri build
```

This command:
1. Builds the frontend (if not already built)
2. Compiles the Rust backend in release mode
3. Bundles the application into platform-specific installers

Build artifacts are located in `src-tauri/target/release/bundle/`:

| Platform | Format | Location |
|----------|--------|----------|
| Windows | NSIS installer | `nsis/4DA_1.0.0_x64-setup.exe` |
| macOS | DMG | `dmg/4DA_1.0.0_x64.dmg` |
| Linux | AppImage | `appimage/4DA_1.0.0_amd64.AppImage` |
| Linux | DEB | `deb/4da_1.0.0_amd64.deb` |
| Linux | RPM | `rpm/4da-1.0.0-1.x86_64.rpm` |

### 6. Development Mode (Optional)

To run the application in development mode with hot-reloading:

```bash
pnpm run tauri dev
```

The frontend dev server starts on `http://localhost:4444`. The Rust backend compiles and launches with the dev frontend.

---

## Running Tests

4DA has comprehensive test suites for both the Rust backend and the React frontend.

### Rust Tests (2,215 tests)

```bash
cd src-tauri
cargo test
```

If the dev server is running and locking the binary:

```bash
cargo test --lib
```

### Frontend Tests (1,173 tests)

```bash
pnpm run test
```

### Full Validation Suite

```bash
pnpm run validate
```

This runs file size checks, GAME component validation, command validation, ESLint, TypeScript type checking, frontend tests, and the production build.

### Rust Validation

```bash
pnpm run validate:rust
```

Runs `cargo fmt --check`, `cargo clippy -- -D warnings`, and `cargo test`.

---

## Build Verification

### Compare Source Hash to Release Tag

Each release tag corresponds to a specific commit. Verify you're building the same code:

```bash
git log --oneline -1
# Should match the commit hash listed in the GitHub release
```

### Verify No Unexpected Dependencies

```bash
# Rust dependency tree
cd src-tauri
cargo tree

# Frontend dependency tree
cd ..
pnpm list --depth=0
```

Review the dependency list for anything unexpected. 4DA uses pure Rust implementations for critical functionality:
- **OCR:** `ocrs` crate (no C bindings)
- **PDF:** `pdf-extract` + `lopdf` (pure Rust)
- **Office:** `docx-rs` + `calamine` (pure Rust)

### Verify Network Behaviour

After building, you can monitor the application's network activity:
- **Windows:** Use Wireshark or Fiddler
- **macOS:** Use Little Snitch or Wireshark
- **Linux:** Use Wireshark or `tcpdump`

See [NETWORK-TRANSPARENCY.md](NETWORK-TRANSPARENCY.md) for the complete list of expected connections.

---

## Build Configuration

### `src-tauri/tauri.conf.json`

Key settings:
- **CSP** (line 35): Content Security Policy restricting allowed connections
- **Updater** (lines 44-49): Minisign public key and update endpoint
- **Bundle targets** (line 52): `nsis`, `dmg`, `appimage`, `deb`, `rpm`
- **Publisher**: 4DA Systems Pty Ltd

### `src-tauri/rust-toolchain.toml`

Pins the Rust compiler to channel `1.93`. Rustup handles this automatically when you build inside the `src-tauri/` directory.

### `src-tauri/Cargo.toml`

Rust dependencies and build configuration. The `Cargo.lock` file pins exact versions.

---

## Troubleshooting

### `error: linker 'link.exe' not found` (Windows)

Install Visual Studio Build Tools 2022 with the "Desktop development with C++" workload.

### `pkg-config` errors (Linux)

Install the development libraries listed in the Linux prerequisites section above.

### `error[E0554]: #![feature] may not be used on the stable release channel`

Your Rust toolchain is too old. Run `rustup update` to get the version specified in `rust-toolchain.toml`.

### `pnpm: command not found`

Enable corepack: `corepack enable && corepack prepare pnpm@9.15.0 --activate`

### Port 4444 already in use (dev mode)

The dev server uses port 4444. Kill any existing process on that port, or the `kill-port.cjs` script handles this automatically.

### Build is slow

First Rust compilation downloads and compiles all crates. Subsequent builds use the incremental cache. A full release build typically takes 5-15 minutes depending on hardware.

---

## Cross-Compilation Notes

Tauri supports cross-compilation with limitations:

- **Windows to Linux/macOS:** Not natively supported. Use CI (GitHub Actions) or a VM.
- **macOS to Linux:** Not natively supported. Use CI or Docker.
- **Linux to Windows:** Possible with `cross` but not officially supported by Tauri.

For multi-platform builds, the recommended approach is GitHub Actions CI with platform-specific runners.

---

## Related Documents

- [VERIFY-DOWNLOADS.md](VERIFY-DOWNLOADS.md) -- Verify pre-built binaries
- [NETWORK-TRANSPARENCY.md](NETWORK-TRANSPARENCY.md) -- Every outbound connection
- [SECURITY-AUDIT-GUIDE.md](SECURITY-AUDIT-GUIDE.md) -- Audit trust-critical code paths
- [TRUST-ARCHITECTURE.md](TRUST-ARCHITECTURE.md) -- Why local-first means trust-optional

---

4DA Systems Pty Ltd (ACN 696 078 841) | FSL-1.1-Apache-2.0

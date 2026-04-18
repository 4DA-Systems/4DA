# Contributing to 4DA

4DA is open for contributions. The easiest entry points are source adapters and bug fixes.

## Quick Start

```bash
# Prerequisites:
#   - Rust 1.93+ (rustup will auto-pin from src-tauri/rust-toolchain.toml)
#   - Node.js 20 LTS (.nvmrc provided)
#   - pnpm 9.15.0 (pinned in package.json packageManager)
# Platform build tools:
#   - Windows: Visual Studio Build Tools 2022 + "Desktop development with C++"
#   - macOS:   xcode-select --install
#   - Linux:   see docs/BUILD-FROM-SOURCE.md for the apt/dnf list
git clone https://github.com/runyourempire/4DA.git
cd 4DA
pnpm install
pnpm tauri dev
```

Full build guide with troubleshooting: **[docs/BUILD-FROM-SOURCE.md](docs/BUILD-FROM-SOURCE.md)**
First-run app setup (API keys, context dirs): **[docs/GETTING_STARTED.md](docs/GETTING_STARTED.md)**

## Development Commands

```bash
pnpm tauri dev              # Dev server (localhost:4444)
cargo test                  # Rust tests (from src-tauri/)
pnpm test                   # Frontend tests
pnpm run validate:all       # Full validation (required before PR)
pnpm run validate:sizes     # File size limits check
```

## Architecture Overview

```
src-tauri/src/
  lib.rs              # App entry, plugin setup, startup
  commands.rs          # Tauri command handlers
  analysis.rs          # Core analysis pipeline
  scoring/             # 5-axis scoring engine
  sources/             # Source adapters (one file each)
  ace/                 # Active Context Engine (codebase scanning)
  domain_profile.rs    # Developer tech identity
  content_quality.rs   # Clickbait/quality filtering
  novelty.rs           # Intro content detection
  monitoring.rs        # Background scheduler + notifications
  db.rs                # SQLite + sqlite-vec operations

src/
  App.tsx              # Main app shell
  components/          # React components
  store/               # Zustand store (11 slices)
  hooks/               # Custom hooks
  config/sources.ts    # Source registry (labels, colors)

mcp-4da-server/        # MCP server (MIT licensed, npm publishable)
```

## Contributing a Source Adapter

Source adapters are the easiest contribution. Each is a single Rust file in `src-tauri/src/sources/`.

### Steps

1. Copy an existing adapter (e.g., `lobsters.rs`) as your template
2. Implement the `Source` trait: `name()`, `fetch()`, `source_type()`
3. Register it in `src-tauri/src/sources/mod.rs`
4. Add frontend metadata in `src/config/sources.ts` (label, color, full name)
5. Add the source ID to the `ALL_SOURCE_IDS` array
6. Write tests for the parser (mock the HTTP response)

### Source Trait

```rust
#[async_trait]
pub trait Source: Send + Sync {
    fn name(&self) -> &str;
    fn source_type(&self) -> &str;
    async fn fetch(&self, client: &reqwest::Client) -> Result<Vec<GenericSourceItem>>;
}
```

## File Size Limits

New source files must stay within limits:
- **TypeScript/TSX**: 350 lines (warn), 500 lines (error)
- **Rust**: 800 lines (warn), 1500 lines (error)

If your file exceeds limits, split it. Run `pnpm run validate:sizes` to check.

## Code Style

- **Rust**: `cargo fmt` + `cargo clippy -- -D warnings`
- **TypeScript**: ESLint config in repo. `pnpm run lint`
- **Imports**: Follow the ordering in CLAUDE.md (framework > external > internal > relative > types)

## PR Process

1. Fork and create a branch from `main`
2. Make your changes
3. Run `pnpm run validate:all` — all checks must pass
4. Submit a PR using the template
5. Address review feedback

## CLA

By submitting a PR, you agree to the [Contributor License Agreement](CLA.md).

## Questions?

Open a [Discussion](https://github.com/runyourempire/4DA/discussions) on GitHub.

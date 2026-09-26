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
git clone https://github.com/4DA-Systems/4DA.git
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

mcp-4da-server/        # MCP server (Apache-2.0 licensed, npm publishable)
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
- **TypeScript (`.ts`)**: 300 lines (warn), 500 lines (error)
- **TSX**: 350 lines (warn), 500 lines (error)
- **Rust**: 700 lines (warn), 1000 lines (error)
- **Test files** (`*.test.*`, `*_tests.rs`): no warnings, error at 2x the normal limit

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

### How a PR reaches `main`

`main` accepts changes only through pull requests and the **merge queue**. There
are no direct pushes, no force pushes, and no bypass actors, including for
maintainers.

- **Squash only.** The PR title becomes the commit subject on `main`, and the PR
  body becomes the commit message, word for word. Use a
  [Conventional Commit](https://www.conventionalcommits.org/) title
  (`fix(scoring): …`, `feat(mcp): …`, `chore(deps): …`) and write the body for
  someone reading `git log` later.
- **Required checks.** `Validate Success` is the only required check. On the PR
  it runs the fast checks: frontend lint, types and tests, the MCP server, the
  relay, repo-wide guards, a scan of PR metadata, and for Rust, `fmt`, clippy on
  every feature set, `cargo audit` and `cargo deny`. Path filters skip legs a PR
  does not touch.
- **Rust tests run in the merge queue.** The full Rust suite (compile, unit and
  integration tests on every feature set, plus the real-embedding calibration)
  runs once, when the PR enters the queue. It runs with no path filters, against
  the latest `main` plus everything ahead of it in the queue. A failing Rust test
  therefore shows up as a queue ejection, not a red PR, so run `cargo test` in
  `src-tauri/` before you push. A PR can also be ejected if `main` has moved or
  rotted. Either way, read the queue run before re-queueing. A test that fails
  once and passes on retry is reported as `FLAKY` in that run.
- **After the merge.** Every push to `main` also runs a cold, cacheless
  fresh-clone build and test on Linux and Windows (`Hermetic Fresh-Clone`) and a
  CodeQL scan. They do not block merges. When one goes red on `main` it opens an
  issue (`hermetic-main` or `codeql-main`), which closes itself when `main` is
  green again.
- **External contributors:** workflows on fork PRs start only after a
  maintainer approves them.

Security reports go through [SECURITY.md](SECURITY.md), not public issues.

## CLA

By submitting a PR, you agree to the [Contributor License Agreement](CLA.md).

## Questions?

Open a [Discussion](https://github.com/4DA-Systems/4DA/discussions) on GitHub.

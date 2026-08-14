# 4DA — Conventions

> **Generated from `CLAUDE.md`** — the maintained source of truth for this repo.
> Regenerate this file whenever `CLAUDE.md` changes; do not edit it in isolation or it will
> drift back out of sync. Last synced: 2026-08-14.

## What Is 4DA

**4DA reads the internet for developers — privately, locally. Your codebase decides what's relevant.**

4DA (4 Dimensional Autonomy) is the Tauri 2.0 desktop app that delivers on that description. It scores
every item against your actual stack — your manifests, your dependencies, your recent commits. When the
engine improves it re-judges everything it already holds: yesterday's noise becomes tomorrow's signal.

<!-- retired-ok: this notice quotes the banned phrases in order to ban them -->
**Retired (AD-030):** "gets sharper every day" and "it learns from how you engage with what it shows
you." Do not reintroduce these claims on any surface — enforced by `scripts/check-retired-claims.cjs`.
Improvement is attributed to engine updates and the user's codebase, never to engagement.

It is **not** a content tool or a news reader — it is proactive developer intelligence.

**Stack:** Rust backend + React/TypeScript frontend + SQLite (with sqlite-vec for vector search)
**Dev server:** localhost:4444 | **Package manager:** pnpm

## Commands

```bash
pnpm run tauri dev         # Dev server (frontend + Rust backend)
pnpm run tauri build       # Production build
cargo test                 # Rust tests (run from src-tauri/)
pnpm run test              # Frontend tests (Vitest)
pnpm run validate:all      # Full validation suite
pnpm run validate:sizes    # Check file size limits
```

## Principles

1. **Privacy first** — raw data never leaves the machine
2. **BYOK** — user provides API keys, never stored remotely
3. **Local first** — works offline with Ollama fallback
4. **Minimal** — no feature bloat, every element earns its place
5. **Accurate first** — never show intelligence the system can't stand behind. Correct results from
   a capable model beat fast results from a weak one. If the model can't do the job, don't fake it.

## Architecture

```
src/                    # React frontend (TypeScript)
  components/           # UI components (200+ files)
  types/                # Shared TypeScript types
src-tauri/              # Rust backend
  src/                  # Core logic (300+ modules)
    ace/                # Autonomous Context Engine (project scanner)
    db/                 # SQLite + sqlite-vec database layer
    extractors/         # File format extractors (PDF, Office, etc.)
    scoring/            # PASIFA scoring algorithm (multi-module)
    settings/           # Settings management + keychain + validation
    sources/            # 20+ content source adapters (HN, Reddit, RSS, GitHub, arXiv,
                        #   dev.to, Lobsters, ProductHunt, Bluesky, crates.io, npm, PyPI,
                        #   HuggingFace, PapersWithCode, CVE/OSV, StackOverflow, X/Twitter,
                        #   YouTube, Go modules, Mastodon, Lemmy)
  src/embeddings.rs     # Local embedding via Ollama
data/                   # Runtime data (gitignored)
  settings.json         # User config (use settings.example.json as template)
  4da.db                # SQLite database
mcp-memory-server/      # MCP server for persistent dev memory
mcp-4da-server/         # MCP server exposing 4DA tools (14 tools; 9 standalone)
```

**Path accuracy note:** `db` and `scoring` are **directories**, not single files. There is no
`src-tauri/src/db.rs` and no `src-tauri/src/relevance.rs` — the relevance/PASIFA logic lives in
`src-tauri/src/scoring/`. Do not reference those retired paths.

## Code Conventions

### Import Order
- **TypeScript:** React/framework > External packages > Internal (`@/`) > Relative > Types
- **Rust:** std > External crates > `crate::` > `super::`

### File Size Limits

Enforced by `scripts/check-file-sizes.cjs` (`pnpm run validate:sizes`):

| Kind                | Warn | Error |
|---------------------|------|-------|
| TypeScript (`.ts`)  | 300  | 500   |
| TypeScript (`.tsx`) | 350  | 500   |
| Rust (`.rs`)        | 700  | 1000  |

- Test files (`*.test.*`, `*_tests.rs`) are exempt from warnings and error at **2x** the normal
  error threshold.
- Rust functions: max 60 lines (convention; not gate-enforced — clippy's `too_many_lines` is set
  to `allow` in `src-tauri/Cargo.toml`).
- Exceeding files must be split or added to the exception list in `scripts/check-file-sizes.cjs`.

### Error Handling
- Rust: use `thiserror` for error types, `anyhow` for application errors
- TypeScript: explicit error boundaries for components, try/catch at API boundaries
- Never `unwrap()` or `panic!()` in production Rust code — use graceful fallbacks

### Naming
- Rust: snake_case for functions/variables, PascalCase for types/traits
- TypeScript: camelCase for functions/variables, PascalCase for components/types
- Files: kebab-case for TypeScript, snake_case for Rust

## Design System

Tokens are defined in `src/App.css`. The real CSS custom-property names carry a `--color-` prefix.

```css
/* Background */
--color-bg-primary: #0A0A0A;     --color-bg-secondary: #141414;   --color-bg-tertiary: #1F1F1F;
/* Text */
--color-text-primary: #FFFFFF;   --color-text-secondary: #A0A0A0; --color-text-muted: #8A8A8A;
/* Accent */
--color-accent-primary: #FFFFFF; --color-accent-gold: #D4AF37;    --color-border: #2A2A2A;
/* Status */
--color-success: #22C55E;        --color-error: #EF4444;
```

A **light theme** overrides the same token names with a separate palette (see `src/App.css`) —
never hard-code a hex value; always reference the token so both themes work.

Fonts: Inter (UI), JetBrains Mono (code) | Weights: 400, 500, 600

## Key Technical Gotchas

- **sqlite-vec KNN queries** require `k = ?` in the WHERE clause, NOT `LIMIT` at the end
- **MutexGuard<SourceRegistry>** is not Send — cannot hold across await points in Rust
- **OCR:** use the `ocrs` crate (pure Rust), not tesseract (requires C bindings)
- **PDF:** pdf-extract + lopdf. **Office:** docx-rs + calamine
- **ts-rs** v10 with serde-compat generates TypeScript types from Rust structs
- **Vite dep updates while `fourda.exe` runs** — the running process keeps stale module paths in
  memory and crashes on resolution. Fix: `taskkill /F /IM fourda.exe && pnpm install --frozen-lockfile`

## Reference Docs

Before modifying architecture or invariants, read the relevant `.ai/` file:
- `WISDOM.md` — the operating system for 4DA development (authority stack, principles, gates)
- `INVARIANTS.md` — non-negotiable system constraints
- `DECISIONS.md` — architectural decisions log (prevents re-litigation)
- `ARCHITECTURE.md` — system structure reference
- `FAILURE_MODES.md` — known fragile areas and previous regressions

## Never Commit

- `data/settings.json` — contains user API keys. Use `data/settings.example.json` as template.
- `data/*.db` — runtime databases
- `src-tauri/target/` — Rust build artifacts

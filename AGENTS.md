# 4DA — Codex Instructions

## What Is 4DA

4DA (4 Dimensional Autonomy) is a Tauri 2.0 desktop app that surfaces developer-relevant content from the internet — privately, locally, with zero configuration.

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
5. **Zero-config value** — a new user should see useful content within 60 seconds

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
    sources/            # Content source adapters (HN, Reddit, RSS, GitHub)
  src/embeddings.rs     # Local embedding via Ollama
data/                   # Runtime data (gitignored)
  settings.json         # User config (use settings.example.json as template)
  4da.db                # SQLite database
mcp-memory-server/      # MCP server for persistent dev memory (Codex)
mcp-4da-server/         # MCP server exposing 4DA tools (Codex)
```

## Code Conventions

### Import Order
- **TypeScript:** React/framework > External packages > Internal (`@/`) > Relative > Types
- **Rust:** std > External crates > `crate::` > `super::`

### File Size Limits
- TypeScript (.ts): warn at 300 lines, error at 500
- TypeScript (.tsx): warn at 300 lines, error at 450
- Rust: warn at 500 lines, error at 800
- Rust functions: max 60 lines (warning only)
- Exceeding files must be split or added to `scripts/check-file-sizes.cjs` exceptions

### Error Handling
- Rust: use `thiserror` for error types, `anyhow` for application errors
- TypeScript: explicit error boundaries for components, try/catch at API boundaries
- Never `unwrap()` or `panic!()` in production Rust code — use graceful fallbacks

### Naming
- Rust: snake_case for functions/variables, PascalCase for types/traits
- TypeScript: camelCase for functions/variables, PascalCase for components/types
- Files: kebab-case for TypeScript, snake_case for Rust

## Design System

```css
/* Background */
--bg-primary: #0A0A0A;    --bg-secondary: #141414;   --bg-tertiary: #1F1F1F;
/* Text */
--text-primary: #FFFFFF;   --text-secondary: #A0A0A0; --text-muted: #8A8A8A;
/* Accent */
--accent-primary: #FFFFFF; --accent-gold: #D4AF37;    --border: #2A2A2A;
/* Status */
--success: #22C55E;        --error: #EF4444;
```

Fonts: Inter (UI), JetBrains Mono (code) | Weights: 400, 500, 600

## Key Technical Gotchas

- **sqlite-vec KNN queries** require `k = ?` in WHERE clause, NOT `LIMIT` at end
- **MutexGuard<SourceRegistry>** is not Send — cannot hold across await points in Rust
- **OCR:** use `ocrs` crate (pure Rust), not tesseract (requires C bindings)
- **PDF:** pdf-extract + lopdf. **Office:** docx-rs + calamine
- **ts-rs** v10 with serde-compat generates TypeScript types from Rust structs

## Reference Docs

Before modifying architecture or invariants, read the relevant `.ai/` file:
- `WISDOM.md` — **the operating system** for 4DA development (authority stack, principles, gates, enforcement)
- `INVARIANTS.md` — non-negotiable system constraints
- `DECISIONS.md` — architectural decisions log (prevents re-litigation)
- `ARCHITECTURE.md` — system structure reference
- `FAILURE_MODES.md` — known fragile areas and previous regressions

## Never Commit

- `data/settings.json` — contains user API keys. Use `data/settings.example.json` as template.
- `data/*.db` — runtime databases
- `src-tauri/target/` — Rust build artifacts

## Codex-Specific

- Agent definitions: `.Codex/agents/` (4DA-specific agents for source debugging, trend analysis, etc.)
- Slash commands: `.Codex/commands/` (project-specific commands)
- MCP servers: memory (persistent decisions/learnings), 4da (33 intelligence tools)
- Hooks: wisdom gates (PreToolUse), consequence processing (UserPromptSubmit), session capture (Stop), prompt analyzer

## User Working Preferences

- Screenshot folder: `D:\lightshot`. If the user references a screenshot by name, look there first.
- `Screenshot_1914.png` captures the preferred development posture: be a rigorous senior auditor, bug finder, advisor, and plan generator.
- Audits and plans should be concrete, codebase-grounded, file-level, and implementation-oriented. Favor evidence, line references, targeted tests, and clear sequencing over abstract strategy.
- Fix Tier 0 correctness issues before adding architecture: duplicated or broken UI wiring, failing integration tests, vulnerable dependencies, suspect WASM/batch settings, and other real defects found in audit docs.
- Development strategy should follow the screenshot's priority stack: Tier 0 fixes, Tier 1 activate latent intelligence, Tier 2 proof layer, Tier 3 MCP production, Tier 4 cross-platform hardening, Tier 5 intelligence packs / preemption engine.
- It is acceptable to do objectively useful adjacent work when it is low-risk and directly improves the requested outcome, such as reading relevant `.ai/` docs, checking nearby tests, adding regression coverage, validating file sizes, or surfacing a better sequence. For high-risk, destructive, broad, or architectural changes, call out the recommendation before proceeding.

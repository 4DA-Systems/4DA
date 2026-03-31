# 4DA — Claude Code Instructions

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
mcp-memory-server/      # MCP server for persistent dev memory (Claude Code)
mcp-4da-server/         # MCP server exposing 4DA tools (Claude Code)
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

## AWE Integration (Artificial Wisdom Engine)

AWE is the judgment layer that transmutes intelligence into calibrated wisdom. It runs as an MCP server with 7 tools backed by a Rust engine.

**When to use AWE tools:**
- `awe_transmute` — Before any **high-stakes decision** (architecture changes, irreversible actions, new abstractions). Returns bias detection, consequence modeling, confidence calibration, and trade-off analysis.
- `awe_quick_check` — Fast sanity check on any decision. Use liberally — it's cheap.
- `awe_consequence_scan` — Before irreversible actions. Models 1st/2nd/3rd order consequences with reversibility scoring.
- `awe_feedback` — **After every decision outcome is known.** This is critical — AWE compounds by learning from outcomes. Feed it confirmed/refuted/partial results.
- `awe_recall` — At session start or before decisions in a domain. Retrieves accumulated principles, anti-patterns, and precedents.
- `awe_calibration` — Periodic check on AWE's judgment quality per domain.

**Automated integration (hooks handle this):**
- Session start: AWE wisdom (principles/anti-patterns) injected automatically
- Session start: Previous session's AWE decisions queued for feedback
- Session end: Recent AWE decisions captured in pending.json for next-session feedback
- Wisdom Gate 2: Destructive action warnings include `awe_consequence_scan` reminder

**Decision recording flow:**
1. Identify a significant decision → `awe_quick_check` (fast bias scan)
2. If high-stakes → `awe_transmute` (full pipeline, auto-persists to Wisdom Graph)
3. Also record in `remember_decision` (MCP memory) for session memory
4. When outcome is known → `awe_feedback` with decision_id

**Three decision stores — clear routing:**
| Store | Purpose | When to use |
|-------|---------|-------------|
| **AWE** (`awe_transmute`) | Judgment-augmented decisions with consequence modeling | Architectural choices, irreversible actions, design trade-offs |
| **MCP Memory** (`remember_decision`) | Dev session memory that survives context compaction | Learnings, gotchas, workflow decisions, code locations |
| **4DA Decision Memory** (`decision_memory`) | Tech stack tracking and alignment checking | Auto-inferred tech choices, `check_decision_alignment` queries |

Do NOT record the same decision in all three. Route by purpose. AWE is for decisions that need judgment. Memory is for session persistence. 4DA is for tech alignment.

**AWE binary:** `D:\runyourempire\awe\target\release\awe.exe`
**Wisdom database:** `%APPDATA%\awe\wisdom.db` (87 decisions, 1 validated principle, compounding)

## Claude-Specific

- Agent definitions: `.claude/agents/` (4DA-specific agents for source debugging, trend analysis, etc.)
- Slash commands: `.claude/commands/` (project-specific commands)
- MCP servers: memory (persistent decisions/learnings), 4da (33 intelligence tools), awe (7 wisdom tools)
- Hooks: wisdom gates (PreToolUse), consequence processing + AWE wisdom injection (UserPromptSubmit), session capture + AWE feedback tracking (Stop), prompt analyzer

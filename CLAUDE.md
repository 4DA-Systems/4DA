# 4DA — Claude Code Instructions

## What Is 4DA

**4DA reads the internet for developers — privately, locally — and gets sharper every day.**

That's the one-sentence description. Use it verbatim in marketing, onboarding, about dialogs, and any
surface where someone asks "what is 4DA?". The follow-up beat (held in reserve, used when asked
"how does it get sharper?") is: *It learns from how you engage with what it shows you. Yesterday's noise
becomes tomorrow's signal.*

4DA (4 Dimensional Autonomy) is the Tauri 2.0 desktop app that delivers on that promise.

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
5. **Accurate first** — never show intelligence the system can't stand behind. Correct results from a capable model beat fast results from a weak one. If the model can't do the job, don't fake it.

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
    sources/            # 20+ content source adapters (HN, Reddit, RSS, GitHub, arXiv, dev.to, Lobsters, ProductHunt, Bluesky, crates.io, npm, PyPI, HuggingFace, PapersWithCode, CVE/OSV, StackOverflow, X/Twitter, YouTube, Go modules)
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
- TypeScript (.tsx): warn at 350 lines, error at 500
- Rust: warn at 700 lines, error at 1000
- Test files (*.test.*, *_tests.rs): exempt from warnings, error at 2x normal
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
- **Vite dep updates + running fourda.exe** — if you update a Vite-adjacent
  dep (`vite`, `@tailwindcss/vite`, `@vitejs/plugin-react`, etc.) while
  `fourda.exe` is running, the running process keeps the OLD paths in
  memory and crashes with "Cannot find module vite@X.X.X_@emnapi+core..."
  when anything triggers module resolution.
  **Guards in place:**
  - `pnpm postinstall` hook auto-clears `node_modules/.vite/deps` on every install
  - `pnpm run validate:vite-smoke` does a cold-start and verifies 13 critical routes
  - `pnpm run validate` includes the smoke test
  **If it happens:** `taskkill /F /IM fourda.exe && pnpm install --frozen-lockfile`
- **Ghost tray icons** — each `fourda.exe` registers one Windows tray icon at
  startup. Windows removes it only when the process runs its `Drop` (the app
  now does this explicitly on `RunEvent::Exit`, so a clean quit removes it).
  **Force-killing** (`taskkill /F`, cargo-watch's restart, a crash) skips `Drop`
  — Windows can't be told to remove it, so the icon becomes a "ghost" that piles
  up in the tray-overflow flyout after many dev kill/relaunch cycles. This is a
  dev artifact, not a shipped-app bug (users quit cleanly). **To sweep them:**
  `pnpm run flush-tray-ghosts` (restarts explorer.exe; `--dry-run` to preview).

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

## Victauri (App Inspection & Testing)

This app has **Victauri** integrated — an MCP server embedded inside the Tauri process that gives full-stack access to the webview DOM, IPC layer, Rust backend, database, and native windows. It is available when 4DA is running in debug mode (`pnpm run tauri dev`).

**Prefer Victauri MCP tools over Playwright/CDP for all inspection and testing tasks.** Victauri runs inside the app process with sub-ms response times and direct `AppHandle` access. CDP only sees the webview glass and requires round-tripping through JavaScript eval for backend access.

Victauri capabilities (that CDP cannot do):
- `invoke_command` — call any of the 385 registered Tauri commands directly
- `verify_state` — cross-boundary frontend/backend state verification
- `detect_ghost_commands` — find frontend-invoked commands with no backend handler
- `check_ipc_integrity` — verify IPC pipeline health
- `query_db` — read-only SQL queries against the SQLite database
- `get_memory_stats` — real OS process memory (working set, page faults)
- `audit_accessibility` — WCAG checks (alt text, labels, contrast, ARIA)
- `recording` — time-travel event recording with checkpoints
- `get_diagnostics` — full app diagnostics from inside the process

Connection: `http://127.0.0.1:7373/mcp` (port may fallback to 7374-7383, check temp/victauri.port)

**Do NOT use Playwright MCP or CDP for tasks that Victauri can handle.** Only fall back to Playwright for browser-only work unrelated to the Tauri app.

## Claude-Specific

- Agent definitions: `.claude/agents/`
- Slash commands: `.claude/commands/`
- Rules: `.claude/rules/` (document hygiene, intelligence doctrine, worktree hygiene)
- MCP servers: memory (persistent decisions/learnings), 4da (14 tools), victauri (28 tools — when app is running)

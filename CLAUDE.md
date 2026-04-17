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

## Reference Docs

Before modifying architecture or invariants, read the relevant `.ai/` file:
- `WISDOM.md` — **the operating system** for 4DA development (authority stack, principles, gates, enforcement)
- `INVARIANTS.md` — non-negotiable system constraints
- `DECISIONS.md` — architectural decisions log (prevents re-litigation)
- `ARCHITECTURE.md` — system structure reference
- `FAILURE_MODES.md` — known fragile areas and previous regressions

## Intelligence Reconciliation (2026-04-16 — IN PROGRESS)

Before touching ANY intelligence surface (Briefing, Preemption, Blind Spots, Evidence, AWE, Knowledge Decay, Signal Chains), read:

- `docs/strategy/INTELLIGENCE-RECONCILIATION.md` — THE plan: 12→5 tab collapse, AWE as invisible spine, one canonical type.
- `docs/strategy/EVIDENCE-ITEM-SCHEMA.md` — the canonical `EvidenceItem` contract every intelligence surface must emit.
- `.claude/rules/intelligence-doctrine.md` — the ten enforced rules (no new types, no vanity metrics, no AWE panels, etc).

The Momentum tab is **being deleted**. AWE becomes infrastructure, not a feature. Five parallel intelligence systems are collapsing into one pipeline. Do NOT add new Alert/Gap/Recommendation/Insight struct variants — extend `EvidenceItem` via ADR instead.

## Never Commit

- `data/settings.json` — contains user API keys. Use `data/settings.example.json` as template.
- `data/*.db` — runtime databases
- `src-tauri/target/` — Rust build artifacts

## Document Hygiene (Planning-Doc Protocol)

Internal planning / strategy / audit / checklist / roadmap docs **must not live at the repo root.** The pre-commit gate (`scripts/check-doc-location.cjs`) rejects commits that try. Full framework: `.claude/rules/document-hygiene.md`.

Before writing any `*.md` at repo root whose name contains `PLAN`, `STRATEGY`, `AUDIT`, `CHECKLIST`, `ROADMAP`, `TRAJECTORY`, `VIRAL`, `LAUNCH`, `FORTIFICATION`, `EXECUTION`, `ASCENT`, `BATTLE`, `MASTER`, `BARRIER`, `IMPROVEMENTS`, `CRITICAL`, `DEVELOPER-OS`, `NOTIFICATION-INTELLIGENCE`, `INTELLIGENCE-CONSOLE`, `whats-next`, `NEXT-STEPS`, `MISSION_`, `SHIP_`, `FIRST-CONTACT`:

1. **Default**: use TodoWrite + in-conversation reasoning — no file.
2. If a persistent doc is needed: write to `.claude/plans/` (gitignored, never leaks).
3. Curated strategy docs go to `docs/strategy/` **only after explicit user approval per-file**.
4. Never write such docs at repo root. If the gate says a doc belongs there, it's wrong — move it.

Public-facing root `.md` allowlist: `README CHANGELOG LICENSE CONTRIBUTING CODE_OF_CONDUCT SECURITY CLAUDE AGENTS CONVENTIONS TRADEMARKS CLA LINUX NETWORK`. Escape hatch for genuine public docs that match a block pattern: add `<!-- public-ok: <reason> -->` to the first 10 lines.

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

## Airlock (Agent Safety Gates)

Airlock is safety middleware for agent-to-agent communication. Every message between agents passes through 9 composable safety gates before delivery. Irreversible actions require human acknowledgement. Every message is logged in both machine-parseable and human-readable form.

**Repository:** `D:\runyourempire\glyph` (standalone Rust workspace, Apache 2.0) — GitHub repo rename to `airlock` pending
**Dictionary:** 60 glyphs across 8 categories, hash-locked via blake3
**Crates:** airlock-core, airlock-engine, airlock-compile, airlock-lift, airlock-safety, airlock-cli

**9 safety gates** (all load-bearing, non-optional):

1. Roundtrip invariant — every envelope compiles to non-empty NL, property-tested
2. Payload invariant — payloads cannot contain glyphs (anti-steganography)
3. Anti-steg statistical monitor — per-agent frequency divergence flagging
4. Capability declaration — agents register which glyphs they may emit
5. Reversibility gate — `⟲` and `🔒` envelopes route through AWE consequence scan
6. Mandatory human ACK — `🔴` `⬛` `🔒` `✋` envelopes block until human ACK
7. Dual-form audit log — wire + compiled NL, always written (even for rejections)
8. Dictionary version gate — hard mismatch error, no compatibility mode
9. Semantic drift detector — weekly recompile of stored envelopes vs current dict

**Rollout phases:**
- Phase 0 — spec + tokenizer measurement ✅ (compression claim retracted, safety gates are the value)
- Phase 1 — Rust crate with 35 tests ✅
- Phase 1.5 — Integration harness (SqliteAuditSink, mocks, demo) ✅
- Phase 2 — audit-only mode in 4DA (shadow envelopes, no agent behavior change)
- Phase 3 — first opt-in agent emits real envelopes
- Phase 4 — broker routing by typed headers
- Phase 5 — safety hardening (real AWE + UI bridges)
- Phase 6 — compound measurement + AWE feedback loop

**Kill gates:** Phase 2 aborts if categorical coverage <50% or audit log growth excessive.
**Canonical spec:** `D:\runyourempire\glyph\docs\SPEC.md` (dual-licensed CC-BY-4.0)

## Worktree Hygiene

Subagents spawned with `isolation: "worktree"` create a new worktree under `.claude/worktrees/agent-<hash>/` and a matching branch `worktree-agent-<hash>`. After the subagent's commits are merged into main, the worktree directory and branch remain — neither the subagent nor the orchestrator cleans up. Over time these accumulate and trigger sentinel alarms.

**Prevention:** `node scripts/cleanup-orphaned-worktrees.cjs` — dry-run by default, shows what would be removed. Add `--execute` to apply. Safe by design: refuses to remove any branch whose tip is NOT reachable from main, or any worktree with uncommitted changes. Reflog preserves everything for 90 days. Suggested cadence: run nightly or via a pre-push hook.

**2026-04-12 cleanup:** 11 dead `worktree-agent-*` branches + 4 stale `.claude/worktrees/agent-*` directories deleted. All verified safe (every branch tip was reachable from main; all directories were empty or held only stale snapshots with zero unique content).

## Claude-Specific

- Agent definitions: `.claude/agents/` (4DA-specific agents for source debugging, trend analysis, etc.)
- Slash commands: `.claude/commands/` (project-specific commands)
- MCP servers: memory (persistent decisions/learnings), 4da (33 intelligence tools), awe (7 wisdom tools)
- Hooks: wisdom gates (PreToolUse), consequence processing + AWE wisdom injection (UserPromptSubmit), session capture + AWE feedback tracking (Stop), prompt analyzer

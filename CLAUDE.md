# 4DA — Claude Code Instructions

**4DA = 4 Dimensional Autonomy** — Tauri 2.0 (Rust + React + SQLite)
**Dev:** localhost:4444 | **Package manager:** pnpm | **Decisions log:** `.ai/DECISIONS.md`

## Commands

```bash
pnpm run tauri dev         # Dev server
pnpm run tauri build       # Production build
cargo test                 # Rust tests (from src-tauri/)
pnpm run test              # Frontend tests
pnpm run validate:all      # Full validation
```

## Principles

1. Privacy first — raw data never leaves the machine
2. BYOK — user provides API keys, never stored remotely
3. Local first — works offline with Ollama fallback
4. Minimal — no feature bloat, every element earns its place

## Design Tokens

```css
--bg-primary: #0A0A0A;    --bg-secondary: #141414;   --bg-tertiary: #1F1F1F;
--text-primary: #FFFFFF;   --text-secondary: #A0A0A0; --text-muted: #666666;
--accent-primary: #FFFFFF; --accent-gold: #D4AF37;    --border: #2A2A2A;
--success: #22C55E;        --error: #EF4444;
```

Fonts: Inter (UI), JetBrains Mono (code) | Weights: 400, 500, 600

## Import Order

**TS:** React/framework > External packages > Internal (`@/`) > Relative > Types
**Rust:** std > External crates > `crate::` > `super::`

## File Size Limits

New source files must stay within line-count limits:
- **TypeScript/TSX**: warn at 350, error at 500 lines
- **Rust**: warn at 600, error at 1000 lines

Exceeding files must be split or added to exception list in `scripts/check-file-sizes.cjs`.
Run `pnpm run validate:sizes` to check.

## Reference

Before modifying architecture or invariants, read the relevant `.ai/` file:
`INVARIANTS.md` | `ARCHITECTURE.md` | `DECISIONS.md` | `FAILURE_MODES.md`

Full spec: `specs/ARCHITECTURE.md`

## Never Commit

- `data/settings.json` — contains user API keys and local paths. Use `data/settings.example.json` as template.

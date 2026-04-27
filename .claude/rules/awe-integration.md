# AWE Integration

AWE is the judgment layer that transmutes intelligence into calibrated wisdom. It runs as an MCP server with 7 tools backed by a Rust engine.

## When to use AWE tools

- `awe_transmute` — Before any **high-stakes decision** (architecture changes, irreversible actions, new abstractions). Returns bias detection, consequence modeling, confidence calibration, and trade-off analysis.
- `awe_quick_check` — Fast sanity check on any decision. Use liberally — it's cheap.
- `awe_consequence_scan` — Before irreversible actions. Models 1st/2nd/3rd order consequences with reversibility scoring.
- `awe_feedback` — **After every decision outcome is known.** This is critical — AWE compounds by learning from outcomes. Feed it confirmed/refuted/partial results.
- `awe_recall` — At session start or before decisions in a domain. Retrieves accumulated principles, anti-patterns, and precedents.
- `awe_calibration` — Periodic check on AWE's judgment quality per domain.

## Automated integration (hooks handle this)

- Session start: AWE wisdom (principles/anti-patterns) injected automatically
- Session start: Previous session's AWE decisions queued for feedback
- Session end: Recent AWE decisions captured in pending.json for next-session feedback
- Wisdom Gate 2: Destructive action warnings include `awe_consequence_scan` reminder

## Decision recording flow

1. Identify a significant decision → `awe_quick_check` (fast bias scan)
2. If high-stakes → `awe_transmute` (full pipeline, auto-persists to Wisdom Graph)
3. Also record in `remember_decision` (MCP memory) for session memory
4. When outcome is known → `awe_feedback` with decision_id

## Three decision stores — clear routing

| Store | Purpose | When to use |
|-------|---------|-------------|
| **AWE** (`awe_transmute`) | Judgment-augmented decisions with consequence modeling | Architectural choices, irreversible actions, design trade-offs |
| **MCP Memory** (`remember_decision`) | Dev session memory that survives context compaction | Learnings, gotchas, workflow decisions, code locations |
| **4DA Decision Memory** (`decision_memory`) | Tech stack tracking and alignment checking | Auto-inferred tech choices, `check_decision_alignment` queries |

Do NOT record the same decision in all three. Route by purpose. AWE is for decisions that need judgment. Memory is for session persistence. 4DA is for tech alignment.

## AWE binary and database

- Binary: `D:\runyourempire\awe\target\release\awe.exe`
- Database: `%APPDATA%\awe\wisdom.db`

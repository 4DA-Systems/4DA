# Architectural Decisions Log
## Engineering Memory for 4DA

> **Authority Level: STANDARD** — Decisions are subordinate to both `INVARIANTS.md` and `WISDOM.md`. A decision cannot override an invariant or contradict a principle. Decisions override mitigations in `FAILURE_MODES.md`. Authority stack: `INVARIANTS.md` > `WISDOM.md` > `DECISIONS.md` > `FAILURE_MODES.md` > `CLAUDE.md`.

**Version:** 2.0.0
**Source:** Restored to `.ai/DECISIONS.md` on 2026-06-27 from the archived log (`.claude/plans/archive-2026-04-18/ai/DECISIONS.md`, AD-001→AD-025), reconciled against current code/license state, with AD-026 promoted from code evidence. The file had been referenced by `CLAUDE.md`, the WISDOM authority stack, and the pre-push hook but did not exist on disk.
**Purpose:** Prevent re-litigation of settled decisions.

---

## How to Use This File

1. **Before proposing changes:** Check if a relevant decision already exists.
2. **When making new decisions:** Add to this file immediately with a sequential `AD-NNN` id.
3. **When challenging decisions:** Note alternatives in the "Considered" section; re-litigation requires new evidence.
4. **When a decision evolves:** Keep the original entry for the historical record and append a dated **Update** note (or mark `Status: Superseded by AD-YYY`). Do not silently rewrite history.

## Decision Statuses

| Status | Meaning |
|--------|---------|
| **Final / Accepted** | Active and in effect. This is how the project works. |
| **Active** | In effect but expected to transition (e.g., warnings-mode CI graduating to blocking). |
| **Superseded** | Replaced by a newer decision; the superseding `AD-YYY` is noted. |
| **Deprecated** | No longer applicable; the system has changed enough that the decision is irrelevant. |

---

## Core Architecture

### AD-001: Tauri 2.0 over Electron
- **Decision:** Use Tauri 2.0 (Rust + WebView) instead of Electron.
- **Rationale:** 10x smaller binary, 5x faster startup, native Rust performance for indexing.
- **Considered:**
  - Electron: Rejected — too heavy for an ambient background tool.
  - Flutter: Rejected — less mature desktop support, Dart learning curve.
- **Date:** Project inception
- **Status:** Final

### AD-002: SQLite + sqlite-vec for Vector Storage
- **Decision:** Use SQLite with the vector-search extension for embeddings — single embedded file, no external database.
- **Rationale:** No external database needed, single file, portable, sufficient for a local-first app.
- **Considered:**
  - Pinecone/Weaviate: Rejected — violates local-first principle, adds complexity.
  - PostgreSQL + pgvector: Rejected — too heavy for a desktop app.
  - Qdrant: Rejected — external dependency for a local-first app.
- **Date:** Project inception
- **Status:** Final
- **Update (2026-06-27):** The original decision named `sqlite-vss`. The codebase now uses `sqlite-vec` (`src-tauri/Cargo.toml`: `sqlite-vec = "0.1"`), the maintained successor extension and SQLCipher-compatible. KNN queries require `k = ?` in the `WHERE` clause, not a trailing `LIMIT` (see `CLAUDE.md` gotchas). The decision (SQLite-embedded vector search, local-first) is unchanged; the extension implementation moved vss→vec.

### AD-003: BYOK (Bring Your Own Key) Model
- **Decision:** Users provide their own API keys, never stored remotely.
- **Rationale:** Privacy-first principle, no server costs, user controls their data.
- **Considered:**
  - Server-side API proxy: Rejected — privacy violation, liability.
  - Free tier with our keys: Rejected — unsustainable, creates wrong incentives.
- **Date:** Project inception
- **Status:** Final

---

## Embedding Strategy

### AD-004: Embedding Model Selection
- **Decision:** Use fastembed with MiniLM-L6-v2 (384 dimensions) for local embeddings, in-process by default.
- **Rationale:** Runs locally without API calls, deterministic results, sufficient quality for similarity search, fast CPU inference.
- **Considered:**
  - OpenAI text-embedding-3-small: Good but requires an API and costs money.
  - Ollama embeddings: Viable fallback but slower.
- **Date:** Phase 0 implementation
- **Status:** Final — fastembed (ONNX) is the in-process default; Ollama is the fallback; zero vectors are the last resort.

---

## Frontend Architecture

### AD-005: React 19 + TypeScript + Tailwind v4
- **Decision:** Standard modern web stack (React 19, Tailwind v4, Vite).
- **Rationale:** Developer familiarity, excellent tooling, Tailwind for rapid UI.
- **Considered:**
  - Vue: Rejected — smaller ecosystem.
  - Svelte: Rejected — less mature Tauri integration.
  - Solid: Rejected — smaller community.
- **Date:** Project inception
- **Status:** Final

---

## Design System

### AD-006: Matte Black Minimalism
- **Decision:** Dark theme (#0A0A0A base), minimal chrome, gold accent used sparingly.
- **Rationale:** An ambient tool should be visually quiet, not attention-seeking.
- **Considered:**
  - Light theme: Rejected as the *default* — most developers prefer dark. (A "Paper" light theme has since shipped as an opt-in, not a replacement of the matte-black default.)
  - Colorful UI: Rejected — too attention-seeking for an ambient tool.
- **Date:** Project inception
- **Status:** Final

---

## CADE Decisions

### AD-007: Cognition Artifacts in .ai/
- **Decision:** Create a dedicated `.ai/` directory for cognition artifacts, separate from `.claude/` (runtime state and hooks).
- **Rationale:** `.ai/` holds stable truth-source documents that define agent behavior; `.claude/` holds dynamic runtime state. Clear separation of concerns.
- **Considered:**
  - Merge with `.claude/`: Rejected — conflates runtime and truth-source.
  - Root-level files: Rejected — clutters the project root.
- **Date:** CADE implementation
- **Status:** Final

### AD-008: Two-Phase Protocol Enforcement
- **Decision:** Require an explicit Phase 1 (Orientation) before Phase 2 (Execution).
- **Rationale:** Prevents premature coding, ensures shared understanding before work begins, reduces rework from misunderstood requirements.
- **Date:** CADE implementation
- **Status:** Final

### AD-009: CI as Validation Authority
- **Decision:** GitHub Actions CI is the validation authority, not the agent.
- **Rationale:** Agents cannot self-certify correctness; machine verification prevents fabricated claims; CI logs are the audit trail.
- **Considered:**
  - Agent self-validation: Rejected — agents can fabricate confidence.
  - Manual review only: Rejected — not scalable.
- **Date:** CADE implementation
- **Status:** Final

### AD-010: Warnings-First CI Rollout
- **Decision:** Start CI gates in warnings mode (`continue-on-error: true`), then graduate to blocking.
- **Rationale:** Allows baseline establishment, prevents productivity loss during tuning, graduates to blocking once patterns are understood.
- **Date:** CADE implementation
- **Status:** Active — most gates have since graduated to blocking (aggregate gates + branch protection); retained as the historical rollout decision.

### AD-011: Frontend Test Infrastructure First
- **Decision:** Set up Vitest infrastructure without writing extensive tests initially.
- **Rationale:** Gets gates in place, allows incremental test addition, doesn't derail the main CADE implementation.
- **Date:** CADE implementation
- **Status:** Final

---

## Void Engine

### AD-012: Void Engine — Heartbeat is Production, Universe is Experimental
- **Decision:** The Void Engine heartbeat (ambient 48px signal indicator) is a production feature. The 3D universe (Three.js spatial visualization) is experimental and receives no further investment until the core product loop (signals, briefings, feedback) is mature.
- **Rationale:**
  - The heartbeat communicates real system state (scanning, idle, stale, error, discoveries) through a 48px ambient glow, fitting the "quiet ambient tool" philosophy.
  - The 3D universe contradicts 4DA's core value proposition — 4DA *delivers* what matters; it doesn't ask you to explore a cloud of dots (delivery mode, not discovery mode).
  - Johnson-Lindenstrauss random projection (384-dim → 3D) doesn't produce human-interpretable clusters.
  - The Three.js bundle (~908KB) is ~2.5x the rest of the app for a rarely-used feature (code-split via `React.lazy`, so it costs nothing when not loaded).
  - Particle relevance scores were never populated (all 0.0).
- **Considered:**
  - Remove the universe entirely: Rejected — code is clean, code-split, and costs nothing unused; keeping it preserves optionality.
  - Invest in fixing the universe: Deferred — signals/briefings/feedback have higher ROI.
  - Make the universe primary: Rejected — antithetical to ambient delivery.
- **Date:** 2026-02-09
- **Status:** Final

### AD-013: Void Engine Signal Architecture
- **Decision:** Void signals are change-driven (emit only when values differ), not timer-driven; the frontend interpolates locally at 30fps.
- **Rationale:** Zero CPU cost when idle (most of the time for an ambient tool); emissions are hooked into real backend events (fetch start, analysis complete, error, ACE scan); the RAF loop with a cancelled flag prevents memory leaks on unmount.
- **Date:** 2026-02-09
- **Status:** Final

---

## Module Structure

### AD-014: lib.rs Decomposition into Focused Modules
- **Decision:** Split the monolithic `lib.rs` (3,835 lines) into focused modules while preserving all `use crate::` import paths via re-exports.
- **Rationale:** The single file had unrelated responsibilities (types, global state, embeddings, text processing, events, and 15+ Tauri commands). The re-export pattern means no other module needs to change. Same pattern that succeeded with the `scoring.rs` → `scoring/` split.
- **Structure:** `lib.rs` (mod declarations, re-exports, `run()`), `commands.rs`, `utils.rs`, `state.rs`, `embeddings.rs`, `types.rs`, `events.rs`.
- **Considered:**
  - Keep as a single file: Rejected — painful navigation, review, and merge conflicts.
  - Domain-specific `*_commands.rs` files: Deferred — would fragment the invoke_handler further.
- **Date:** 2026-02-15
- **Status:** Final

### AD-015: Re-export Pattern for Module Decomposition
- **Decision:** When splitting modules, always re-export from `lib.rs` to preserve `use crate::item` paths; never require callers to change imports.
- **Rationale:** Zero-disruption refactoring, incremental extraction (one module at a time, test after each), easy rollback.
- **Date:** 2026-02-15
- **Status:** Final

---

## License & Monetization

### AD-016: FSL-1.1-Apache-2.0 over BUSL-1.1
- **Decision:** Use FSL-1.1-Apache-2.0 for the application (switched from an earlier BUSL-1.1 plan).
- **Rationale:**
  - BUSL-1.1 is not OSI-approved, causing enterprise legal friction and developer hesitancy.
  - FSL-1.1 provides equivalent competitive-fork protection while converting to a permissive license.
  - FSL avoids the "HashiCorp backlash" association BUSL carries.
  - Apache 2.0 as the future/change license is permissive and widely trusted.
- **Considered:**
  - AGPL-3.0: Rejected — too restrictive for a desktop app, scares enterprise users.
  - MIT/Apache-2.0 immediately: Rejected — no competitive protection for monetization.
  - Keep BUSL-1.1: Rejected — adoption friction outweighs stricter protection.
- **Date:** 2026-02-17
- **Status:** Final
- **Update (2026-06-27):** Two details corrected against the live `LICENSE` and published packages:
  - **Conversion period is 3 years, not 2.** `LICENSE` sets `Change Date: 2029-04-20` ("the third anniversary"), converting to Apache License 2.0.
  - **The published MCP server is Apache-2.0, not MIT.** `@4da/mcp-server` (`mcp-4da-server/package.json`) ships `"license": "Apache-2.0"`. The split is deliberate: the app is FSL-1.1-Apache-2.0; the published npm MCP server is Apache-2.0 for maximum ecosystem adoption. Do not "fix" this to MIT.

### AD-017: Signal Tier Feature Gate ($12/mo, $99/yr)
- **Decision:** Gate compound-intelligence features behind a Signal tier. The free tier retains all source adapters, the scoring engine, the feed UI, and basic signal detection.
- **Rationale:** The free tier must remain genuinely useful (sources + scoring + feed + BYOK-run AI) to drive adoption; Signal sells the proprietary intelligence that compounds over time. License key stored locally (BYOK philosophy extends to licensing).
- **Considered:**
  - Usage-based pricing: Rejected — unpredictable costs scare BYOK users.
  - Open core with a separate repo: Rejected — maintenance overhead of two codebases.
  - Donations/sponsorship only: Rejected — insufficient for sustainable development.
- **Date:** 2026-02-17
- **Status:** Final — see AD-025 for the BYOK-aware recalibration of exactly what is gated.

---

## Wisdom Layer

### AD-018: Wisdom Layer as Principles Document, Not Code Framework
- **Decision:** Implement the wisdom layer as `.ai/WISDOM.md` — a living document of principles, zero zones, and practical gates. Not a TypeScript framework, database schema, or enforcement engine.
- **Rationale:** Principles that live in a document get read; code that enforces principles gets worked around. 4DA's reality is one human + one AI partner. The MCP memory server already provides consequence tracking. Zero zones map directly to existing INVARIANTS.
- **Considered:**
  - Full TypeScript wisdom framework: Rejected — enterprise-grade governance creates friction without proportional benefit for solo development.
  - Database-backed consequence ledger with SQL triggers: Rejected — MCP memory already provides this.
  - No wisdom layer: Rejected — AI-assisted development at velocity requires explicit principles to prevent drift.
- **Date:** 2026-02-22
- **Status:** Final

### AD-019: AI Engineering Contract Absorbed into Wisdom Layer
- **Decision:** Merge the behavioral rules from `AI_ENGINEERING_CONTRACT.md` into `WISDOM.md` v2.0.0. The contract is superseded; WISDOM.md is the single behavioral authority.
- **Rationale:** Two behavioral documents with overlapping scope created authority ambiguity. The Wisdom Layer has autonomous enforcement hooks (PreToolUse gate, UserPromptSubmit processing, Stop capture); the contract had none. Contract concepts are fully absorbed (Two-Phase Protocol → Development Covenant, Forbidden Actions → Zero Zones, Validation Artifacts → Gate 3). The authority stack (INVARIANTS > WISDOM > DECISIONS > FAILURE_MODES > CLAUDE.md) eliminates precedence ambiguity.
- **Considered:**
  - Keep both documents: Rejected — overlapping authority with no precedence rule.
  - Delete the contract entirely: Rejected — it remains a historical reference (marked SUPERSEDED in place).
  - Create a constitution above both: Rejected — governance meta-layers add complexity without proportional benefit.
- **Date:** 2026-02-23
- **Status:** Final

### AD-020: Pure Rust Dependencies Over C/System Library Bindings
- **Decision:** When choosing between a Rust crate with C/system-library bindings and a pure-Rust alternative, prefer pure Rust when quality is comparable. Document exceptions explicitly with build instructions.
- **Rationale:** C-binding dependencies (tesseract, whisper-rs, system OpenSSL) cause Windows build failures, require vcpkg/system-lib setup, and add cross-platform fragility. Three independent experiences confirmed this (tesseract→ocrs, tesseract+whisper removal, native-binding failures). Pure-Rust crates (ocrs, pdf-extract, lopdf, docx-rs, calamine) eliminated whole categories of build problems. First pattern promoted via `/crystallize`.
- **Considered:**
  - Allow C bindings freely: Rejected — build failures and platform fragility outweigh marginal quality.
  - Ban C bindings absolutely: Rejected — sometimes no pure-Rust alternative exists (e.g., SQLite itself); exceptions are documented, not banned.
- **Date:** 2026-02-23 (crystallized from MCP memory)
- **Status:** Final

---

## Game Engine

### AD-021: Game Engine Achievement Schema (Pinned)
- **Decision:** Lock the achievement/game-state schema. Fields are final and must not be renamed or restructured without a new AD entry.
- **Schema:**
  - `Achievement`: `id`, `name`, `description`, `icon`, `counter_type`, `threshold`
  - `AchievementState` (frontend): the above + `current`, `unlocked`, `unlocked_at`
  - `AchievementUnlocked` (event): `id`, `name`, `description`, `icon`, `unlocked_at`
  - `GameState`: `counters: Vec<CounterState>`, `achievements: Vec<AchievementState>`, `streak`, `last_active`
  - `CounterState`: `counter_type`, `value`
- **Rationale:** The schema just underwent a breaking rename (`title`→`name`, `progress`→`current`, flat stats→`counters` array) requiring coordinated changes across six files; further churn multiplies cost for no user value. The counter-based design is clean and extensible — new achievements only add entries to `all_achievements()`.
- **Considered:**
  - Allow organic evolution: Rejected — the rename just cost a full-stack change; locking prevents repeats.
  - Add more fields now (rarity, category, xp_reward): Deferred — add when achievement count exceeds 25.
- **Date:** 2026-03-02
- **Status:** Final

---

## Tiers, Team & Monetization (continued)

### AD-022: Tier Rename Pro→Signal + Enterprise Tier + STREETS Coaching Deprecation
- **Decision:** Rename the "Pro" tier to "Signal", add an "enterprise" tier, and remove the STREETS Community/Cohort tiers.
- **Rationale:** "Signal" reinforces brand vocabulary ("All signal. No feed."), is identity-based not feature-based, and is unique in the market. The STREETS coaching/cohort tiers were never launched — the STREETS playbook stays free for all users (and now publishes on 4da.ai). Enterprise supports bottom-up PLG.
- **Tier structure:** Free → Signal ($12/mo, $99/yr) → Team ($29/seat/mo) → Enterprise (custom).
- **Backwards compat:** Legacy `"pro"` in settings.json is accepted via `is_paid_tier()`; new activations write `"signal"`. (User-facing language is always "Signal"; `ProGate`/`isPro` internal code is fine.)
- **Removed:** `STREETS_COMMUNITY_FEATURES`, `STREETS_COHORT_FEATURES`, `is_streets_feature_available()`, `require_streets_feature()`, `get_streets_tier()`, `activate_streets_license`.
- **Date:** 2026-03-10
- **Status:** Final

### AD-023: Team Relay Architecture — Encrypted Metadata Sync
- **Decision:** Build a thin coordination relay server for Team/Enterprise multi-seat features. "Dumb relay, smart clients" — the relay stores and routes E2E-encrypted blobs and cannot read team metadata; clients aggregate locally.
- **Rationale:** 4DA is a desktop app — each seat has its own SQLite database; multi-seat features (shared DNA, signal-chain aggregation, team decisions, org dashboards) need a data transport layer. Four options were evaluated — (A) thin cloud relay, (B) designated coordinator machine, (C) Keygen metadata piggyback, (D) P2P mesh — and (A) won on reliability, UX, and privacy preservation.
- **Architecture:** `docs/strategy/TEAM-RELAY-ARCHITECTURE.md`.
- **Encryption:** XChaCha20Poly1305 + X25519 key exchange + HKDF derivation (all pure Rust).
- **Conflict resolution:** Last-Write-Wins with a Hybrid Logical Clock (not CRDTs — overkill for key-value metadata).
- **Transport:** WebSocket (real-time) + HTTP polling (offline catch-up).
- **Self-hosted:** Enterprise customers run the relay on their own infrastructure via Docker.
- **Crates:** chacha20poly1305 0.10, x25519-dalek 2, hkdf 0.12, uhlc 0.7, tokio-tungstenite 0.23; relay: axum, sqlx.
- **Date:** 2026-03-11
- **Status:** Final

### AD-024: Team/Enterprise Launch Deferral
- **Decision:** Ship Free + Signal tiers only at launch. Team and Enterprise are built and tested pre-launch but hidden from the pricing page until organic demand warrants activation.
- **Rationale:** Shipping identical functionality at three price points erodes trust. Team requires the relay + shared-intelligence features; Enterprise requires audit logs, webhooks, SSO, multi-team orgs — all built on the relay (AD-023). Building before launch ensures readiness; deferring visibility ensures quality.
- **Trigger to enable:** Organic user signals ("I wish my team could see this"), waitlist volume, or direct enterprise inquiry.
- **Date:** 2026-03-11
- **Status:** Final

### AD-025: BYOK-Aware Tier Recalibration
- **Decision:** Recalibrate Free vs Signal around BYOK reality. Free gets the engine *including* AI features that run on the user's own key (daily AI briefing, basic NL search, behavior learning) at zero marginal cost. Signal sells compound intelligence (temporal analysis, identity intelligence, persistent watchers, "what you would have missed" analytics, Key Signals categorization, signal-classification labels).
- **Rationale:** The previous split gated AI features that cost 4DA nothing to provide (BYOK), which felt extractive and misaligned with privacy-first values. The new split gates proprietary intelligence that compounds over accumulated data — defensible value the user cannot replicate.
- **Considered:**
  - Keep AI features gated: Rejected — BYOK means zero marginal cost; gating pass-through compute feels extractive.
  - Make everything free: Rejected — compound intelligence is real proprietary value worth paying for.
  - Usage-based pricing for AI: Rejected — unpredictable costs scare BYOK users.
- **Date:** 2026-04-05
- **Status:** Final
- **Code:** `src-tauri/src/settings/license/gating.rs` — `natural_language_query` and `generate_ai_briefing` run on the user's key and are intentionally *not* in `SIGNAL_FEATURES`.

### AD-026: Developer DNA Un-gated — Free-Tier Viral Sharing
- **Decision:** Leave Developer DNA cards un-gated and free (deliberately excluded from the `SIGNAL_FEATURES` list).
- **Rationale:** Developer DNA is a viral growth loop — shareable DNA cards drive word-of-mouth and team adoption. Paywalling a core identity/sharing feature would suppress the network effect that brings new users in. The compounding value (and the paid surface) lives in the Signal-tier intelligence that DNA feeds into (AD-025), not in the shareable card itself.
- **Considered:**
  - Gate DNA behind Signal: Rejected — kills the free viral sharing loop that drives acquisition.
  - Gate only DNA *export/sharing*: Rejected — friction on the exact action that creates the growth loop.
- **Date:** 2026-05-25
- **Status:** Final
- **Code:** `src-tauri/src/settings/license/gating.rs:38` — "Developer DNA un-gated (AD-026): free tier viral sharing of DNA cards".

---

## Rejected Alternatives (Reference)

| ID | Alternative | Reason for Rejection |
|----|-------------|---------------------|
| REJ-001 | Electron | Too heavy for an ambient background tool |
| REJ-002 | External Vector DB | Violates local-first principle |
| REJ-003 | Server-side API keys | Privacy violation, liability |
| REJ-004 | Agent self-certification | Agents can fabricate confidence |
| REJ-005 | Light theme as default | Most developers prefer dark (opt-in light "Paper" theme shipped later) |
| REJ-006 | 3D universe as primary view | Contradicts ambient delivery philosophy; discovery vs delivery mode |
| REJ-007 | Remove universe code entirely | Code is clean, code-split, costs nothing unused; preserves optionality |
| REJ-008 | Keep BUSL-1.1 license | Adoption friction outweighs stricter protection period |
| REJ-009 | AGPL-3.0 license | Too restrictive for a desktop app, scares enterprise users |
| REJ-010 | Usage-based Signal pricing | Unpredictable costs scare BYOK users |

---

## Pending Decisions

*Decisions under active consideration.*

| ID | Topic | Options | Status |
|----|-------|---------|--------|
| — | (None currently) | — | — |

---

## Decision Template

When adding a new decision:

```markdown
### AD-NNN: [Short Title]
- **Decision:** [What was decided]
- **Rationale:** [Why this choice was made]
- **Considered:**
  - [Alternative 1]: [Why rejected]
  - [Alternative 2]: [Why rejected]
- **Date:** [When decided]
- **Status:** [Final/Active/Superseded/Deprecated]
```

---

*Decisions are made once and referenced often. Re-litigation requires new evidence.*

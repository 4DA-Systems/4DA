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
- **Decision:** Gate the Signal analysis layer behind a paid tier. The free tier retains all source adapters, the scoring engine, the feed UI, and basic signal detection.
- **Rationale:** The free tier must remain genuinely useful (sources + scoring + feed + BYOK-run AI) to drive adoption; Signal sells the analysis layer — the derived intelligence (Signal Chains, Knowledge Gaps, Semantic Shifts, temporal and identity analysis, persistent watchers) that the free engine does not compute. License key stored locally (BYOK philosophy extends to licensing).
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
- **Decision:** Recalibrate Free vs Signal around BYOK reality. Free gets the engine *including* AI features that run on the user's own key (daily AI briefing, basic NL search, Learned Preferences — user-controlled filtering) at zero marginal cost. Signal sells the analysis layer (temporal analysis, identity intelligence, persistent watchers, "what you would have missed" analytics, Key Signals categorization, signal-classification labels).
- **Rationale:** The previous split gated AI features that cost 4DA nothing to provide (BYOK), which felt extractive and misaligned with privacy-first values. The new split gates the derived analysis layer — work the free engine does not perform — rather than a forward-looking claim about accumulated learning.
- **Considered:**
  - Keep AI features gated: Rejected — BYOK means zero marginal cost; gating pass-through compute feels extractive.
  - Make everything free: Rejected — the analysis layer is real proprietary value worth paying for.
  - Usage-based pricing for AI: Rejected — unpredictable costs scare BYOK users.
- **Date:** 2026-04-05
- **Status:** Final
- **Amended:** 2026-08-12 (AD-030) — "compound intelligence"/"behavior learning" framing replaced; the free/paid boundary itself is unchanged.
- **Code:** `src-tauri/src/settings/license/gating.rs` — `natural_language_query` and `generate_ai_briefing` run on the user's key and are intentionally *not* in `SIGNAL_FEATURES`.

### AD-026: Developer DNA Un-gated — Free-Tier Viral Sharing
- **Decision:** Leave Developer DNA cards un-gated and free (deliberately excluded from the `SIGNAL_FEATURES` list).
- **Rationale:** Developer DNA is a viral growth loop — shareable DNA cards drive word-of-mouth and team adoption. Paywalling a core identity/sharing feature would suppress the network effect that brings new users in. The paid surface is the Signal-tier analysis that DNA feeds into (AD-025), not the shareable card itself. *(Amended 2026-08-12, AD-030.)*
- **Considered:**
  - Gate DNA behind Signal: Rejected — kills the free viral sharing loop that drives acquisition.
  - Gate only DNA *export/sharing*: Rejected — friction on the exact action that creates the growth loop.
- **Date:** 2026-05-25
- **Status:** Final
- **Code:** `src-tauri/src/settings/license/gating.rs:38` — "Developer DNA un-gated (AD-026): free tier viral sharing of DNA cards".

### AD-027: Cloud Embedding Requires Explicit Opt-In — Local-Only by Default (INV-004)
- **Decision:** Embedding never leaves the machine unless the user explicitly opts in via `llm.allow_cloud_embeddings` (default `false`). `embed_texts` routes through a single pure decision point, `resolve_embedding_route`, which can only return a cloud route when the opt-in is set. Setting a cloud *LLM* key no longer, as a side-effect, sends embeddings — including local file / project / context content — to `api.openai.com`.
- **Rationale:** `embed_texts` was content-agnostic: with `provider=openai` (or `anthropic` + an OpenAI key) it embedded ALL callers' text via OpenAI, and its callers include the ACE project scanner, README indexing, and context ingestion — i.e. the user's local files. A user who picked a cloud provider for *chat* silently shipped their indexed local content to OpenAI (retained 30 days per OpenAI policy). That is a direct violation of Principle #1 / INV-004 ("raw data never leaves the machine without explicit consent"). Default-off is the only posture consistent with a privacy-first, local-first product. `fastembed-local` is a default feature, so gated-off users embed locally (ONNX, zero network) with no loss of capability.
- **Considered:**
  - Keep content-agnostic routing, document the behaviour: Rejected — a documented silent exfiltration is still a silent exfiltration; violates the founding promise.
  - Tag each call site as local vs shareable and only gate "local" content: Rejected — brittle, easy to regress, and the honest default for a local-first app is that *nothing* leaves without consent.
  - Remove cloud embeddings entirely: Rejected — some users legitimately want OpenAI embeddings; the fix is consent, not removal.
- **Migration:** A one-shot `app_meta`-guarded reconciliation (`embedding_privacy_gate_v1`) re-embeds exactly the previously-cloud cohort into the local vector space on first launch after upgrade; existing local users store a byte-identical identity and are untouched. The persisted embedding identity now carries a `(cloud)` marker only on the cloud route, so a later opt-in/opt-out re-embeds honestly.
- **Date:** 2026-07-07
- **Status:** Final
- **Code:** `src-tauri/src/embeddings.rs` (`resolve_embedding_route`, `EmbeddingRoute`); `src-tauri/src/settings/types.rs` (`LLMProvider::allow_cloud_embeddings`); `src-tauri/src/reembed.rs` (`effective_embedding_identity`, `check_embedding_privacy_gate_migration`); `src-tauri/src/app_setup.rs` (startup wiring). Opt-in is config-only for now (`data/settings.json`); a settings-UI toggle is a deliberate follow-up.

### AD-028: Signal Lifetime Plan — Included, Priced at 3× Annual ($299 AUD)
- **Decision:** Sell a Signal Lifetime license: $299 AUD one-time (3.0× annual), alongside monthly ($12) and annual ($99). Lifetime = all Signal features and all future Signal updates for the lifetime of the 4DA product; the signed key verifies offline (2099 expiry), so the license keeps working even if 4DA is discontinued. Defined in TOS §4.1; 14-day money-back in TOS §5.3.
- **Rationale:** 4DA has zero marginal cost per Signal user (BYOK + local-first + stateless Cloudflare licensing), so lifetime carries none of the hosted-SaaS liability. The lifetime promise is unusually honest here because the cost structure carries it, not a forward-looking product claim: zero marginal cost per Signal user, and a signed key that verifies offline to 2099 — so the license keeps working even if 4DA is discontinued. The buyer psychographic (privacy-first, local-first developers) is the most subscription-averse segment in software; a buy-once option answers the "why does local software need rent?" objection that otherwise dominates launch threads. Priced at 3.0× annual (low end of the credible 3–5× band) rather than the un-decided $249 (2.5×) that had sat on the page since 2026-03-23, which over-cannibalized annual.
- **Considered:**
  - No lifetime (subscription only): Rejected — brand-dissonant for local-first; loses the anti-subscription buyer entirely.
  - Keep $249 (2.5× annual): Rejected — below the credible ratio band; converts the highest-LTV cohort at a discount.
  - Perpetual-license-plus-3-years-updates (Sublime/JetBrains model): Rejected — requires version-gated licensing that isn't built; current implementation (2099 expiry) already delivers honest full lifetime.
  - Capped "founder seats": Deferred — can be applied operationally at any time by deactivating the Stripe price; no code needed.
- **Context:** The lifetime toggle shipped 2026-03-23 inside an unrelated bulk commit with no decision record, and the Stripe price was never provisioned — the buy button 500'd until found by live E2E verification on 2026-07-26. This AD retroactively makes the call explicit.
- **Date:** 2026-07-26
- **Status:** Final
- **Amended:** 2026-08-12 (AD-030) — compounding rationale replaced; the $299 price and lifetime terms are unchanged.
- **Code:** `site/src/signal.njk` (plan toggle), `site/functions/api/signal/checkout.js` (payment mode), `site/functions/api/license/refresh.js` (lifetime entitlement), `site/functions/api/license/activate.js` (2099 expiry; at `api/streets/activate` until 2026-08-20), `site/setup-signal.mjs` (price provisioning), TOS §4.1/§4.2/§5.3.

### AD-029: Behavioral Learning Demoted from Scoring Authority
- **Decision:** Engagement-derived signals no longer move relevance scores or verdicts. Removed from scoring (PIPELINE_VERSION 19): the engagement multiplier's user-history terms (affinity ×0.3–1.7, anti-topics, feedback boosts, taste embedding, learned source quality), the confirmation gate's learned axis, the topic-attention-gap boost (deleted outright), persona-posterior and stability-facet score injections, ACE anti-topic auto-exclusions, autophagy scoring corrections (calibration deltas, topic half-lives, source/feed autopsies, anti-patterns, archetypes), synthetic affinity seeding, both threshold auto-tuners (frozen at the 0.40 default; kv resurrection path removed), and the MCP fallback scorer's affinity/anti weights. KEPT: all context learning (ACE stack/dep/git grounding), item-side community signal, verdict epochs, capture pipeline + Learned Preferences panel (pin/forget/reset), engagement dashboards, Brief-rejection demotions, and user-authored exclusions.
- **Rationale:** Accuracy first (Principle 5). Four documented incidents in two months: the 2026-07-13 doom loop (passive scroll noise drove the user's own stack to −1.0 affinity at ×[0.3,1.7] authority — "yesterday's noise becomes tomorrow's signal" running in reverse); the v18 attention-boost cap bypass (145 look-alike crates over threshold); the silently dead capture wiring (2026-06-07); and the 2026-08-11 degenerate calibration curve (fit from mislabeled pairings, survived a DB reset as a file, remapped honest 1/5 judgments to 5/5 and inflated 48 items/cycle by +0.15 while re-recording its own output as training data). The capture layer ran three incompatible strength scales (one recorded dismissals as +0.3 positive). The loop was permanently starved (7 explicit feedback at 200k-corpus scale) so its benefit was never measurable — all risk, no demonstrated lift.
- **Considered:**
  - Full removal (delete capture + learning): Rejected — destroys the honest, user-controllable surfaces (preferences panel, dashboards), orphans the product promise where it is load-bearing (AD-028 lifetime rationale, AD-025/017 tier moat), and is not needed for accuracy once authority is removed.
  - Fix-in-place (keep weighting, patch bugs): Rejected — the incident class re-offends (four in two months), the data remains starved, and an unmeasurable benefit cannot justify a repeatedly-measured cost.
- **Re-enable criteria (ALL required):** (1) one unified capture strength scale across ACE/ContextEngine/MCP with no positive-valued negative gestures; (2) a calibration-harness (/calibrate) measured lift over the neutral baseline on labeled data; (3) degeneracy guards on every fitted artifact at save AND load, with raw (pre-transform) values persisted; (4) fitted state bound to the corpus it was fit on (no artifact outliving its data); (5) a user-visible off switch.
- **Date:** 2026-08-11
- **Status:** Final
- **Amended:** 2026-08-17 (AD-031 / v20b) — the "Full removal" option this AD rejected was PARTIALLY adopted: the IMPLICIT half of the capture layer (scroll/ignore signals, the topic_affinities / anti_topics / source_preferences / activity_patterns tables, persona-posterior updates, the three affinity anomaly detectors) is now deleted, because the honest user-facing surfaces the rejection protected (Learned Preferences panel, its readers) were themselves removed in v20a — the "destroys honest surfaces" objection no longer applied. EXPLICIT capture (click/save/share/dismiss/mark_irrelevant/briefing/engagement_complete/save_with_context interactions, stability facets, record_item_feedback) and all context learning remain. Scoring is untouched (no PIPELINE_VERSION change); the re-enable criteria stand — a future re-enable now also requires rebuilding capture per criterion (1).
- **Code:** `scoring/pipeline_v2.rs`, `scoring/gate.rs`, `scoring/context.rs`, `scoring/semantic/boost.rs`, `scoring/triage.rs`, `scoring/affinity.rs` (now display-only), `monitoring.rs` + `ace/mod.rs` + `commands.rs` + `ace_commands/scanning.rs` (tuner freeze), `analysis_status.rs` (seed removal), `calibration.rs`/`calibration_store.rs`/`analysis_rerank.rs` (mesh guards), `db/migrations.rs` Phase 103, `mcp-4da-server/src/db.ts`. Amends INV-023.

### AD-030: Retire the "Gets Sharper Every Day" Product Promise
- **Decision:** Retire "gets sharper every day" as the canonical one-sentence description, and remove derivative compounding / behavioural-learning promises from all user-facing surfaces. New canonical: **"4DA reads the internet for developers — privately, locally. Your codebase decides what's relevant."** KEPT: "All signal. No feed." (brand line); "Yesterday's noise becomes tomorrow's signal" **re-attributed** to corpus re-judging (verdict epochs + `scoring/reexamination.rs`), never to user engagement; factual description of Learned Preferences as a user-controlled filter; ACE stack/dependency context learning; the 92%/98% benchmark with its methodology intact as body copy (never headline); "compound knowledge" in THE-4DA-FRAMEWORK's authority-stack sense (developer process, not scoring accuracy).
- **Rationale:** Accuracy first (Principle 5). AD-029 removed the mechanism the promise described, and INV-023 now fixes the Learned Behavior weight at 0.0, stating learned behavior "feeds ONLY user-facing surfaces — never scores or verdicts." Roughly 30 in-app strings, the installer metadata, the homepage JSON-LD and a published npm README still assert the removed mechanism — several inside Score Autopsy, the explainability surface, which makes them live INV-023 violations rather than marketing taste. The claim was unsupportable even before v19: AD-029 records the loop as "permanently starved… all risk, no demonstrated lift." Partial softening (PR #414) left ~100 instances standing and produced live self-contradiction: `README.md` says the Learned axis is "Reserved — held out of scoring" while `site/src/docs/scoring.md` on the same domain said it "boosts or suppresses future scores." Pre-launch (public but unadvertised) is the only cheap moment to fix this; after launch it is a retraction.
- **Considered:**
  - Keep the tagline and rely on the v19 softening: Rejected — softened and un-softened copy sat on the same domain contradicting each other; "every day" remains false on cadence (improvement is per engine-update) and agency (it implies the user's usage drives it).
  - Keep it as aspirational marketing: Rejected — an unmeasurable headline claim baked into installer metadata and search-indexed structured data is exposure, not aspiration.
  - Delete the whole arc including "yesterday's noise becomes tomorrow's signal": Rejected — that line is true and implemented (re-examination + verdict epochs); deleting true differentiators to atone for false ones is over-correction.
  - "…and rejects 92% of it" as the new tagline: Rejected — a simulation-derived figure in the headline slot immediately after retiring a claim for being unmeasurable repeats the original error. 92%/98% stays in body copy with methodology.
  - Rename the `compound_advantage` metric and MCP tool: Rejected — it measures realized outcomes (window response rate, lead time, knowledge-gap closure), not accumulated learning, and it is a published MCP tool name. UI labels change; the API name does not.
- **House rewrite rule:** replace forward-looking promises with present-tense verifiable statements; re-attribute improvement from "your engagement" to "engine updates and your codebase."
- **Re-claim criteria:** An "improves with use" claim may be made only after the AD-029 re-enable criteria are met AND a `/calibrate`-measured lift over the neutral baseline is published. At that point the claim would be made for the first time with evidence behind it.
- **Enforcement:** `scripts/check-retired-claims.cjs` (wired into `test:scripts` + `validate`) fails the build on retired phrases outside allowlisted historical-record files.
- **Date:** 2026-08-12
- **Status:** Final — amends AD-017, AD-025, AD-026, AD-028.

### AD-031: Remove the Implicit Behavioral-Capture Layer (v20b)
- **Decision:** Delete implicit behavioral capture end-to-end: the scroll/ignore signal emitters (`use-view-tracking.ts`, the `Scroll`/`Ignore` BehaviorAction variants, `on_implicit_skip`), the derived-profile writers (topic_affinities, anti_topics, source_preferences, activity_patterns, persona-posterior updates), their remaining readers of the now-dropped tables (the three affinity anomaly detectors, the tech-radar affinity overlay, the learned_behavior export section; standing-query suggestions and Developer DNA engaged-topics re-sourced to explicit engagement instead), and the backing tables (migration Phase 105 drops topic_affinities, anti_topics, activity_patterns, source_preferences, persona_posterior, posterior_snapshots). Also removed in the same change, as a separate proof class (write-only, not implicit): the `decision_outcome` digest analyzer, whose output no reader ever consumed. KEPT: explicit engagement capture (`interactions` rows for click/save/share/dismiss/mark_irrelevant/briefing_click/briefing_dismiss/engagement_complete/save_with_context), stability_detector facets, `record_item_feedback`, skill-gap detection over the explicit six-type predicate, the engagement dashboard, and data export of feedback/interactions. `interactions` rows are NOT deleted — the bootstrap-mode count (`scoring/context.rs`) reads them, and deleting rows would flip bootstrap state.
- **Rationale:** AD-029 removed implicit capture's scoring authority (v19); v20a removed its last honest UI surfaces. What remained was a write-only pipeline: signals captured, profiles built, nothing read them. A capture layer with no consumer is pure liability — privacy surface, storage growth, code weight, and resurrection risk (the exact class AD-029's four incidents came from). Explicit gestures stay because they have live, honest consumers.
- **Considered:**
  - Keep capture dormant for a future re-enable: Rejected — AD-029's re-enable criterion (1) requires a REBUILT capture layer with one unified strength scale; the existing layer is the thing that criterion disqualifies. Reflog and git history preserve everything.
  - Also delete implicit `interactions` rows: Rejected — bootstrap-mode is currently held OFF by those rows (live probe 2026-08-17, post-#482: 713 interactions, 296 with |signal_strength| >= 0.3, all 296 implicit, effective_feedback_count = 98), and deleting them would flip bootstrap ON and change scoring. Open v20c question: should bootstrap-exit require EXPLICIT signal? That is a scoring change and needs a PIPELINE_VERSION bump.
  - Also clean `mcp-4da-server`: Deferred to v20c — its reads degrade gracefully on missing tables; its `CREATE TABLE IF NOT EXISTS` in `db.ts` may resurrect empty topic_affinities/anti_topics tables in 4da.db (cosmetic, cleaned in v20c).
- **Scoring inertness:** NO PIPELINE_VERSION bump. The bootstrap term is untouched and its input rows are kept; golden snapshots and simulation-asserted numbers are unchanged.
- **Date:** 2026-08-17
- **Status:** Final — amends AD-029 (partial adoption of its rejected "full removal" option); retires INV-071.
- **Code:** `ace/behavior/` (tracking, types, decay; `queries.rs` deleted), `ace_commands/interactions.rs`, `engagement_telemetry.rs`, `scoring/ace_context.rs`, `anomaly.rs`, `taste_test/continuous.rs` (deleted), `autophagy/decision_outcomes.rs` (deleted), `data_export.rs`, `standing_queries_suggestions.rs`, `developer_dna.rs`, `tech_radar_compute.rs`, `db/migrations.rs` Phase 105, `src/hooks/use-view-tracking.ts` (deleted), `src/store/feedback-slice.ts`.

### AD-032: MCP tools/list Serves Real Schemas for Required-Param Tools
- **Decision:** In the MCP server's `tools/list` response, a tool whose JSON Schema declares `required` parameters serves its REAL `inputSchema`; tools whose parameters are all optional keep the slim `{"type":"object"}` with the full schema available as an MCP Resource (`4da://schema/{tool}`). The line is computed from the schema files at runtime (`schema-registry.ts::inputSchemaIfRequired`), never hand-maintained.
- **Rationale:** The all-slim design (a ~4500→~500 token optimization) assumed clients lazy-load schemas via MCP Resources; in practice most MCP clients never read Resources, so the five required-param tools (`record_feedback`, `decision_memory`, `agent_memory`, `check_decision_alignment`, `what_should_i_know`) were effectively uncallable — a client cannot construct a valid call without knowing the required params (GPT adversarial audit 2026-08-23, finding 7). Serving real schemas for exactly those five costs ~800 tokens; `{}` is already a valid call for every all-optional tool, so slim remains honest there.
- **Considered:**
  - Serve full schemas for all 14 tools: Rejected — ~1700 extra tokens per listing for information the all-optional tools do not need to be callable.
  - Keep all-slim and document the Resources path better: Rejected — documentation cannot fix clients that structurally never read Resources; the tools stay broken for them.
- **Date:** 2026-08-23
- **Status:** Final
- **Code:** `mcp-4da-server/src/schema-registry.ts`, tests in `mcp-4da-server/src/__tests__/schema-registry.test.ts`.

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

### AD-033: The Scoring Drain Is a Vector-Search Problem, Not a Scoring Problem

- **Decision:** Materialise the per-item context match (`item_context_cache`, schema 113) and maintain it INCREMENTALLY against a trigger-maintained context-corpus generation, rather than recomputing it on every re-score. Move the grounding filter into the vec0 partition key so that materialised value is a true top-K. Take the stale-version drain out of the analysis batch, stamp evidence for every item the scorer EVALUATED rather than every item that survived dedup, and trigger a drain-to-completion automatically when the backlog passes 5,000.
- **Rationale:** Measured on the live 53,000-item corpus, 2026-08-27:
  - Scoring one item costs 54.7 ms, of which **52.0 ms (95.8%) is a single sqlite-vec KNN**. All of PASIFA — topics, dependency matching across 143 packages, keywords, content DNA, the confirmation gate, every boost and ceiling — is the other 2.7 ms.
  - That KNN is a pure function of the item's embedding and the context corpus. **145 of 15,599 context chunks changed in fifteen days**, and of 600 sampled items, **zero** had a top-3 touching a changed chunk.
  - The v25+v26 arc was 17 commits and 4,392 lines; **none** were in `db/mod.rs`, `context_admission.rs`, `embeddings.rs` or `context_engine/`. The 96% term was provably invariant across that bump and was recomputed 47,000 times.
  - The in-cycle drain converted **5 of every 500** stale items: the batch layer deleted 831 of 1,458 scored results before the version stamp was written, and the drain items lost that contest systematically. Net 88 items/hour, 22 days to converge, 1.1% of the compute doing useful work.
  - `--engine-drain`, which converts 100%, cleared the same backlog in **22m07s** — but no code path could reach it; only a human typing a flag.
  - Verified after the change: the cache is **index-exact** on 400 real items both cold and after a context-corpus change that moved 352 of 400 top-3 lists, and the incremental merge propagated that change across all 53,752 items in **10 seconds** versus **560 seconds** for a full recompute.
- **Considered:**
  - *A plain generation-counter cache* (as first proposed): Rejected — it would have invalidated 100% of entries on those 145 chunk writes. The measurement that motivates the cache is a PRECISE invalidation, so the implementation had to be an exact delta-merge, not a version equality check.
  - *Lazy score-on-read:* Rejected previously and still rejected — threshold-before-rank surfaces (blind spots > 0.5, autophagy < 0.05, MCP) need scores for items that are never displayed.
  - *Binary quantisation of the context index (recommendation 6 as published):* **Rejected on measurement.** On this corpus, 768-bit codes with exact-L2 rescoring recover only **20% / 45% / 69%** of exact top-3 lists at 4x / 10x / 32x oversampling, and even at 32x, **10% of items get a different top-1** — a different grounding axis, a different score. Code embeddings cluster too tightly for sign-only quantisation. Fast (0.25 ms/item) and wrong is not a trade this product makes.
  - *An exact in-memory Rust f32 scan:* Measured **9.06 ms/item vs 29.78 ms** for sqlite-vec in a release build — exact, and 3.3x faster. NOT adopted: it is a second implementation of the most correctness-critical query in the product, and the cache already removes that scan from the steady state. Revisit only if the context corpus grows enough that a cold rebuild stops being a one-off (the current cold warm is ~9 minutes for 53,752 items).
  - *Per-axis epochs for every signal axis:* Deferred with evidence. The context axis needed materialising because it is 96% of the cost, and it is now epoch-scoped independently of `PIPELINE_VERSION` (the cache is keyed on the CONTEXT generation, so a scoring bump does not invalidate it). The remaining axes are collectively 2.7 ms/item — materialising them would save ~18 seconds on a whole-corpus drain. Registering an unconsumed axis registry would be dead code, which doctrine forbids. Revisit if a future axis acquires its own expensive input.
- **Date:** 2026-08-27
- **Status:** Final

### AD-034: A PIPELINE_VERSION Bump Declares a Blast Radius, Not a Release

- **Decision:** A `PIPELINE_VERSION` bump must correspond to a change in what `score_item` writes to `relevance_score`. Batch-relative changes (cross-encoder, dedup corroboration, diversity, per-source percentile, LLM advisor, final rank cap) write `top_score`/`rank_score` only and must NOT bump it. Where several scoring changes ship together, prefer landing them under separate bumps that each register a scoped epoch over bundling them into one unregistered bump.
- **Rationale:** v26 bundled five changes, one of which (`apply_source_share_diversity`) is a batch-relative ranking stage that provably cannot alter any stored `relevance_score`. Bundling forced the union of five blast radii onto the whole corpus and made the bump unregisterable, because no SQL predicate can bound the reach of the widest member. The version number should describe what was invalidated, not when it shipped.
- **Considered:**
  - *Keep bundling and rely on the drain being fast:* Rejected as the only mitigation — AD-033 makes a full drain affordable, but an affordable full drain is still strictly worse than a scoped one, and the discipline costs nothing at authoring time.
- **Date:** 2026-08-27
- **Status:** Final

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

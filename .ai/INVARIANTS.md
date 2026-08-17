# System Invariants
## What Must ALWAYS or NEVER Happen in 4DA

**Version:** 1.1.0
**Source:** Extracted from ACE-STONE-TABLET.md and ARCHITECTURE.md
**Authority:** These are non-negotiable constraints. Violating an invariant is a critical bug.

---

## The ACE Guarantees (Canonical)

These are the five inviolable guarantees from the ACE specification:

### INV-001: ACE Always Hits Its Mark (corrected 2026-08-16)
- Relevance quality is gated by the persona simulation in
  `src-tauri/src/scoring/simulation/reality.rs`. The enforced floors are:
  - **Aggregate precision MUST be >= 0.70** and **aggregate F1 >= 0.40**, measured
    across all personas (`reality_aggregate_summary`)
  - **Noise rejection MUST be >= 80% for every persona** — at most 20% of the items a
    persona is expected to reject may come back relevant
    (`reality_noise_rejection_all_personas`)
  - Per-persona precision/recall/F1 floors, set individually per persona via
    `SimMetrics::assert_quality` (precision floors range 0.40–0.70)
- Every relevance decision MUST be explainable
- Confidence scores MUST accurately reflect actual certainty
- **Violation Detection:** the simulation runs in `cargo test` and fails the build below
  any floor above. At runtime, `scoring::calibration_monitor` snapshots this developer's
  own precision/recall miss rates every 6h and warns when composite health drops below
  0.7 (`monitoring.rs`); it is cold-start-silent — no feedback, no signal.
- **What this used to say:** "Precision MUST be >85% or an alert MUST be triggered." No
  85% / 0.85 precision threshold exists anywhere in the codebase. The figure is an
  aspiration carried over from `specs/ACE-STONE-TABLET.md` §1 and was never implemented;
  quoting it as an invariant meant the doc asserted a gate that could not fail.

### INV-002: ACE Never Requires User Input
- System MUST work from first launch with zero configuration
- User input MUST enhance results but MUST NOT be required
- Basic functionality MUST be available without any setup
- **Violation Detection:** Test cold start scenario, verify output without config

### INV-003: ACE Never Fails Silently
- ALL errors MUST be logged with full context
- Graceful degradation MUST be preferred over crashes
- Health status MUST always be visible/queryable
- **Violation Detection:** Error handler coverage, health endpoint tests

### INV-004: ACE Respects Privacy Absolutely
- NO data leaves the machine without explicit user consent
- Activity tracking MUST be OFF by default
- User MUST be able to delete ALL data at any time
- **Violation Detection:** Network audit, data flow tracing

### INV-005: ACE Learns But Doesn't Creep
- User MUST always understand why items are shown
- NO unexplainable "magic" recommendations
- Learning signals MUST be transparent and inspectable
- **Violation Detection:** Explanation generation for all recommendations

---

## Performance Invariants

### INV-010: Latency Bounds
- Context lookup MUST complete in <100ms
- Recovery from any single failure MUST complete in <5s
- UI MUST remain responsive during background operations
- **Verification:** Performance benchmarks in CI

### INV-011: Memory Bounds
- ACE overhead MUST NOT exceed 100MB
- No unbounded growth in any data structure
- **Verification:** Memory profiling, leak detection

### INV-012: Cold Start Performance
- System MUST provide useful results within 5 user interactions
- Initial scan MUST complete within reasonable time for typical project sizes
- **Verification:** Cold start test suite

---

## Data Integrity Invariants

### INV-020: Confidence Thresholds
- Signals with confidence <0.3 MUST be rejected (not stored)
- No unvalidated data may enter the interest model
- **Code Pattern:**
```rust
if confidence < 0.3 {
    return None;  // MANDATORY rejection
}
```

### INV-021: Idempotent Database Writes
- All database write operations MUST be idempotent
- Duplicate requests MUST NOT corrupt state
- **Verification:** Replay tests, concurrent write tests

### INV-022: Embedding Consistency
- Same input text MUST always produce same embedding
- Embedding model changes MUST trigger full re-embedding
- **Verification:** Determinism tests

### INV-023: Context Layer Authority (amended by AD-029, 2026-08-11; corrected 2026-08-16)
- **Learned behavior contributes nothing to scoring** (AD-029, PIPELINE_VERSION 19). Its
  authority may ONLY be restored via the AD-029 re-enable criteria: a single unified
  capture strength scale, a calibration-harness-proven lift over the neutral baseline,
  degeneracy guards on every fitted artifact, and a user-visible off switch. Until then,
  learned behavior feeds ONLY user-facing surfaces (Learned Preferences panel, engagement
  dashboard) — never scores or verdicts. **This half is real and enforced in the pipeline.**
- **Static identity and active context are NOT weighted as layers.** There is no layer
  multiplier. `scoring::context` folds both into a single flat `Vec<Interest>` whose
  per-item `weight` comes from provenance:
  - detected primary tech 0.85, secondary tech 0.40 (`ace_ctx.tech_weights`)
  - active topics 0.50–0.75, scaled by detection confidence
  - direct dependencies 0.30
- **What this used to say:** "Static Identity weight: 1.0 / Active Context weight: 0.8",
  with a `STATIC_LAYER_WEIGHT` / `ACTIVE_LAYER_WEIGHT` / `LEARNED_LAYER_WEIGHT` code
  pattern described as CANONICAL. Those three constants have **zero occurrences in any
  `.rs` file**. The pattern was copied from `specs/ACE-STONE-TABLET.md` §6.1 and never
  built, so AD-029's amendment landed on a constant that did not exist — the correct
  behavior shipped in the pipeline, but the invariant documented a mechanism instead of
  the outcome. It now documents the outcome.
- **Violation Detection:** grep the pipeline for engagement-derived terms in any score
  path; AD-029 lists every removal site.

---

## Security Invariants

### INV-030: API Keys Never Logged
- API keys MUST NEVER appear in logs, errors, or debug output
- Credential fields MUST be redacted in all serialization
- **Verification:** Log audit, grep for key patterns

### INV-031: BYOK Integrity
- User API keys MUST be stored locally only
- NO transmission of API keys to any remote service (except the intended API)
- Users own their keys entirely
- **Verification:** Network traffic analysis
- **Decision (2026-06) — cloud-LLM consent is informed-disclosure, not an enforced gate.** The BYOK
  setup UI (onboarding + Settings → AI Provider) discloses what is sent to the provider; configuring
  a cloud provider with a key records that acceptance (`cloud_llm_disclosure_accepted`, set at
  configure-time in `settings/manager.rs`, never silently at call-time). We deliberately do NOT block
  cloud calls behind an acknowledgment gate: it is the user's own key and data with no third-party
  recipient — 4DA receives nothing — so a hard gate would add friction to the recommended BYOK path
  for no protective benefit. Zero-retention defaults: first-party OpenAI requests send `store:false`;
  other providers are governed by their own policy (documented in `NETWORK.md` §2a).

### INV-032: Local-First Architecture
- Core functionality MUST work completely offline (with Ollama)
- External API calls MUST gracefully degrade when unavailable
- **Verification:** Offline mode test suite
- **Known divergence (recorded 2026-08-16, not fixed).** The SSRF guard added to `llm.rs`
  re-validates the chat URL at use-time and exempts exactly one provider string:
  `if self.provider.provider != "ollama" { validate_not_internal(&url)? }`
  (`llm.rs:495` and `llm.rs:706`). Every other loopback LLM server is therefore rejected
  as an internal address — including the three the app itself probes for and offers the
  user in setup: LM Studio (`:1234`), llama.cpp (`:8080`) and Jan (`:1337`)
  (`settings_commands_llm/mod.rs:167-172`). "Works offline" currently means "works
  offline **with Ollama**" in the literal sense, and the four-server detector advertises
  three configurations that cannot complete a request. Fixing this means widening the
  exemption from a provider-name match to a loopback-host match; it is a code change and
  is deliberately not made here.

---

## Architectural Invariants

### INV-040: Tauri IPC Boundary
- All Rust↔Frontend communication MUST go through Tauri IPC
- No direct file system access from frontend
- Commands MUST be typed and validated
- **Verification:** IPC audit, type coverage

### INV-041: SQLite as Single Source of Truth for Corpus and Derived State (corrected 2026-08-16)
- **Content, scores, verdicts, context and every derived intelligence artifact MUST live in
  SQLite.** No second store may hold a copy that can disagree with the database.
- **Four subsystems persist state outside SQLite, by design.** Each is an exception with a
  reason, not a violation to be silently tolerated:
  - `data/settings.json` — user configuration. `settings/manager.rs:74` states it plainly:
    "The on-disk file is the authoritative source; the keychain is secondary." Settings must
    be hand-editable and must survive a corrupt or quarantined database.
  - **OS keychain** — API keys and webhook secrets (`settings/keystore.rs`,
    `webhooks/secrets.rs`), via the `keyring` crate. A SQLite file offers no at-rest
    protection for a credential; the platform keystore does.
  - `data/calibrations/{identity_hash}/{task}.json` — fitted calibration curves
    (`calibration_store.rs`). Write-rarely/read-once artifacts keyed by a stable hash;
    the module documents the choice under "Why files, not SQLite".
  - `data/signal_terminal_token.txt` — the Signal Terminal bearer token
    (`signal_terminal.rs:101`), which must be readable by the user without a DB client.
- **The rule that follows from those exceptions:** any state kept outside SQLite MUST be
  bound to the data it was derived from, or a database reset leaves a stale conclusion
  behind with its evidence gone. This is not hypothetical — see
  `FAILURE_MODES.md` → "Fitted artifact outlives the data it was fit on": a calibration
  curve fit on 2026-06-19 kept loading from disk after the 07-31 database reset wiped
  the samples it was fit from, and went undetected until 08-11.
- **Verification:** State audit. Any new out-of-DB persistence needs an entry above.
- **What this used to say:** "ALL persistent state MUST live in SQLite database. No state
  split across multiple storage mechanisms." It is false in four places, three of which
  document their own reasoning in the module that implements them — so the invariant was
  not describing a rule anyone intended to follow. An invariant contradicted by shipped
  code teaches readers to discount the file.

### INV-042: Error Handling Hierarchy
- Use `thiserror` for all custom error types
- Errors MUST propagate context (not just messages)
- **Code Pattern:**
```rust
#[derive(Error, Debug)]
pub enum MyError {
    #[error("Failed to {action}: {source}")]
    Contextual {
        action: String,
        #[source] source: SomeError,
    },
}
```

### INV-043: No Console Window From Spawned Processes
- fourda.exe is a GUI-subsystem binary (`windows_subsystem = "windows"`) and owns no console. Every
  child process it spawns that *can run on Windows* MUST suppress the console window, or Windows
  allocates a fresh console and a black window flashes on the user's desktop — which reads as malware.
- Every `std`/`tokio` `Command` reachable on Windows MUST set `CREATE_NO_WINDOW` (`0x0800_0000`),
  inline or via a helper (e.g. `suppress_console_window`).
- Background/headless binaries that are themselves console-subsystem (`fourda-engine`) MUST hide their
  own console at process entry — see `hide_scheduler_spawned_console()` in `headless.rs`.
- **Code Pattern:**
```rust
use std::os::windows::process::CommandExt;
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
cmd.creation_flags(CREATE_NO_WINDOW);
```
- **Enforcement:** `scripts/check-no-window-spawns.cjs` (`pnpm run validate:no-window`) — scans every
  `Command::new` under `src-tauri/src` and fails the build unless the flag is applied, the program is
  genuinely Unix-only, or an explicit `// no-window-ok: <reason>` marker justifies a visible window.
- **Origin:** 2026-06-10/11 — a 30-min background refresh, then a self-hosted CI runner, each flashed a
  console; the founder's first instinct was to kill the process. The gate makes silence by-construction
  instead of by-discipline.

---

## UI/UX Invariants

### INV-050: Matte Black Theme
- Primary background: #0A0A0A
- These colors are CANONICAL design system values
- Gold accent (#D4AF37) used sparingly for highlights only
- **Verification:** Visual regression tests

### INV-051: Accessible Contrast
- All text MUST meet WCAG AA contrast ratios
- Interactive elements MUST be clearly visible
- **Verification:** Accessibility audit

---

## Exclusion Strength Invariants

### INV-060: Exclusion Application (corrected 2026-08-16)
- **Exclusion is binary and absolute. There are no strength tiers.** A user-authored
  exclusion matching an item's topics sets `top_score = 0.0`, `relevant = false`,
  `excluded = true`, and records `excluded_by` — before any scoring work runs.
- Matching is substring-symmetric and case-insensitive: an item is excluded if any
  extracted topic contains an exclusion term or an exclusion term contains the topic
  (`utils/topics.rs::check_exclusions`).
- **Only user-authored exclusions exist.** ACE anti-topic auto-exclusion was removed in
  PIPELINE_VERSION 19 (AD-029) — a topic could be auto-banned on dismissal count alone.
  The suppress-topic button is the entire suppression path.
- **Code Pattern:**
```rust
// scoring::pipeline_v2 — exclusion check runs before any scoring work
if let Some(exclusion) = check_exclusions(&topics, &ctx.exclusions) {
    return SourceRelevance { top_score: 0.0, relevant: false,
                             excluded: true, excluded_by: Some(exclusion), .. };
}
```
- **What this used to say:** a three-tier Soft/Hard/Absolute model reducing score by
  50%/90%/100%, with "these percentages are CANONICAL". The `ExclusionStrength` enum it
  named has **zero occurrences in any `.rs` file**. It comes from
  `specs/ACE-STONE-TABLET.md` §5 / §6.2 and was never built — and its tier selector
  (`compute_exclusion_strength`) derived strength from dismissal counts, which AD-029
  retired outright. Nothing about the tiered model is recoverable.

---

## Behavioral Invariants

### INV-070: Temporal Decay
- Learned behavior has 30-day half-life
- Active context decays over 7 days
- **Code Pattern:**
```rust
// 30-day half-life for learned behavior
let decay = 0.5_f32.powf(days_since / 30.0);
```

### INV-071: Minimum Data for Learning — RETIRED (v20b, 2026-08-17, AD-031)
Retired because the implicit topic-affinity learning this invariant governed was
deleted outright in v20b — `RECOMPUTE_AFFINITY_SQL`, its three thresholds, and
the `topic_affinities` table no longer exist, so there is no learning left to
gate. (Note: the "display gate" bullet below was already stale before
retirement — its cited counter at `ace_commands\interactions.rs:429` was removed
in v20a.) The body is preserved as the historical record of what the gates were:

There is no single threshold. Three different ones were live, and one path had none:

- **Compute gate — 3.** `RECOMPUTE_AFFINITY_SQL` (`ace/behavior/tracking.rs:37`) only lets
  the positive/negative ratio drive `affinity_score` at `total_exposures >= 3`; below that
  the CASE falls through to `0.0`. Every result is additionally damped by
  `MIN(total_exposures / 10.0, 1.0)`, so a topic reaches full magnitude at 10, not 3.
- **Read gate — 5.** `get_topic_affinities()` filters `total_exposures >= 5`
  (`ace/behavior/queries.rs:11-12`). This is the only place the "5" in the old wording was
  ever true. `get_topic_affinities_min(n)` overrides it; tests use 1.
- **Display gate — >3.** The engaged-topic counter uses `total_exposures > 3`
  (`ace_commands/interactions.rs:429`), i.e. 4 or more. Stricter than the compute gate,
  looser than the read gate, for no stated reason.
- **Explicit rejection has NO exposure floor.** The first arm of the CASE fires whenever
  `explicit_negative_signals > 0 AND weighted_positive <= 0.0`, ahead of the `>= 3` check.
  One explicit dismissal of a never-engaged topic produces a negative affinity from a
  single exposure. That is deliberate — an explicit "never show me this" is evidence in a
  way that a scroll-past is not — but it means "no learning from insufficient data" is not
  true as an unqualified statement.
- **Scope note (AD-029).** Whatever any of these gates admit no longer reaches a score.
  Topic affinity feeds the Learned Preferences panel and the engagement dashboard only.
- **Code Pattern:**
```sql
-- ace/behavior/tracking.rs — affinity_score recompute
WHEN explicit_negative_signals > 0 AND weighted_positive <= 0.0 THEN   -- no floor
    -1.0 * MIN(CAST(total_exposures AS REAL) / 10.0, 1.0)
WHEN total_exposures >= 3 THEN                                        -- compute gate
    MAX(-1.0, MIN(1.0,
        (weighted_positive - weighted_negative) / CAST(total_exposures AS REAL)
    )) * MIN(CAST(total_exposures AS REAL) / 10.0, 1.0)
ELSE 0.0
```
- **What this used to say:** "Topic affinity MUST have ≥5 exposures before contributing",
  over a Rust `if self.total_exposures < 5 { return 0.0 }` pattern that does not exist —
  the gates are SQL, there are three of them, and one path bypasses all three.

---

## Validation Invariants

### INV-080: Multi-Source Confidence Boost
- Single source: base confidence
- Two sources agreeing: +10% confidence
- Three sources agreeing: +20% confidence (cap at 0.95 for inferred)
- Explicit user input: confidence = 1.0 (always wins)
- **Verification:** Cross-validation tests

### INV-090: File Size Limits (corrected 2026-08-16)
- New TypeScript/TSX files MUST stay under 500 lines
- New Rust files MUST stay under 1000 lines
- Warning thresholds are **`.ts` 300, `.tsx` 350, `.rs` 700** — the values in
  `scripts/check-file-sizes.cjs`, derived there from the repo's own median/p90
  distribution. The doc previously said TS 350 / RS 600, which matched neither.
- Test files (`*.test.*`, `*_tests.rs`) are exempt from warnings and error at **2x** the
  normal error threshold
- Files exceeding limits MUST be split or added to exception list with justification
- **Exception list:** `scripts/check-file-sizes.cjs` EXCEPTIONS constant
- **Enforcement:** Pre-commit hook, CI pipeline, `pnpm run validate:sizes`

---

## How to Use This File

1. **Before modifying code:** Check if any invariants apply to the area you're touching
2. **During code review:** Verify no invariants are violated
3. **After incidents:** Check which invariant was violated, add detection
4. **When adding features:** Consider if new invariants are needed

---

## Invariant Violation Protocol

If you discover an invariant violation:

1. **STOP** - Do not proceed with the current task
2. **DOCUMENT** - Record the violation and how it was discovered
3. **ASSESS** - Determine impact and scope
4. **FIX** - Create minimal fix that restores the invariant
5. **VERIFY** - Add test to prevent regression
6. **REPORT** - Update FAILURE_MODES.md if this was a new failure pattern

---

*These invariants are extracted from the canonical specifications. Changes require spec updates first.*

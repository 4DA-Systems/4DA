# Silent-Failure Defense Architecture

**Status:** Draft — Layer 1 + Layer 2 foundations landing in this commit.
**Last updated:** 2026-04-12
**Owner:** T-SILENT-FAILURE-DEFENSE (will be historical after this commit)
**Related antibodies:**
- `.claude/wisdom/antibodies/2026-04-12-silent-cli-failures.md`
- `.claude/wisdom/antibodies/2026-04-12-ghost-ipc-and-idempotency-amnesia.md`

---

## Why this document exists

Over one week of pre-launch hardening, three distinct bug reports were filed against 4DA. They looked unrelated on the surface:

1. **An external CLI tool was returning empty results** — root cause: a CLI flag was rejected as "Unknown stage", but the calling code never checked the exit status or scanned stderr for error strings. The call had been silently failing **for months**.

2. **A command was uncallable from the frontend** — root cause: a handler had `#[tauri::command]` attribute but was missing from `generate_handler!` in `lib.rs`. Frontend `invoke()` returned "command not found" at runtime only when a user actually tried to use the feature.

3. **The "immune scan pending" warning wouldn't go away** — root cause: the session-end hook detected "recent bug-fix commit in git log" and set the flag **every session**, with no memory of which commits had already been scanned. Clearing the flag was cosmetic.

Three surface symptoms, **one failure shape**:

> **A boundary between two systems where "success" on side A does not guarantee the intended effect on side B, and no loud signal fires when the effect fails to occur.**

Then, while investigating bug #3, I found bug #4 and #5:

4. **The hygiene parser silently ignored every T-named terminal claim** — root cause: regex `^###\s+(T[0-9]+)` only matched numeric terminal IDs. Every `T-WAR-ROOM`, `T-GLYPH`, `T-SCORING` claim was invisible to the parser, causing the "unclaimed files" warning to over-count systematically. Nobody noticed for weeks because the warning was always "kind of right."

5. **Test schemas drifted from production schemas** — multiple `CREATE TABLE` strings in test setup didn't include the `is_direct` column that migration 53 added to `project_dependencies`. Tests "passed" against incomplete schemas while production code used columns the tests didn't know about.

**Five boundaries, one pattern.** The fact that I kept finding new instances in different parts of the codebase means 4DA does not yet have a **class-level defense** against silent failures. We have individual reactive fixes, not systemic prevention.

This document is the architectural plan for the systemic prevention.

---

## Part 1 — The meta-pattern

Every silent failure has three ingredients. Remove any one and the failure becomes impossible.

### Ingredient 1: A boundary

Process boundary (CLI subprocess), module boundary (IPC), language boundary (Rust↔JS), persistence boundary (filesystem/DB), protocol boundary (HTTP), or abstraction boundary (regex/parser).

### Ingredient 2: Asymmetric success criteria

Side A defines "success" locally — exit code 0, HTTP 200, `cargo check` passed, function returned `Ok(())`. Side B's required outcome — the command actually ran, the data was actually persisted, the function is actually callable from the frontend, the correct rows were modified — is **not implied** by side A's local signal.

### Ingredient 3: Missing verification

Side A does not explicitly check that side B's outcome occurred. The code trusts the local signal and moves on.

**All three ingredients required.** A boundary without asymmetric criteria (e.g., a Rust function call where both caller and callee live in the same process and share types) is not silent-failure-prone. Asymmetric criteria with explicit verification are safe. Verification without a boundary is overkill.

The defenses in the rest of this document attack ingredient 3 — **forcing explicit verification at every boundary** — because ingredients 1 and 2 are inherent to distributed systems and can't be removed.

---

## Part 2 — Taxonomy of boundaries in 4DA

Ground-truth from a 2026-04-12 audit. Counts are approximate and may drift slightly as code evolves.

### Class 1 — Frontend ↔ Rust (Tauri IPC)

| | Current state |
|---|---|
| Sites | 443 `#[tauri::command]` occurrences across 113 files; ~374 unique command definitions; 385 handler registrations; 385 TypeScript `CommandMap` keys (+11 gap = feature-gated commands) |
| Current defense | ✅ `scripts/validate-commands.cjs` catches ghost commands + orphan TS keys. Runs in pre-commit. ✅ ts-rs generates TS types from Rust structs. ✅ `CommandMap` typed invoke wrapper (0 raw `invoke()` calls). |
| Silent-failure risks | Serde tolerates unknown fields by default — frontend can pass extra keys that Rust ignores. Rust can return `Ok(())` after a side-effect silently failed (DB write, filesystem write). Enum variant rename is unsafe if TS callers hardcoded the old name. |
| Risk rating | **Medium** |

### Class 2 — Rust ↔ External CLIs

| | Current state |
|---|---|
| Sites | ~20 `Command::new` sites. Breakdown: git: 1 site (`ace/git.rs`). System tools (powershell, codesign, ldconfig, lspci, reg, fc-list, ps, xdotool, where/which, npm, cargo): ~15 sites. Node/Python (plugins/loader): 3 sites. |
| Current defense | ❌ No typed wrapper enforcing contract verification at the type level. ❌ No clippy lint preventing raw `Command::new(...)` calls outside a defense module. |
| Silent-failure risks | **This is the class that introduced the silent-failure family to 4DA.** CLI contract drift. Unchecked stderr. Client-generated IDs not matched against server-assigned IDs. Missing exit-code checks. |
| Risk rating | **HIGH** |

### Class 3 — Rust ↔ HTTP APIs

| | Current state |
|---|---|
| Sites | 77 `reqwest::Client` occurrences across 38 files. Major consumers: `http_client.rs` (11), `llm.rs` (7), `llm_stream.rs` (5), `team_sync_scheduler.rs` (5), every source adapter (1-3 each), `ollama.rs` (3), `settings_commands_llm.rs` (3). Only **1** `.json::<serde_json::Value>` untyped site (in `settings/validation.rs`) — most HTTP call sites use typed deserialization, which is strong defense. |
| Current defense | ✅ Structured JSON deserialization via serde — shape drift fails loudly at deserialize step. ✅ `reqwest::Response::status().is_success()` check in most sites. ⚠️ No integration tests against real APIs (would be flaky). ❌ No structured logging of "successful call returned unexpected empty body." |
| Silent-failure risks | "API returned 200 with empty data because we asked the wrong question." Rate-limit responses treated as success. Stale cached responses. |
| Risk rating | **Medium** (typed deserialization is the main defense and it's mostly in place) |

### Class 4 — Rust ↔ SQLite

| | Current state |
|---|---|
| Sites | Hundreds across `db/`, `scoring/`, `decision_*`, `agent_memory.rs`. |
| Current defense | ✅ Transactional migrations with auto-rollback (`run_versioned_migration`). ✅ Pre-migration backups (last 2 kept). ✅ Preemptive `PRAGMA quick_check` before open (from `15f2c708`). ✅ Foreign-key enforcement via `PRAGMA foreign_keys = ON`. ❌ No check on **affected_rows** for UPDATEs — a query matching 0 rows "succeeds" with 0 changes. ❌ No schema drift detection between test setup `CREATE TABLE` strings and production migrations. |
| Silent-failure risks | **Demonstrated this week with `is_direct` column drift.** Updates that match 0 rows. `INSERT OR REPLACE` doing the wrong thing silently. Test schemas that don't match production. |
| Risk rating | **HIGH** |

### Class 5 — Rust ↔ Filesystem

| | Current state |
|---|---|
| Sites | Medium. `runtime_paths`, `db/`, `startup_watchdog` (markers), `settings/`. |
| Current defense | ✅ `RuntimePaths` centralizes resolution. ✅ `.running`/`.healthy`/`.stalled` marker protocol. ⚠️ Some sites use `.ok()` to silently ignore cleanup failures (deliberate best-effort). ❌ No fsync + read-back verification post-write. |
| Risk rating | **Medium** |

### Class 6 — Hooks ↔ Rust app state (marker files + ops-state)

| | Current state |
|---|---|
| Sites | ~5 marker files (`.running`, `.healthy`, `.stalled`, `.scanned`, `.smoke-test-results.json`) + `ops-state.json`. |
| Current defense | ✅ `startup_watchdog.rs` documents the marker protocol clearly. ✅ Clean-shutdown removes `.running`. ✅ `scannedBugFixCommits[]` dedup added this week (fixes idempotency amnesia). ❌ No schema definition for the marker files — drift between hook writer and Rust reader is possible. |
| Silent-failure risks | **Demonstrated twice this week:** idempotency amnesia in session-end hook; hygiene parser regex too narrow. |
| Risk rating | **HIGH** |

### Class 7 — Regex parsers

| | Current state |
|---|---|
| Sites | **0 dynamic `Regex::new` sites in Rust** — all regexes compile-time or `Lazy<Regex>`. Node/Bash scripts use regex liberally in hooks (`session-stop-unified.cjs`, `git-hygiene-check.sh`, etc.). |
| Current defense | ✅ Rust syntax errors fail at `cargo check`. ❌ Semantic narrowness (hygiene parser `T[0-9]+` missing T-named) is silent in Rust AND in Node. ❌ Most script regexes have no tests against real-world input variants. |
| Silent-failure risks | **Demonstrated this week** with the hygiene parser. Any regex-based script that processes git log, commit messages, TERMINALS.md entries, version strings, etc., is at risk if the input format evolves. |
| Risk rating | **Medium** (small surface in Rust, but demonstrated bug in script layer) |

### Class 8 — Serde/ts-rs type synchronization

| | Current state |
|---|---|
| Sites | ~100+ shared types via `#[derive(TS)]`. |
| Current defense | ✅ ts-rs generates `.ts` from Rust at build time. ✅ IPC validator cross-references. ❌ Enum variant renames can break runtime invoke calls that hardcoded old names. |
| Risk rating | **Low-Medium** |

---

## Part 3 — The defense architecture (5 layers)

Defense in depth. The goal: any silent failure must escape **all five layers** to reach a user.

### Layer 1 — Type-level enforcement (compile time)

**Goal:** Make the silent-failure pattern impossible to write by construction.

**Mechanisms:**

- **Typed external-tool wrappers** — `src-tauri/src/external/` with `OllamaClient`, `GitClient`. Each method returns `Result<TypedOutput, TypedError>`. You cannot obtain a `TypedOutput` without passing all contract checks. The compiler enforces verification.
- **Forbid raw `Command::new` for known external tools** — clippy `disallowed-methods` lint (if per-argument matching ever lands) OR custom pattern check in `validate-boundary-calls.cjs`.
- **Typed HTTP response deserialization** — prefer typed structs over `serde_json::Value`. Only 1 untyped site in the codebase currently; keep it that way.
- **`deny_unknown_fields` on settings structs** — catches field-rename drift.

**First iteration shipping in this commit:** `src-tauri/src/external/mod.rs` skeleton. Methods defined, call-site migration deferred to a follow-up.

### Layer 2 — Pre-commit contract validators (commit time)

**Goal:** Catch violations of the boundary-verification pattern before they land in HEAD.

**Mechanisms:**

- ✅ **`scripts/validate-commands.cjs`** — already exists, already in pre-commit. Catches ghost IPC commands. **Keep this load-bearing.**
- **`scripts/validate-boundary-calls.cjs`** (NEW, shipping in this commit) — greps for the unverified `Command::new` pattern. Flags any `Command::new(...).output()` that doesn't have a nearby `status.success()` or stderr check. Flags hook scripts that set warning flags without a corresponding "scanned" set.
- **Future: `scripts/validate-schema-drift.cjs`** — compares test-schema `CREATE TABLE` strings against production migration `CREATE TABLE` strings. Flags drift. (Backlog; not in this commit.)
- **Future: `scripts/validate-regex-tests.cjs`** — finds every `Regex::new(...)`, `new RegExp(...)`, and regex in Bash scripts, checks for a nearby test asserting the regex matches real input. (Backlog; not in this commit.)

**First iteration shipping in this commit:** `validate-boundary-calls.cjs` — ready to wire into pre-commit hook in a follow-up.

### Layer 3 — Integration tests that run real binaries (CI time)

**Goal:** Catch contract drift between 4DA and external tools at test time, not in production.

**Mechanisms:**

- **Real-binary integration tests** — expand to cover every external client method. Similar for Ollama.
- **Full migration chain test** — `test_sqlite_migration_chain.rs` that runs all 54+ migrations in sequence against a fresh temp DB, asserts none panic or roll back.
- **Anti-mocking rule** — any external-integration module with a mocked unit test MUST also have a real-binary integration test. Enforced via code review.

**Not yet shipping:** belongs in a follow-up commit. Layer 1 wrappers are a prerequisite.

### Layer 4 — Cold-boot smoke test (first-launch time)

**Goal:** Catch drift between 4DA and the user's environment (stale Ollama, missing binary, corrupted DB, disk full) before they hit it in a feature.

**Mechanisms:**

- **`src-tauri/src/smoke_test.rs`** — new module. On first launch of each cold boot, spawns a background task that exercises every critical boundary with a tiny probe:
  - Ollama: `GET /api/version` (already in `check_ollama_version`)
  - SQLite: `SELECT sqlite_version()` + `PRAGMA quick_check`
  - Filesystem: write+read+delete a `.smoke-test-probe` file
  - Keyring: read a known probe secret (if enabled)
  - Tauri IPC: self-invoke one no-op probe command
- Writes results to `data/.smoke-test-results.json`.
- Next cold boot reads results, surfaces regressions as `HealthIssue` via the existing channel.

**Not yet shipping:** backlog item. Reuses a lot of `startup_health.rs` and `startup_watchdog.rs` infrastructure.

### Layer 5 — Production telemetry with anomaly detection (deployed time)

**Goal:** Catch drift in the wild when user environment + 4DA code combine in unpredictable ways.

**Mechanisms:**

- **Structured logging** — every boundary crossing logs a `*.started` and `*.completed` (or `.failed`) event with `duration_ms`, `result_type`, and `error_category`.
- **Local aggregation** — `telemetry.rs` (already exists) rolls up hourly counters by boundary + result type, stored in `4da.db`.
- **Anomaly alarms** — if success rate of any boundary drops below 50% over a 1-hour window, surface as `HealthIssue` severity Error.
- **Privacy-first** — telemetry stays local, never leaves the machine.

**Not yet shipping:** infrastructure exists in `telemetry.rs`, needs extension to aggregate by boundary and threshold on anomalies.

---

## Part 4 — Prioritized recommendations

Ranked by `(silent-failure coverage × 1/effort)`. Items higher in the list have the best ROI.

### Priority 1 (shipping in this commit) — `validate-boundary-calls.cjs`

**What:** A Node script that greps the Rust codebase for `Command::new(` sites and checks that the next N lines contain a verification pattern (`status.success()`, stderr scan, or a call to a known-safe wrapper). Flags any site that doesn't.

**Why first:** Cheapest, broadest, runs on every commit, catches the pattern that introduced the silent-failure family to 4DA.

**Cost:** ~4 hours of implementation, 0 runtime cost.

### Priority 2 (backlog) — `external::ollama::OllamaClient`

Same pattern for Ollama HTTP calls. Small number of call sites, but critical because Ollama is the embedding + chat backbone. ~1 day.

### Priority 3 (backlog) — `validate-schema-drift.cjs`

Diff test-schema `CREATE TABLE` strings against production migration strings. The `is_direct` bug this week was exactly this. ~4 hours.

### Priority 4 (backlog) — `src-tauri/src/smoke_test.rs`

Cold-boot self-test. Catches user-environment drift. ~1 day. Reuses `startup_health` infrastructure.

### Priority 5 (backlog) — Integration tests for every external tool

Real-binary tests for every `OllamaClient` method. Anti-mocking enforcement. ~2 days setup + ongoing maintenance.

### Priority 6 (backlog, post-launch) — Production telemetry aggregation + anomaly alarms

Layer 5. Extends existing `telemetry.rs`. ~2 days.

### Priority 7 (ongoing) — Code review discipline

Anti-patterns to reject in review:
- New `Command::new("ollama"|"git"...)` outside `src-tauri/src/external/`
- `.json::<serde_json::Value>` in HTTP deserialization (prefer typed struct)
- `.unwrap_or(default)` on `Result` types (prefer typed error propagation)
- Hook scripts setting `*Pending = true` without matching `scanned*` set
- New regex without a nearby test case against real input

---

## Part 5 — Metrics and dashboards

You can't ship a bulletproof system you can't measure. These metrics prove the defense architecture is working.

| Metric | Source | Target |
|---|---|---|
| Ghost IPC commands detected at commit | `validate-commands.cjs` | 0 |
| Unverified `Command::new` sites detected at commit | `validate-boundary-calls.cjs` | 0 (after migration to wrappers) |
| External-tool integration tests passing | CI | 100% |
| Cold-boot smoke-test success rate (per boundary) | `data/.smoke-test-results.json` | >99% |
| Production boundary success rate (per boundary, per 1h) | `4da.db::telemetry` | >99% |
| Active antibodies in `.claude/wisdom/antibodies/` | filesystem | growing over time |
| **New silent-failure bug reports per month** | git log + manual classification | **→ 0** |

The last metric is the north star. If the defense architecture is working, the rate of new silent-failure bugs should decay exponentially over months. A flat curve means more layers are needed.

---

## Part 6 — Anti-goals

These patterns make silent failures WORSE. Avoid them.

### Anti-goal 1 — Fire-and-forget defaults

Never write `let _ = do_something()` without a comment explaining why the error is being ignored. The 290 `let _ =` sites in the current codebase are a mix of legitimate fire-and-forget (cleanup, telemetry emission, best-effort logging) and real silent failures (ignored HTTP errors, ignored DB write errors). **Pre-launch action:** audit the 290 sites, tag each with a category, fix the real silent failures. Estimate ~1-2 days of focused work.

### Anti-goal 2 — Catching Exception: pass

The Rust equivalent:
```rust
match risky_op() {
    Ok(v) => v,
    Err(_) => default_value, // ← silent failure if default is wrong
}
```
Use typed errors, at minimum log the error. If a default is genuinely correct, document why. The `.unwrap_or(default)` pattern on `Result` is suspect.

### Anti-goal 3 — Over-abstraction that hides boundaries

A generic `execute(cmd: &str) -> String` helper that takes any command string and returns any output hides the boundary, making per-boundary verification impossible. **Keep each external boundary visible and explicit.** `OllamaClient::embed(...)` is better than `generic_cli("ollama", &["embed", ...])`.

### Anti-goal 4 — Exclusive reliance on unit tests with mocks

Mocks pass forever even if the real thing silently broke months ago. **Never ship an external integration whose only test is a mock.** Always pair with at least one real-binary test.

### Anti-goal 5 — Warnings without clear-paths

A sentinel warning that says "immune scan pending" with no way to clear it creates idempotency amnesia. Every warning must include: (a) what happened, (b) what the user should do, (c) how to mark the issue as handled.

### Anti-goal 6 — "Tests pass so we're done"

Tests catch regressions, not silent failures. Silent failures bypass tests because the tests verify the wrong thing (mocks, partial schemas, local success signals). Treat tests as one layer among five.

### Anti-goal 7 — Perfect-is-enemy-of-good

Don't block launch on 100% boundary coverage. Ship Layers 1+2+3 for high-risk boundaries (Ollama, SQLite migrations). Skip low-risk ones until post-launch. Protection is additive.

---

## Part 7 — Immediate execution plan

What ships in THIS commit:

1. **This strategy document** — canonical source of truth for the architecture.
2. **`scripts/validate-boundary-calls.cjs`** — Priority 1 validator. Flags unverified `Command::new` sites and hook flag-without-dedup patterns. Ready to wire into `.husky/pre-commit` in a follow-up.
3. **`src-tauri/src/external/mod.rs`** — skeleton for typed external wrappers. Defines method signatures and the internal `invoke` helper that performs exit-code + stderr checks. **Not yet wired** to call sites — migration is a follow-up.
4. **`.claude/TERMINALS.md`** — active claim for T-SILENT-FAILURE-DEFENSE during this commit, moved to historical record on completion.

What comes in follow-up commits:

5. **Run `validate-boundary-calls.cjs` against current HEAD**, collect the backlog of violations, file as issues, fix in batches.
6. **Wire `validate-boundary-calls.cjs` into `.husky/pre-commit`** once the backlog is down to a manageable count.
7. **Migrate remaining raw `Command::new(...)` call sites** to typed wrappers. After migration, `validate-boundary-calls.cjs` can ban raw invocations outside `external/`.
8. **`external::ollama::OllamaClient`** — same pattern for Ollama.
9. **`src-tauri/src/smoke_test.rs`** — Layer 4 cold-boot self-test.
10. **`validate-schema-drift.cjs`** — test-schema vs production-migration diff.
11. **Integration test suite** in `src-tauri/tests/integration/`.

---

## Part 8 — Current session audit pass findings

A one-shot audit against HEAD (2026-04-12) surfaced these real counts:

**Rust ↔ CLI boundary (Class 2):** ~50 `Command::new` sites total.
- git: 1 site (`ace/git.rs`)
- System tools (powershell, codesign, ldconfig, lspci, reg, fc-list, ps, xdotool, where, which, npm, cargo, fc-list): ~15 sites scattered across `startup_health.rs`, `diagnostics.rs`, `integrity.rs`, `local_audit.rs`, `lib.rs`, `free_briefing.rs`, `settings/helpers.rs`, `desktop_pin.rs`, `plugins/loader.rs`
- Node/Python sidecar plugins: 3 sites in `plugins/loader.rs`
- fc-list / reg: 2 sites in `startup_health.rs` (already have contract checks — OK)

**Rust ↔ HTTP boundary (Class 3):** 77 `reqwest::Client` hits across 38 files. Well-distributed. **Only 1 `serde_json::Value` untyped deserialization** — strong default.

**Rust ↔ IPC boundary (Class 1):** 443 `#[tauri::command]` occurrences (many files have multiple decorators). Validator reports 374 unique defs / 385 registrations / 385 TS keys — clean.

**Rust ↔ Regex boundary (Class 7):** **0 dynamic `Regex::new` sites** — all regexes are compile-time or `Lazy`. Script-layer risk remains (Node/Bash hooks).

**Error-swallowing surface:** 290 `let _ = <expr>` occurrences across 66 files. **Not all are silent failures** — many are legitimate fire-and-forget (telemetry emission, cleanup, best-effort logging, event emission). **Pre-launch audit action:** walk the list, tag each with one of:
- `fire_and_forget_ok` (telemetry, logging, event emission — fine)
- `best_effort_cleanup` (file removal, cache invalidation — fine)
- `deliberate_partial` (last-resort recovery fallback — fine)
- **`silent_failure_fix_me`** — the ones that need typed error propagation

Estimate ~20-40 of the 290 are genuine silent-failure sites that need fixes. ~1-2 days of focused audit work.

---

## Part 9 — The honest part

This architecture doesn't cover every failure mode. Specifically, it doesn't catch:

1. **User-environment drift** — graphics driver update breaks WebGPU. Layer 4 smoke test catches some; Layer 5 telemetry catches more; neither is bulletproof.
2. **Semantic drift** — an API still works but means something subtly different (e.g., Ollama tokenizer change → embedding drift). Defense is out of scope for mechanical boundary checks; requires periodic content-quality audits.
3. **Data poisoning** — upstream data sources containing intentionally misleading content. Defense is domain-specific (sanitization, ranking, trust scoring).
4. **LLM hallucination** — model confidently generates wrong output. Defense is out of scope for boundary checking; requires consequence-modeling at the application layer.

The architecture is for **mechanical silent failures** — broken integrations, unverified contracts, drift between side A's local success and side B's actual outcome. Semantic and data-quality failures are a separate class, handled by different systems (PASIFA scoring, content moderation).

## Why this matters specifically for 4DA

4DA is an **intelligence system that depends on correctness to be trusted.** If an external tool returns garbage because the CLI silently failed, the user has no way to know the intelligence is compromised. The feedback loop is broken. Compound intelligence cannot compound when the input signal is silently contaminated.

Every silent failure in 4DA isn't just a bug — it's a **trust erosion event.** Users relying on proactive intelligence need to trust that when 4DA says "here's what matters," the underlying pipelines actually ran correctly. Silent failures are structurally incompatible with the product promise.

The five bugs I found this week were harmless only because I happened to look. A pre-launch 4DA with 5-10 more of the same shape sitting in it would ship with those failures, and users would discover them in the worst way — by making wrong decisions based on broken data.

---

## Part 10 — The bulletproof definition

4DA is "bulletproof against silent failures" when:

1. ✅ Every boundary in the taxonomy (Parts 1-2) has at least Layer 2 + Layer 3 coverage (validator + integration test)
2. ✅ Every HIGH-risk boundary (Ollama, SQLite migrations, filesystem writes, hook state) has all 5 layers
3. ✅ New bug-fix commits automatically spawn antibodies AND trigger audit passes for the same class across the codebase
4. ✅ Production telemetry surfaces any anomaly before the user hits it in a feature
5. ✅ Pre-commit validators catch 95%+ of new instances before they land
6. ✅ Integration tests run on CI for every external dependency
7. ✅ Code review rejects the anti-patterns in Part 6

A new silent failure at that point would need to escape all five layers simultaneously. That's rare, and when it happens, Layer 5 (production telemetry) catches it before it spreads.

**Current state: ~Layer 2 coverage on IPC (validate-commands.cjs), 0 coverage on other classes.** This commit adds Layer 1 skeleton + Layer 2 validator for the CLI class. Full coverage is a ~2-week pre-launch sprint.

---

## Cross-references

- `.claude/wisdom/antibodies/2026-04-12-silent-cli-failures.md` — Bug 1 + Bug 2 (the external CLI instances)
- `.claude/wisdom/antibodies/2026-04-12-ghost-ipc-and-idempotency-amnesia.md` — Bug 3 + Bug 4 (ghost IPC + hook idempotency)
- `docs/strategy/PRELAUNCH-HARDENING.md` — related pre-launch risk mitigations
- `.ai/FAILURE_MODES.md` — should be updated with a "Silent Failure Family" section cross-referencing this document (follow-up action item)
- `scripts/validate-commands.cjs` — existing Layer 2 IPC validator
- `scripts/validate-boundary-calls.cjs` — NEW Layer 2 CLI validator (this commit)
- `src-tauri/src/external/` — NEW Layer 1 typed wrapper module (skeleton in this commit)

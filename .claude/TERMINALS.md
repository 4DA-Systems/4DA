# Terminal Coordination

## Protocol
1. **Before editing**: Read this file. If your files are claimed by another terminal, STOP.
2. **Claim files**: Add your entry below with the files you'll touch.
3. **Commit Lock**: Set `**Commit Lock**: HELD` before committing. Only one terminal commits at a time.
4. **After committing**: Remove your entry and release the lock.
5. **Conflicts**: If two terminals touch the same file, the one that committed first wins. The other must rebase.

## Active Terminals

<!-- Add entries below. Format:
### T[N] — [short description]
- **Status**: working | committing | done
- **Since**: ISO timestamp (e.g., 2026-03-22T10:00:00Z)
- **Commit Lock**: HELD | (omit if not held)
- **Files**: list of files being modified
-->

### T-WAR-ROOM — AWE deep wiring (critical bugs + real-time + autonomous tiers)
- **Status**: working — implementing full AWE assessment plan
- **Prior waves** (done): d8823c5d source-balanced DB, 80433345 Wisdom Trajectory redesign, 1297a15e chapter health dots
- **Current scope**:
  - src-tauri/src/awe_commands.rs (bug fixes + event emission + autonomous wiring)
  - src-tauri/src/context_commands.rs (bug #1 invalid stage name)
  - src-tauri/src/monitoring.rs (autonomous tiers in daily job)
  - src-tauri/src/awe_events.rs (NEW — typed Tauri event layer)
  - src-tauri/src/awe_autonomous.rs (NEW — Tier 0 seed, Tier 1 classify, Tier 3 retriage)
  - src-tauri/src/awe_source_mining.rs (NEW — Tier 2 source-item decision mining)
  - src/store/awe-slice.ts (incremental event-driven updates)
  - src/hooks/use-awe-live-events.ts (NEW — event subscription hook)
  - src/components/awe/MomentumWisdomTrajectory.tsx (subscribe to live events)
- **NOT touching**: lib.rs, app_setup.rs, commands.ts, en/ui.json (all have uncommitted changes from other terminals); any file in T-PRELAUNCH-HARDENING or T-SCORING claims

### T-GLYPH — Glyph Envelope Protocol (GEP) foundation
- **Status**: staged — waiting for T-PRELAUNCH-HARDENING to release Commit Lock before 4DA commit
- **Since**: 2026-04-11T10:00:00Z
- **Glyph repo**: D:\runyourempire\glyph — **COMMITTED** (74cba31, 44 files, 5829 insertions, 30+ tests passing)
- **Files staged for 4DA commit (docs-only)**: CLAUDE.md (+49 lines Glyph section), docs/glyph/GEP-SPEC.md, GEP-SAFETY.md, GEP-INTEGRATION.md, GEP-ALPHABET.md, .claude/TERMINALS.md (this file)
- **Files in staging**: confirmed via `git add` with explicit paths only — zero overlap with any other terminal's claim
- **Commit ready**: user or next session may run `git commit` once T-PRELAUNCH-HARDENING releases
- **NOT touching**: any src-tauri/src/** file, any migrations, any src/components/**, db/sources.rs. Rust integration deferred to Phase 2 follow-up commit.

### T-PRELAUNCH-HARDENING — Pre-launch risk mitigations (wave 2 — WIRED)
- **Status**: committing
- **Commit Lock**: HELD
- **Since**: 2026-04-11T22:55:00Z
- **Files**: src-tauri/src/startup_health.rs, src-tauri/src/ollama.rs, src-tauri/src/db/migrations.rs, src-tauri/src/db/mod.rs (1-line `mod migrations` → `pub(crate) mod migrations`), src-tauri/src/state.rs (recover-before-Database::new + result stash, near line 271, far from any other claimed hunk), docs/strategy/PRELAUNCH-HARDENING.md
- **db/mod.rs surgical change**: line 13 only (`mod migrations` → `pub(crate) mod migrations`). T-SCORING's db/mod.rs work has not yet started — coexists at hunk level.
- **state.rs surgical change**: lines around 271 only (preemptive recovery insertion). The orphaned 0.35→0.40 threshold change at line 698 is from a different prior session and is NOT mine — different hunk, can be split via git add -p or committed together if user prefers.
- **Scope**: WIRED — `recover_corrupt_db_if_needed` now runs before `Database::new()`, results surfaced via existing `HealthIssue` channel through the new `set_db_recovery_notice`/`take_db_recovery_notice` static. NO new Tauri commands, NO new frontend listeners needed (existing health-issue display handles it).
- **NOT touching**: db/sources.rs (T-WAR-ROOM's prior claim), Momentum.tsx, MomentumWisdomTrajectory.tsx, awe-slice.ts, en/ui.json, awe_commands.rs/awe_events.rs/awe_source_mining.rs (T-WAR-ROOM in-progress), context_engine.rs (T-SCORING), app_setup.rs (orphaned changes from prior session, not mine to commit), lib.rs (orphaned changes), tauri.conf.json.

### T-SCORING — Scoring hardening + onboarding + feedback instrumentation
- **Status**: working
- **Since**: 2026-04-11
- **Files**: src-tauri/src/scoring/dependencies.rs, src-tauri/src/scoring/pipeline_v2.rs, src-tauri/src/scoring_config.rs, src-tauri/scoring/pipeline.scoring, src-tauri/src/ace/scanner.rs, src-tauri/src/context_engine.rs, src/components/onboarding/*, src-tauri/src/db/mod.rs
- **NOT touching**: startup_health.rs, ollama.rs, migrations.rs, docs/glyph/*, Momentum*.tsx, CategoryChapterView.tsx

### T-PREEMPTION-FIX — Fix preemption timeout + blind_spots schema bugs (worktree isolated)
- **Status**: working — in isolated worktree ../4DA-preemption-fix
- **Since**: 2026-04-12T00:00:00Z
- **Worktree**: D:\4DA-preemption-fix (branched from origin/main, not local HEAD)
- **Files**: src-tauri/src/blind_spots.rs, src-tauri/src/preemption.rs, src-tauri/src/project_health_dimensions.rs, scripts/validate-wiring.cjs (add SQL column check), new tests in src-tauri/src/blind_spots.rs + preemption.rs + new test file
- **Scope**: 8 fixes — column name corrections (name→package_name, i.created_at→i.timestamp), is_direct filtering, score normalization, missed_signals dedup via user_events, real why_relevant, compute_all_project_health project cap, detect_chains memoization, health dimensions is_direct batching
- **NOT touching**: ANY file claimed by T-WAR-ROOM, T-GLYPH, T-PRELAUNCH-HARDENING, or T-SCORING. No lib.rs, no app_setup.rs, no migrations.rs, no scoring/**, no sources/**, no awe_*, no startup_health.rs, no state.rs, no project_health.rs (only *_dimensions.rs).
- **Target**: preemption feed returns in <5s against real 239MB DB (vs current 30s+ timeout)

### T2 — Phases 0-3 execution — COMPLETE
- **Status**: done — 10 commits pushed
- **Phase 0 — 3834a557** (23 files): RuntimePaths, audit bugs, Clippy gate
- **Strategy docs — 792ec1ad** (7 files): Master strategy + 6 phase plans
- **Phase 1.1 — 20ab8271** (7 files): preemption.rs, blind_spots.rs, trust_ledger.rs + migration 52
- **Phase 1.3 — 93d7295f** (12 files): PreemptionView, BlindSpotsView, TrustDashboard
- **Test fixes — e9dbc459** (2 files): Tab count assertions
- **Phase 1.4 — c9d31173** (12 files): Decision health, briefing integration, trust feedback loop
- **Phase 2 — f8a3a5b9** (4 files): Precision computation engine, domain breakdown, FP analysis
- **Phase 3 — 09f6078d** (8 files): MCP trust_summary, preemption_feed, enhanced context packet
- **Total**: 10 commits, 75+ files, ~12,000 lines added, 2,541 Rust + 1,193 frontend = 3,734 tests



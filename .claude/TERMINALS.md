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

### T-GLYPH — Glyph Envelope Protocol (GEP) foundation — DONE
- **Status**: done
- **Glyph repo commits**: 74cba31 (foundation, 44 files, 30 tests), 9d4c609 (Phase 2 harness + tokenizer, 11 files, 5 tests)
- **4DA commit**: e0da9bf5 (docs only, 6 files, 590 insertions)
- **Total**: 55 files in glyph repo, 35 tests passing, 6 files in 4DA
- **Phase 2 template**: D:\runyourempire\glyph\crates\glyph-integration-harness is the drop-in reference. When T-SCORING/T-PREEMPTION-FIX release their claims on src-tauri/src/db/*, the 4DA glyph_integration module can be built from this template.
- **Phase 0 kill gate**: provisional PASS (offline avg 2.00 tokens/glyph, max 2). Real Anthropic API measurement still pending — run scripts/measure-tokens.mjs with ANTHROPIC_API_KEY for authoritative verdict.

### T-PRELAUNCH-HARDENING — DONE (wave 1+2+3 in HEAD)
- **Status**: done
- **Commits**: 15f2c708 (wave 1+2 — wrong message due to race, contents are pre-launch hardening), 96ba9fed (wave 3 — offline installer config, key rotation runbook, status doc update)
- **Coverage**: All four pre-launch risk classes (a-d) mitigated. WebView2 version check + Ollama version check + DB corruption recovery (wired) + MSVC redist (already in .cargo/config.toml). Both installer modes buildable. Key rotation runbook written.
- **Strategy doc**: docs/strategy/PRELAUNCH-HARDENING.md (current source of truth)
- **Key rotation runbook**: docs/strategy/UPDATER-KEY-ROTATION.md

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



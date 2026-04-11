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

### T-HYGIENE — Orphaned worktree cleanup + prevention script — DONE
- **Status**: done
- **Commit**: 5eea8b1e
- **Deleted**: 11 dead worktree-agent-* branches + 6 stale directories (4 git-tracked + 2 orphaned)
- **Added**: scripts/cleanup-orphaned-worktrees.cjs (safe dry-run by default) + CLAUDE.md Worktree Hygiene section
- **Reflog**: all 11 branches recoverable for 90 days

### T-DOCS-HYGIENE — Strategy docs batch commit — DONE
- **Status**: done
- **Commit**: 9f62eb7c (7 files, 932 insertions)
- **NOT touched**: src-tauri/src/awe_commands.rs (T-WAR-ROOM's active claim)

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

### T-GLYPH — Phase 2 integration — DONE
- **Status**: done
- **Phase 2 commit**: 7548a690 (cherry-picked from worktree glyph-phase2/85a73b54)
- **Files landed**: src-tauri/Cargo.toml (+5 optional deps, +glyph_audit feature), src-tauri/Cargo.lock, src-tauri/src/db/migrations.rs (Phase 54 glyph_audit table), src-tauri/src/lib.rs (1-line mod decl), src-tauri/src/glyph_integration/** (5 new files, 4 passing tests)
- **Feature off by default.** Default cargo check clean. cargo check --features glyph_audit clean. 4 glyph_integration tests pass.
- **Prior commits**: glyph repo 74cba31→b0db4cb (4 commits), 4DA docs e0da9bf5/953f6d78/8b87de21, lock release 07b3b641

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

### T-PREEMPTION-FIX — Fix preemption timeout + blind_spots schema bugs — COMPLETE
- **Status**: done — COMMITTED dd71762b, pushed to origin/main
- **Files**: src-tauri/src/blind_spots.rs, src-tauri/src/preemption.rs, scripts/validate-wiring.cjs
- **Result**: preemption feed 248s→2.9s (85x speedup). Blind spot score no longer pinned to 100. Missed signals dedup via user_events. Real why_relevant via dep mention detection.
- **Tests**: 22 new unit tests (14 blind_spots + 8 preemption), 2,632 total Rust tests passing
- **New guard**: wiring-validator check #9 "SQL schema column drift" catches future column-name bugs
- **Worktree**: D:\4DA-preemption-fix (removed after commit)

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



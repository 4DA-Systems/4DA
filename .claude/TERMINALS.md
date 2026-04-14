# Terminal Coordination

## Protocol
1. **Before editing**: Read this file. If your files are claimed by another terminal, STOP.
2. **Claim files**: Add your entry below with the files you'll touch.
3. **Commit Lock**: Set `**Commit Lock**: HELD` before committing. Only one terminal commits at a time.
4. **After committing**: Remove your entry and release the lock.
5. **Conflicts**: If two terminals touch the same file, the one that committed first wins. The other must rebase.

## Active Terminals

### T-GAME-PURGE (claim)
**Files**: src/lib/game-components/**, src/lib/game-components.ts, src/hooks/use-game-component.ts,
src/**/*.tsx, src/**/*.ts (all consumers), public/notification.*, public/briefing.*,
src/App.css, scripts/validate-game-components.cjs, package.json, src/vite-env.d.ts,
src-tauri/src/game_engine.rs, src-tauri/src/game_achievements.rs, src-tauri/src/game_commands.rs,
src-tauri/src/game_engine_stub.rs, src-tauri/src/game_achievements_stub.rs,
src-tauri/src/lib.rs, src-tauri/src/db/migrations.rs, src-tauri/src/*.rs (all consumers)
**Commit Lock**: HELD
**Work**: 5-commit full purge of game-* references — frontend module/CSS, Rust modules, DB schema, IPC commands, build tooling. Pre-launch, no users, safe to edit migration in place.

---

## Completed in recent sessions (historical record, no active claim)

- **T2** — Phases 0-3 execution (10 commits: 3834a557, 792ec1ad, 20ab8271, 93d7295f, e9dbc459, c9d31173, f8a3a5b9, 09f6078d). Total 75+ files, ~12,000 lines, 3,734 tests.
- **T-PREEMPTION-FIX** — preemption feed 248s→2.9s (85x speedup), blind_spots + project_health_dimensions schema fixes. Commit `dd71762b`. Worktree removed.
- **T-SCORING** — experience UI + direct/transitive deps + tighter threshold + anti-gaming recal. Commits `26ef0e48`, `700ab104`.
- **T-GLYPH** — Glyph Envelope Protocol foundation (4 docs commits) + Phase 2 integration module (commit `7548a690`, feature-gated behind `glyph_audit`). 4 passing tests.
- **T-PRELAUNCH-HARDENING** — All four pre-launch risk classes (a-d) mitigated. WebView2 + Ollama version checks, DB corruption recovery (wired), static-CRT verified. Commits `15f2c708`, `96ba9fed`, `2b59be0d`, `d0b5070d`, `76de616b`. Strategy doc: `docs/strategy/PRELAUNCH-HARDENING.md`. Key rotation runbook: `docs/strategy/UPDATER-KEY-ROTATION.md`.
- **T-HYGIENE** — orphaned worktree cleanup + prevention script. Commit `5eea8b1e`. Deleted 11 dead branches + 6 stale directories (reflog recoverable 90 days).
- **T-DOCS-HYGIENE** — strategy docs batch (7 files, 932 insertions). Commit `d6fc22a0` (was `9f62eb7c` pre-rebase). Later relocated out of product repo by `7b081c1f`.
- **T-WAR-ROOM (recovered)** — AWE app_handle threading: register_awe_app_handle + cached_awe_app_handle + run_awe_autonomous_now command, 102 insertions. Committed by T-LOCK-CLEANUP as `0f0ae5aa` after user confirmed T-WAR-ROOM was no longer active. cargo check clean.
- **T-LOCK-CLEANUP** — stalled-terminal recovery + diverged branch rebase. Removed stale stash lock, committed T-WAR-ROOM's work, rebased 13 local commits onto origin/main (picked up `dd71762b`), pushed to origin. Tip: `0f0ae5aa`. User authorized all steps.
- **T-SILENT-FAILURE-DEFENSE** — Silent-Failure Defense Architecture Layer 1 + Layer 2 foundations. Strategy doc (`docs/strategy/SILENT-FAILURE-DEFENSE.md`), Layer 2 validator (`scripts/validate-boundary-calls.cjs`, caught ops-session-end.cjs idempotency-amnesia bug on first run), Layer 1 typed wrapper skeleton (`src-tauri/src/external/awe.rs` with 5 passing guard tests encoding Bug #1/#2/binary-not-found defenses), lib.rs `mod external;` declaration, fixed ops-session-end.cjs hook amnesia. 48 unverified `Command::new` sites remain as documented migration backlog. User authorized commit-lock override.

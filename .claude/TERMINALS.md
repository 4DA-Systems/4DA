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

### T-LOCK-CLEANUP — Stalled-terminal recovery + diverged branch rebase
- **Status**: committing
- **Commit Lock**: HELD
- **User authorization**: explicit — user confirmed the terminals holding locks are no longer active
- **Scope**:
  1. Remove stale .git/index.stash.62140.lock (Mar 1, 6 weeks old)
  2. Commit src-tauri/src/awe_commands.rs (102 insertions, syntactically closed, cargo check clean) as T-WAR-ROOM recovery
  3. Release T-IMMUNE-HYGIENE-FOLLOWTHROUGH stale lock in TERMINALS.md (their work already landed as 55ed60ee + 569620eb)
  4. Rebase 12 local commits onto origin/main to pull in dd71762b (T-PREEMPTION-FIX)
  5. Push to origin
- **Files**: src-tauri/src/awe_commands.rs, .claude/TERMINALS.md

---

## Completed in recent sessions (historical record, no active claim)

- **T2** — Phases 0-3 execution (10 commits: 3834a557, 792ec1ad, 20ab8271, 93d7295f, e9dbc459, c9d31173, f8a3a5b9, 09f6078d). Total 75+ files, ~12,000 lines, 3,734 tests.
- **T-PREEMPTION-FIX** — preemption feed 248s→2.9s (85x speedup), blind_spots + project_health_dimensions schema fixes. Commit `dd71762b`. Worktree removed.
- **T-SCORING** — experience UI + direct/transitive deps + tighter threshold + anti-gaming recal. Commits `26ef0e48`, `700ab104`.
- **T-GLYPH** — Glyph Envelope Protocol foundation (4 docs commits) + Phase 2 integration module (commit `7548a690`, feature-gated behind `glyph_audit`). 4 passing tests.
- **T-PRELAUNCH-HARDENING** — All four pre-launch risk classes (a-d) mitigated. WebView2 + Ollama version checks, DB corruption recovery (wired), static-CRT verified. Commits `15f2c708`, `96ba9fed`, `2b59be0d`, `d0b5070d`, `76de616b`. Strategy doc: `docs/strategy/PRELAUNCH-HARDENING.md`. Key rotation runbook: `docs/strategy/UPDATER-KEY-ROTATION.md`.
- **T-HYGIENE** — orphaned worktree cleanup + prevention script. Commit `5eea8b1e`. Deleted 11 dead branches + 6 stale directories (reflog recoverable 90 days).
- **T-DOCS-HYGIENE** — strategy docs batch (7 files, 932 insertions). Commit `9f62eb7c`.

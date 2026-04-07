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

### T1 — Sovereign Cold Boot architecture (8-wave overhaul) — COMPLETE
- **Status**: done — all 8 waves committed
- **Wave 1 (foundation) — COMMITTED 5816ee06** (7 files, 454 LOC)
- **Wave 2 (UX critical) — COMMITTED b758ee7e** (9 files, 609 LOC)
- **Waves 3+4+5 (instant paint + boot context + watchdog) — COMMITTED e59df4e8** (12 files, 854 LOC)
- **Waves 6+7+8 (instrumentation + webview recovery + regression gate) — COMMITTED 3037a39e** (5 files, 601 LOC)
- **Total**: 4 commits, 24 unique files, ~2500 LOC, 25 new tests, 41 regression-gate checks
- All 8 waves verified compiling, all targeted tests passing, IPC validator clean, TypeScript clean.




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

### T1 — Sovereign Cold Boot architecture (8-wave overhaul)
- **Status**: committing
- **Since**: 2026-04-07T03:00:00Z
- **Commit Lock**: HELD
- **Wave 1 (foundation) — COMMITTED 5816ee06** (7 files, 454 LOC)
- **Wave 2 (UX critical) — COMMITTED b758ee7e** (9 files, 609 LOC)
- **Waves 3+4+5 (frontend instant-paint + boot context + watchdog) — READY TO COMMIT**: src/main.tsx, src/components/BriefingView.tsx, src/hooks/use-analysis.ts, src/store/briefing-slice.ts, src/store/types.ts, src-tauri/src/boot_context.rs (NEW), src-tauri/src/startup_watchdog.rs (NEW), src-tauri/src/lib.rs, src-tauri/src/app_setup.rs, src-tauri/src/monitoring.rs, .gitignore
- **Wave 6 (phased startup rewrite)**: src-tauri/src/app_setup.rs
- **Wave 7 (webview navigation listener)**: src-tauri/src/app_setup.rs, src-tauri/src/startup_watchdog.rs
- **Wave 8 (CI cold-boot benchmark)**: scripts/benchmark-cold-boot.cjs (NEW)



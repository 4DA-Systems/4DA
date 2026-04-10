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

### T2 — Codex Audit Strategy & Execution Plans
- **Status**: working
- **Since**: 2026-04-10T12:00:00Z
- **Files**: MASTER-STRATEGY.md, PLAN-PHASE-0-FOUNDATION-FIXES.md, PLAN-PHASE-1-FLAGSHIP-LOOPS.md, PLAN-PHASE-2-PROOF-LAYER.md, PLAN-PHASE-3-MCP-PRODUCTIZATION.md, PLAN-PHASE-4-CROSS-PLATFORM.md, PLAN-PHASE-5-PREEMPTION-ENGINE.md



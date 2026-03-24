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

### T1 — Proactive intelligence hardening + launch pipeline
- **Status**: committing
- **Since**: 2026-03-25T13:00:00Z
- **Commit Lock**: HELD
- **Files**:
  - Wave 1: .github/workflows/release.yml, paddle-webhook/*, docs/LAUNCH-PIPELINE.md, docs/SHOW-HN-DRAFT.md, docs/PROACTIVE-INTELLIGENCE-PLAN.md, .claude/commands/swans.md
  - Wave 2: src/components/first-run/FirstRunTransition.tsx, src-tauri/src/scoring/pipeline.rs
  - Wave 3: mcp-4da-server/src/tools/what-should-i-know.ts
  - Wave 4: src-tauri/src/signal_chains.rs, src-tauri/src/decision_advantage/windows.rs

